//! Поиск по записям: индекс в памяти, разбор запроса, ранжирование.
//!
//! Индекс строится из расшифрованных метаданных после разблокировки и живёт, пока
//! хранилище открыто. Пароли и другие секреты в него не попадают.
//!
//! Запрос:
//! - слова — нечёткий поиск по названию, логину, сайтам, приложениям и тегам
//!   и поиск подстроки в тексте заметок; найтись должно каждое слово;
//! - `#тег` — записи с этим тегом; если такого тега нет — с тегом, который так
//!   начинается (удобно, пока тег ещё набирают);
//! - `is:login`, `is:note`, `is:fav` — тип записи и избранное, `has:totp` — есть 2FA.
//!
//! Если по словам ничего не нашлось, запрос повторяется в другой раскладке
//! клавиатуры: `пшерги` → `github`.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use nucleo_matcher::pattern::{Atom, AtomKind, CaseMatching, Normalization};
use nucleo_matcher::{Config, Matcher, Utf32String, chars};
use serde::{Deserialize, Serialize};

use crate::model::{ItemId, ItemKind, ItemMeta, normalize_tag};
use crate::vault::ItemSummary;

/// Порядок результатов.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    /// По совпадению с запросом; без слов — сначала избранное, потом недавние.
    #[default]
    Relevance,
    /// По названию.
    Title,
    /// Недавно изменённые первыми.
    Updated,
    /// Часто используемые первыми.
    Used,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchOptions {
    pub sort: SortOrder,
    /// Сколько записей вернуть; 0 — все.
    pub limit: usize,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            sort: SortOrder::Relevance,
            limit: 100,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchResult {
    /// Не больше `limit` записей.
    pub items: Vec<ItemSummary>,
    /// Сколько всего записей подходит под запрос.
    pub total: usize,
    /// Слова запроса в другой раскладке, если результаты найдены по ним.
    pub layout_fixed: Option<String>,
    /// Запрос без слов в порядке [`SortOrder::Relevance`]: первые `favorites`
    /// записей — избранные, за ними `recent` недавно использованных, дальше
    /// остальные по названию. В остальных случаях оба числа — 0.
    pub favorites: usize,
    pub recent: usize,
}

/// Тег и число записей с ним (вне корзины).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TagCount {
    /// Как тег записан в самой старой записи с ним.
    pub name: String,
    pub count: usize,
    #[serde(skip)]
    key: String,
}

/// Разобранный запрос.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Query {
    /// Слова для нечёткого поиска.
    pub words: Vec<String>,
    /// Теги без `#`.
    pub tags: Vec<String>,
    pub kind: Option<ItemKind>,
    pub favorite: bool,
    pub has_totp: bool,
}

/// Значения операторов `is:` и `has:`.
const IS_VALUES: [&str; 4] = ["login", "note", "fav", "favorite"];
const HAS_VALUES: [&str; 2] = ["totp", "2fa"];

impl Query {
    pub fn parse(input: &str) -> Self {
        let mut query = Query::default();
        for token in input.split_whitespace() {
            if let Some(tag) = token.strip_prefix('#') {
                let tag = normalize_tag(tag);
                if !tag.is_empty() {
                    query.tags.push(tag);
                }
                continue;
            }
            if let Some((op, value)) = token.split_once(':') {
                let op = op.to_lowercase();
                let value = value.to_lowercase();
                let known: &[&str] = match op.as_str() {
                    "is" => &IS_VALUES,
                    "has" => &HAS_VALUES,
                    _ => &[],
                };
                match (op.as_str(), value.as_str()) {
                    ("is", "login") => query.kind = Some(ItemKind::Login),
                    ("is", "note") => query.kind = Some(ItemKind::Note),
                    ("is", "fav" | "favorite") => query.favorite = true,
                    ("has", "totp" | "2fa") => query.has_totp = true,
                    // Оператор ещё набирают (`is:no`) — пока не фильтруем по нему.
                    _ if known.iter().any(|v| v.starts_with(value.as_str())) => {}
                    _ => query.words.push(token.to_owned()),
                }
                continue;
            }
            query.words.push(token.to_owned());
        }
        query
    }
}

/// Вес поля при совпадении слова (из 4).
const WEIGHT_TITLE: u32 = 4;
const WEIGHT_TAG: u32 = 3;
const WEIGHT_PLACE: u32 = 3;
const WEIGHT_USERNAME: u32 = 2;
/// Совпадение в тексте заметки: очки за каждый символ слова.
const BODY_PER_CHAR: u32 = 6;

const FAVORITE_BONUS: u32 = 24;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;

/// Раздел «Недавние» при пустом запросе: сколько записей и за какой срок.
const RECENT_LIMIT: usize = 5;
const RECENT_WINDOW_MS: i64 = 7 * DAY_MS;

/// Группы при пустом запросе — в очках, чтобы сортировать как обычный поиск.
const GROUP_FAVORITE: u32 = 2;
const GROUP_RECENT: u32 = 1;

/// Короткое поле записи, подготовленное для нечёткого поиска.
#[derive(Debug, Clone)]
struct Field {
    text: Utf32String,
    weight: u32,
    /// Какие символы есть в поле, см. [`char_mask`].
    mask: u128,
}

/// Запись в индексе с подготовленными для поиска полями.
#[derive(Debug, Clone)]
struct Entry {
    item: ItemSummary,
    /// Название, логин, сайты, приложения, теги.
    fields: Vec<Field>,
    /// Символы всех коротких полей вместе.
    mask: u128,
    /// Теги в нижнем регистре.
    tags: Vec<String>,
    /// Текст заметки в нижнем регистре, `ё` → `е`.
    body: String,
    body_mask: u128,
    /// Название для сортировки.
    title_key: Vec<u32>,
}

impl Entry {
    fn new(item: ItemSummary) -> Self {
        let mut fields = Vec::new();
        let mut add = |text: &str, weight: u32| {
            if !text.is_empty() {
                let text = fold_yo(text);
                fields.push(Field {
                    mask: char_mask(&text),
                    text: Utf32String::from(text.as_ref()),
                    weight,
                });
            }
        };
        add(item.meta.title(), WEIGHT_TITLE);
        let mut body = String::new();
        match &item.meta {
            ItemMeta::Login(login) => {
                add(&login.username, WEIGHT_USERNAME);
                for place in login.urls.iter().chain(&login.apps) {
                    add(place, WEIGHT_PLACE);
                }
            }
            ItemMeta::Note(note) => body = fold(&note.body),
        }
        for tag in item.meta.tags() {
            add(tag, WEIGHT_TAG);
        }

        Entry {
            mask: fields.iter().fold(0, |mask, f| mask | f.mask),
            fields,
            tags: item.meta.tags().iter().map(|t| t.to_lowercase()).collect(),
            body_mask: char_mask(&body),
            body,
            title_key: title_key(item.meta.title()),
            item,
        }
    }
}

/// Слово запроса, подготовленное к сравнению.
struct Word {
    atom: Atom,
    /// Символы слова: поле без любого из них можно не сравнивать.
    mask: u128,
    /// Для поиска подстроки в тексте заметок.
    folded: String,
    chars: u32,
}

impl Word {
    fn new(word: &str) -> Self {
        let folded = fold(word);
        Word {
            atom: Atom::new(
                &fold_yo(word),
                CaseMatching::Ignore,
                Normalization::Smart,
                AtomKind::Fuzzy,
                false,
            ),
            mask: folded
                .chars()
                .fold(0, |mask, c| mask | char_bit(chars::normalize(c))),
            chars: u32::try_from(folded.chars().count()).unwrap_or(u32::MAX),
            folded,
        }
    }

    /// Могут ли все символы слова найтись в строке с такой маской.
    fn fits(&self, mask: u128) -> bool {
        self.mask & !mask == 0
    }

    /// Очки лучшего совпадения слова с записью или `None`, если слова в ней нет.
    fn score(&self, entry: &Entry, matcher: &mut Matcher) -> Option<u32> {
        let mut best: Option<u32> = None;
        if self.fits(entry.mask) {
            for field in &entry.fields {
                if !self.fits(field.mask) {
                    continue;
                }
                if let Some(score) = self.atom.score(field.text.slice(..), matcher) {
                    let score = u32::from(score) * field.weight / 4;
                    best = Some(best.map_or(score, |b| b.max(score)));
                }
            }
        }
        if best.is_none() && self.fits(entry.body_mask) && entry.body.contains(&self.folded) {
            best = Some(self.chars.saturating_mul(BODY_PER_CHAR));
        }
        best
    }
}

/// Какие символы есть в строке — по биту на латинскую и русскую букву и цифру,
/// остальные символы делят оставшиеся биты. Учитывает, что nucleo не различает
/// регистр и диакритику. Слово может найтись в строке, только если все биты
/// слова есть в маске строки; так большинство записей отсеивается без nucleo.
fn char_mask(text: &str) -> u128 {
    text.chars().fold(0, |mask, c| {
        let lower = chars::to_lower_case(c);
        mask | char_bit(lower) | char_bit(chars::normalize(lower))
    })
}

fn char_bit(c: char) -> u128 {
    let code = u32::from(c);
    let bit = match c {
        'a'..='z' => code - u32::from('a'),
        '0'..='9' => 26 + code - u32::from('0'),
        'а'..='я' => 36 + code - u32::from('а'),
        _ => 68 + code % 60,
    };
    1 << bit
}

/// Требование к тегам записи.
enum TagFilter {
    Exact(String),
    Prefix(String),
}

impl TagFilter {
    fn matches(&self, tags: &[String]) -> bool {
        match self {
            TagFilter::Exact(tag) => tags.iter().any(|t| t == tag),
            TagFilter::Prefix(prefix) => tags.iter().any(|t| t.starts_with(prefix.as_str())),
        }
    }
}

/// Индекс записей вне корзины.
#[derive(Debug, Clone, Default)]
pub struct SearchIndex {
    entries: Vec<Entry>,
    tags: Vec<TagCount>,
}

impl SearchIndex {
    pub fn new(items: impl IntoIterator<Item = ItemSummary>) -> Self {
        let mut index = SearchIndex {
            entries: items
                .into_iter()
                .filter(|item| item.deleted_at.is_none())
                .map(Entry::new)
                .collect(),
            tags: Vec::new(),
        };
        index.count_tags();
        index
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Добавляет или обновляет запись. Запись из корзины убирается из индекса.
    pub fn upsert(&mut self, item: ItemSummary) {
        if item.deleted_at.is_some() {
            self.remove(item.id);
            return;
        }
        let entry = Entry::new(item);
        match self.position(entry.item.id) {
            Some(i) => self.entries[i] = entry,
            None => self.entries.push(entry),
        }
        self.count_tags();
    }

    /// Запись использовали: обновляет счётчик без расшифровки.
    pub fn record_use(&mut self, id: ItemId, at: i64) {
        if let Some(i) = self.position(id) {
            let item = &mut self.entries[i].item;
            item.use_count += 1;
            item.last_used_at = Some(at);
        }
    }

    pub fn remove(&mut self, id: ItemId) {
        if let Some(i) = self.position(id) {
            self.entries.swap_remove(i);
            self.count_tags();
        }
    }

    /// Теги с количеством записей: самые частые первыми.
    pub fn tags(&self) -> &[TagCount] {
        &self.tags
    }

    pub fn search(&self, input: &str, options: &SearchOptions) -> SearchResult {
        self.search_at(input, options, now_ms())
    }

    fn search_at(&self, input: &str, options: &SearchOptions, now: i64) -> SearchResult {
        let query = Query::parse(input);
        let result = self.run(&query, options, now);
        if result.total > 0 || query.words.is_empty() {
            return result;
        }
        // Ничего не нашлось — возможно, запрос набран не в той раскладке.
        if let Some(words) = switch_layout(&query.words) {
            let fixed = Query {
                words,
                ..query.clone()
            };
            let mut alt = self.run(&fixed, options, now);
            if alt.total > 0 {
                alt.layout_fixed = Some(fixed.words.join(" "));
                return alt;
            }
        }
        result
    }

    fn run(&self, query: &Query, options: &SearchOptions, now: i64) -> SearchResult {
        let tags: Vec<TagFilter> = query.tags.iter().map(|t| self.tag_filter(t)).collect();
        let words: Vec<Word> = query.words.iter().map(|w| Word::new(w)).collect();
        let mut config = Config::DEFAULT;
        config.prefer_prefix = true;
        let mut matcher = Matcher::new(config);

        let mut hits: Vec<(u32, &Entry)> = Vec::new();
        'entries: for entry in &self.entries {
            let meta = &entry.item.meta;
            if query.kind.is_some_and(|kind| kind != meta.kind())
                || (query.favorite && !meta.favorite())
                || (query.has_totp && !matches!(meta, ItemMeta::Login(l) if l.has_totp))
                || !tags.iter().all(|filter| filter.matches(&entry.tags))
            {
                continue;
            }
            let mut score = 0u32;
            for word in &words {
                match word.score(entry, &mut matcher) {
                    Some(s) => score = score.saturating_add(s),
                    None => continue 'entries,
                }
            }
            if !words.is_empty() {
                score = score.saturating_add(bonus(&entry.item, now));
            }
            hits.push((score, entry));
        }

        let total = hits.len();
        let by_words = !words.is_empty();
        let (favorites, recent) = if !by_words && options.sort == SortOrder::Relevance {
            group(&mut hits, now)
        } else {
            (0, 0)
        };
        let cmp = |a: &(u32, &Entry), b: &(u32, &Entry)| compare(options.sort, by_words, a, b);
        let limit = if options.limit == 0 {
            total
        } else {
            options.limit
        };
        if hits.len() > limit {
            // Сортируем только то, что вернём.
            hits.select_nth_unstable_by(limit, cmp);
            hits.truncate(limit);
        }
        hits.sort_unstable_by(cmp);
        SearchResult {
            items: hits.into_iter().map(|(_, e)| e.item.clone()).collect(),
            total,
            layout_fixed: None,
            favorites,
            recent,
        }
    }

    /// `#тег`: точное совпадение, если такой тег есть, иначе начало тега.
    fn tag_filter(&self, tag: &str) -> TagFilter {
        let key = tag.to_lowercase();
        if self.tags.iter().any(|t| t.key == key) {
            TagFilter::Exact(key)
        } else {
            TagFilter::Prefix(key)
        }
    }

    fn position(&self, id: ItemId) -> Option<usize> {
        self.entries.iter().position(|e| e.item.id == id)
    }

    fn count_tags(&mut self) {
        // Тег, записанный по-разному, называется так, как в самой старой записи.
        let mut counts: HashMap<&str, (TagCount, ItemId)> = HashMap::new();
        for entry in &self.entries {
            let id = entry.item.id;
            for (tag, key) in entry.item.meta.tags().iter().zip(&entry.tags) {
                counts
                    .entry(key.as_str())
                    .and_modify(|(c, first)| {
                        c.count += 1;
                        if id < *first {
                            c.name.clone_from(tag);
                            *first = id;
                        }
                    })
                    .or_insert_with(|| {
                        let count = TagCount {
                            name: tag.clone(),
                            count: 1,
                            key: key.clone(),
                        };
                        (count, id)
                    });
            }
        }
        let mut tags: Vec<TagCount> = counts.into_values().map(|(c, _)| c).collect();
        tags.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.key.cmp(&b.key)));
        self.tags = tags;
    }
}

/// Делит записи пустого запроса на избранные, недавние и остальные: группа
/// записывается вместо очков. Возвращает размеры первых двух групп.
fn group(hits: &mut [(u32, &Entry)], now: i64) -> (usize, usize) {
    let mut recent: Vec<(i64, usize)> = Vec::new();
    let mut favorites = 0;
    for (i, (score, entry)) in hits.iter_mut().enumerate() {
        let item = &entry.item;
        *score = 0;
        if item.meta.favorite() {
            *score = GROUP_FAVORITE;
            favorites += 1;
        } else if let Some(used) = item.last_used_at
            && now - used < RECENT_WINDOW_MS
        {
            recent.push((used, i));
        }
    }
    recent.sort_unstable_by_key(|&(used, _)| std::cmp::Reverse(used));
    recent.truncate(RECENT_LIMIT);
    for &(_, i) in &recent {
        hits[i].0 = GROUP_RECENT;
    }
    (favorites, recent.len())
}

/// Прибавка к очкам совпадения: избранное, частота и давность использования.
fn bonus(item: &ItemSummary, now: i64) -> u32 {
    let mut bonus = 0;
    if item.meta.favorite() {
        bonus += FAVORITE_BONUS;
    }
    if item.use_count > 0 {
        let uses = u64::try_from(item.use_count).unwrap_or(0);
        bonus += (8 * (uses + 1).ilog2()).min(40);
    }
    if let Some(used) = item.last_used_at {
        let age = now - used;
        bonus += if age < DAY_MS {
            24
        } else if age < 7 * DAY_MS {
            16
        } else if age < 30 * DAY_MS {
            8
        } else {
            0
        };
    }
    bonus
}

fn compare(sort: SortOrder, by_words: bool, a: &(u32, &Entry), b: &(u32, &Entry)) -> Ordering {
    let (ia, ib) = (&a.1.item, &b.1.item);
    let title = || {
        a.1.title_key
            .cmp(&b.1.title_key)
            .then_with(|| ia.id.cmp(&ib.id))
    };
    let recent = || ib.last_used_at.cmp(&ia.last_used_at);
    match sort {
        SortOrder::Relevance if by_words => b.0.cmp(&a.0).then_with(title),
        // Пустой запрос: группы из `group`; избранные и недавние — по времени
        // использования, остальные — по названию.
        SortOrder::Relevance => {
            b.0.cmp(&a.0)
                .then_with(|| if a.0 > 0 { recent() } else { Ordering::Equal })
                .then_with(title)
        }
        SortOrder::Title => title(),
        SortOrder::Updated => ib.updated_at.cmp(&ia.updated_at).then_with(title),
        SortOrder::Used => ib
            .use_count
            .cmp(&ia.use_count)
            .then_with(recent)
            .then_with(title),
    }
}

/// Ключ сортировки по названию, как в русском алфавитном списке: без учёта
/// регистра, `ё` как `е`, сначала цифры, потом кириллица, потом латиница.
fn title_key(title: &str) -> Vec<u32> {
    fold(title)
        .chars()
        .map(|c| {
            let code = u32::from(c);
            match c {
                'а'..='я' => 0x1_0000 + code - u32::from('а'),
                'a'..='z' => 0x2_0000 + code - u32::from('a'),
                // Пробелы, знаки и цифры — раньше букв, как есть.
                _ if code < 0x80 => code,
                _ => 0x3_0000 + code,
            }
        })
        .collect()
}

/// Нижний регистр и `ё` → `е`.
fn fold(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| if c == 'ё' { 'е' } else { c })
        .collect()
}

/// `ё` → `е` с сохранением регистра: нечёткий поиск сам не различает регистр.
fn fold_yo(text: &str) -> Cow<'_, str> {
    if text.contains(['ё', 'Ё']) {
        Cow::Owned(text.replace('ё', "е").replace('Ё', "Е"))
    } else {
        Cow::Borrowed(text)
    }
}

/// Одни и те же клавиши в английской и русской раскладке.
const LAYOUT_EN: &str = "qwertyuiop[]asdfghjkl;'zxcvbnm,.`";
const LAYOUT_RU: &str = "йцукенгшщзхъфывапролджэячсмитьбюё";

/// Слова в другой раскладке, если они целиком набраны в одной.
fn switch_layout(words: &[String]) -> Option<Vec<String>> {
    let text = words.concat();
    let latin = text.chars().any(|c| c.is_ascii_alphabetic());
    let cyrillic = text
        .chars()
        .any(|c| matches!(c, 'а'..='я' | 'А'..='Я' | 'ё' | 'Ё'));
    let (from, to) = match (latin, cyrillic) {
        (true, false) => (LAYOUT_EN, LAYOUT_RU),
        (false, true) => (LAYOUT_RU, LAYOUT_EN),
        _ => return None,
    };
    let map: HashMap<char, char> = from.chars().zip(to.chars()).collect();
    Some(
        words
            .iter()
            .map(|word| {
                word.chars()
                    .flat_map(char::to_lowercase)
                    .map(|c| map.get(&c).copied().unwrap_or(c))
                    .collect()
            })
            .collect(),
    )
}

/// Меняет `#from` в тексте заметки на `#to`, а без `to` убирает у тега `#`, чтобы
/// тег не вернулся при следующем сохранении. Код в обратных кавычках не трогает.
/// Возвращает `None`, если в тексте такого тега нет.
pub(crate) fn rewrite_hashtags(body: &str, from: &str, to: Option<&str>) -> Option<String> {
    let from = from.to_lowercase();
    let mut out = String::with_capacity(body.len());
    let mut changed = false;
    let mut fence = false;
    for line in body.split_inclusive('\n') {
        if line.trim_start().starts_with("```") {
            fence = !fence;
        }
        if fence || line.trim_start().starts_with("```") {
            out.push_str(line);
            continue;
        }
        let mut code = false;
        let mut prev: Option<char> = None;
        let mut pos = 0;
        while let Some(c) = line[pos..].chars().next() {
            if c == '`' {
                code = !code;
            }
            if c == '#' && !code && prev.is_none_or(|p| p.is_whitespace() || p == '(') {
                let start = pos + 1;
                let end = line[start..]
                    .char_indices()
                    .find(|&(_, ch)| !(ch.is_alphanumeric() || ch == '_' || ch == '-'))
                    .map_or(line.len(), |(i, _)| start + i);
                let tag = &line[start..end];
                if tag.chars().next().is_some_and(char::is_alphanumeric)
                    && tag.to_lowercase() == from
                {
                    match to {
                        Some(to) => {
                            out.push('#');
                            out.push_str(to);
                        }
                        None => out.push_str(tag),
                    }
                    changed = true;
                    prev = tag.chars().last();
                    pos = end;
                    continue;
                }
            }
            out.push(c);
            prev = Some(c);
            pos += c.len_utf8();
        }
    }
    changed.then_some(out)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_millis()).ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
