use super::*;
use crate::model::{LoginMeta, NoteMeta};

const NOW: i64 = 1_800_000_000_000;

fn summary(meta: ItemMeta) -> ItemSummary {
    ItemSummary {
        id: ItemId::generate(),
        meta,
        created_at: 0,
        updated_at: 0,
        deleted_at: None,
        revision: 1,
        use_count: 0,
        last_used_at: None,
    }
}

fn login(title: &str, username: &str, url: &str, tags: &[&str]) -> ItemSummary {
    summary(ItemMeta::Login(LoginMeta {
        title: title.into(),
        username: username.into(),
        urls: if url.is_empty() {
            vec![]
        } else {
            vec![url.into()]
        },
        tags: tags.iter().map(|&t| t.into()).collect(),
        ..Default::default()
    }))
}

fn note(title: &str, body: &str, tags: &[&str]) -> ItemSummary {
    summary(ItemMeta::Note(NoteMeta {
        title: title.into(),
        body: body.into(),
        tags: tags.iter().map(|&t| t.into()).collect(),
        favorite: false,
    }))
}

fn favorite(mut item: ItemSummary) -> ItemSummary {
    match &mut item.meta {
        ItemMeta::Login(m) => m.favorite = true,
        ItemMeta::Note(m) => m.favorite = true,
    }
    item
}

fn with_totp(mut item: ItemSummary) -> ItemSummary {
    if let ItemMeta::Login(m) = &mut item.meta {
        m.has_totp = true;
    }
    item
}

fn used(mut item: ItemSummary, count: i64, at: i64) -> ItemSummary {
    item.use_count = count;
    item.last_used_at = Some(at);
    item
}

fn all() -> SearchOptions {
    SearchOptions {
        sort: SortOrder::Relevance,
        limit: 0,
    }
}

fn sorted(sort: SortOrder) -> SearchOptions {
    SearchOptions { sort, limit: 0 }
}

/// Названия найденных записей по порядку.
fn titles(index: &SearchIndex, query: &str, options: &SearchOptions) -> Vec<String> {
    index
        .search_at(query, options, NOW)
        .items
        .iter()
        .map(|i| i.meta.title().to_owned())
        .collect()
}

fn find(index: &SearchIndex, query: &str) -> Vec<String> {
    titles(index, query, &all())
}

fn sample() -> SearchIndex {
    SearchIndex::new([
        login(
            "GitHub",
            "dev@example.com",
            "github.com",
            &["работа", "dev"],
        ),
        login("GitLab", "me@corp.ru", "gitlab.corp.ru", &["Работа"]),
        login("Почта", "ivan@mail.ru", "mail.ru", &["личное"]),
        login("Telegram", "+7 900", "", &[]),
        note(
            "Wi-Fi дома",
            "Сеть: Home\nПароль от роутера в #личное",
            &["личное"],
        ),
        note("Ёлка", "Купить игрушки", &[]),
    ])
}

// --- Разбор запроса ---

#[test]
fn query_parsing() {
    let q = Query::parse("  git  #Работа #dev is:login has:totp is:fav  ");
    assert_eq!(q.words, ["git"]);
    assert_eq!(q.tags, ["Работа", "dev"]);
    assert_eq!(q.kind, Some(ItemKind::Login));
    assert!(q.favorite && q.has_totp);

    assert_eq!(Query::parse("is:note").kind, Some(ItemKind::Note));
    assert!(Query::parse("IS:FAVORITE has:2fa").favorite);
    assert!(Query::parse("has:2fa").has_totp);
}

#[test]
fn unfinished_operators_are_ignored_and_unknown_ones_are_words() {
    // Пока оператор набирают, он ничего не отсекает.
    assert_eq!(Query::parse("is:"), Query::default());
    assert_eq!(Query::parse("is:no"), Query::default());
    assert_eq!(Query::parse("has:t"), Query::default());
    assert_eq!(Query::parse("#"), Query::default());
    // Неизвестное — обычное слово (например, адрес).
    assert_eq!(Query::parse("https://x.ru").words, ["https://x.ru"]);
    assert_eq!(Query::parse("is:card").words, ["is:card"]);
}

// --- Поиск по словам ---

#[test]
fn finds_by_every_field() {
    let index = sample();
    assert_eq!(find(&index, "github"), ["GitHub"]);
    // Логин, сайт, тег, текст заметки.
    assert_eq!(find(&index, "ivan"), ["Почта"]);
    assert_eq!(find(&index, "gitlab.corp"), ["GitLab"]);
    assert_eq!(find(&index, "роутера"), ["Wi-Fi дома"]);
    assert!(find(&index, "личное").contains(&"Почта".to_owned()));
}

#[test]
fn every_word_must_match() {
    let index = sample();
    assert_eq!(find(&index, "git lab"), ["GitLab"]);
    assert!(find(&index, "github почта").is_empty());
}

#[test]
fn fuzzy_and_case_insensitive() {
    let index = sample();
    assert_eq!(find(&index, "gthb"), ["GitHub"]);
    assert_eq!(find(&index, "ПОЧТА"), ["Почта"]);
    assert_eq!(find(&index, "telegrm"), ["Telegram"]);
}

#[test]
fn prefilter_keeps_diacritics_digits_and_symbols() {
    let index = SearchIndex::new([
        login("Café Ünïcode", "", "", &[]),
        login("1С: Бухгалтерия", "", "", &[]),
        login("Router", "admin", "192.168.0.1", &[]),
    ]);
    assert_eq!(find(&index, "cafe"), ["Café Ünïcode"]);
    assert_eq!(find(&index, "UNICODE"), ["Café Ünïcode"]);
    assert_eq!(find(&index, "1с:"), ["1С: Бухгалтерия"]);
    assert_eq!(find(&index, "168.0"), ["Router"]);
}

#[test]
fn yo_and_ye_are_the_same() {
    let index = sample();
    assert_eq!(find(&index, "елка"), ["Ёлка"]);
    assert_eq!(find(&index, "ёлка"), ["Ёлка"]);
    assert_eq!(find(&index, "ИГРУШКИ"), ["Ёлка"]);
}

#[test]
fn note_body_is_matched_as_substring_not_fuzzy() {
    let index = SearchIndex::new([note("A", "абв где жзи", &[])]);
    assert_eq!(find(&index, "где"), ["A"]);
    // По буквам вразброс текст заметки не ищется — иначе длинная заметка
    // находилась бы почти по любому запросу.
    assert!(find(&index, "агж").is_empty());
}

// --- Ранжирование ---

#[test]
fn title_beats_other_fields() {
    let index = SearchIndex::new([
        login("Банк", "github-fan", "", &[]),
        login("GitHub", "x", "", &[]),
    ]);
    assert_eq!(find(&index, "github"), ["GitHub", "Банк"]);
}

#[test]
fn prefix_and_contiguous_matches_rank_higher() {
    let index = SearchIndex::new([
        login("My Account In Linkedin", "", "", &[]),
        login("Mailbox", "", "", &[]),
    ]);
    assert_eq!(find(&index, "mail"), ["Mailbox", "My Account In Linkedin"]);
}

#[test]
fn favorites_and_usage_break_ties() {
    let index = SearchIndex::new([
        login("Shop A", "", "", &[]),
        favorite(login("Shop B", "", "", &[])),
        used(login("Shop C", "", "", &[]), 30, NOW - 1000),
    ]);
    let found = find(&index, "shop");
    assert_eq!(found[2], "Shop A");
    assert_eq!(found.len(), 3);
}

#[test]
fn empty_query_shows_favorites_then_recent() {
    let index = SearchIndex::new([
        login("Б", "", "", &[]),
        login("А", "", "", &[]),
        used(login("Старая", "", "", &[]), 1, NOW - 40 * DAY_MS),
        used(login("Свежая", "", "", &[]), 1, NOW - 1000),
        used(login("Вчерашняя", "", "", &[]), 1, NOW - DAY_MS),
        favorite(login("Избранная", "", "", &[])),
    ]);
    let result = index.search_at("", &all(), NOW);
    let found: Vec<_> = result.items.iter().map(|i| i.meta.title()).collect();
    // Давно использованная запись — среди остальных, по названию.
    assert_eq!(
        found,
        ["Избранная", "Свежая", "Вчерашняя", "А", "Б", "Старая"]
    );
    assert_eq!((result.favorites, result.recent), (1, 2));

    // При поиске по словам и другом порядке групп нет.
    let result = index.search_at("а", &all(), NOW);
    assert_eq!((result.favorites, result.recent), (0, 0));
    let result = index.search_at("", &sorted(SortOrder::Title), NOW);
    assert_eq!((result.favorites, result.recent), (0, 0));
}

#[test]
fn recent_section_is_short() {
    let index = SearchIndex::new(
        (0..8).map(|i| used(login(&format!("R{i}"), "", "", &[]), 1, NOW - i * 1000)),
    );
    let result = index.search_at("", &all(), NOW);
    assert_eq!(result.recent, RECENT_LIMIT);
    let found: Vec<_> = result.items.iter().map(|i| i.meta.title()).collect();
    assert_eq!(found, ["R0", "R1", "R2", "R3", "R4", "R5", "R6", "R7"]);
}

#[test]
fn titles_sort_like_a_russian_list() {
    let index = SearchIndex::new(
        ["zoom", "Ёж", "Банк", "1С", "apple", "елка", "(old)"].map(|t| login(t, "", "", &[])),
    );
    assert_eq!(
        titles(&index, "", &sorted(SortOrder::Title)),
        ["(old)", "1С", "Банк", "Ёж", "елка", "apple", "zoom"]
    );
}

#[test]
fn sort_orders() {
    let mut a = login("Альфа", "", "", &[]);
    a.updated_at = 1;
    let mut b = used(login("бета", "", "", &[]), 5, NOW);
    b.updated_at = 3;
    let mut c = used(login("Гамма", "", "", &[]), 9, NOW - DAY_MS);
    c.updated_at = 2;
    let index = SearchIndex::new([c, b, a]);

    assert_eq!(
        titles(&index, "", &sorted(SortOrder::Title)),
        ["Альфа", "бета", "Гамма"]
    );
    assert_eq!(
        titles(&index, "", &sorted(SortOrder::Updated)),
        ["бета", "Гамма", "Альфа"]
    );
    assert_eq!(
        titles(&index, "", &sorted(SortOrder::Used)),
        ["Гамма", "бета", "Альфа"]
    );
    // Порядок сортировки действует и при поиске по словам.
    assert_eq!(
        titles(&index, "а", &sorted(SortOrder::Title)),
        ["Альфа", "бета", "Гамма"]
    );
}

#[test]
fn limit_keeps_the_best_and_reports_total() {
    let mut items: Vec<ItemSummary> = (0..50)
        .map(|i| login(&format!("Item {i:02}"), "", "", &[]))
        .collect();
    items.push(login("Item", "", "", &[]));
    let index = SearchIndex::new(items);

    let options = SearchOptions {
        sort: SortOrder::Title,
        limit: 3,
    };
    let result = index.search_at("item", &options, NOW);
    assert_eq!(result.total, 51);
    let found: Vec<_> = result.items.iter().map(|i| i.meta.title()).collect();
    assert_eq!(found, ["Item", "Item 00", "Item 01"]);
}

// --- Теги и фильтры ---

#[test]
fn tags_are_matched_exactly_and_all_are_required() {
    let index = sample();
    // Без учёта регистра: «Работа» и «работа» — один тег.
    assert_eq!(find(&index, "#работа").len(), 2);
    assert_eq!(find(&index, "#работа #dev"), ["GitHub"]);
    assert_eq!(
        find(&index, "#работа git"),
        ["GitHub", "GitLab"].map(String::from)
    );
    assert!(find(&index, "#dev #личное").is_empty());
}

#[test]
fn unknown_tag_matches_by_prefix_while_typing() {
    let index = SearchIndex::new([
        login("A", "", "", &["work"]),
        login("B", "", "", &["workshop"]),
        login("C", "", "", &["home"]),
    ]);
    // Тег «work» есть — только он, без «workshop».
    assert_eq!(find(&index, "#work"), ["A"]);
    // Тега «wor» нет — подходят все, что так начинаются.
    assert_eq!(find(&index, "#wor"), ["A", "B"]);
    assert!(find(&index, "#nothing").is_empty());
}

#[test]
fn kind_favorite_and_totp_filters() {
    let index = SearchIndex::new([
        with_totp(login("A", "", "", &[])),
        favorite(login("B", "", "", &[])),
        favorite(note("C", "текст", &[])),
    ]);
    assert_eq!(find(&index, "is:login"), ["B", "A"]);
    assert_eq!(find(&index, "is:note"), ["C"]);
    assert_eq!(find(&index, "is:fav"), ["B", "C"]);
    assert_eq!(find(&index, "is:fav is:note"), ["C"]);
    assert_eq!(find(&index, "has:totp"), ["A"]);
}

// --- Раскладка ---

#[test]
fn wrong_keyboard_layout_is_fixed() {
    let index = sample();
    let result = index.search_at("пшерги", &all(), NOW);
    assert_eq!(result.layout_fixed.as_deref(), Some("github"));
    assert_eq!(result.items[0].meta.title(), "GitHub");

    let result = index.search_at("gjxnf", &all(), NOW);
    assert_eq!(result.layout_fixed.as_deref(), Some("почта"));
    assert_eq!(result.items[0].meta.title(), "Почта");

    // Найдено как есть — раскладка не меняется.
    assert_eq!(index.search_at("github", &all(), NOW).layout_fixed, None);
    // Не нашлось и в другой раскладке.
    let result = index.search_at("zzzz", &all(), NOW);
    assert_eq!((result.total, result.layout_fixed), (0, None));
}

#[test]
fn layout_switch_maps_whole_keyboard() {
    let words = |s: &str| switch_layout(&[s.to_owned()]).unwrap();
    // `,` — это «б» в русской раскладке.
    assert_eq!(words("hf,jnf"), ["работа"]);
    assert_eq!(words("ЗФЫЫ"), ["pass"]);
    assert_eq!(words("ghbdtn"), ["привет"]);
    // Смесь раскладок или одни цифры — не меняем.
    assert_eq!(switch_layout(&["gitлаб".to_owned()]), None);
    assert_eq!(switch_layout(&["2024".to_owned()]), None);
}

// --- Индекс ---

#[test]
fn index_updates_and_counts_tags() {
    let mut a = login("A", "", "", &["Работа", "dev"]);
    let b = login("B", "", "", &["работа"]);
    let mut trashed = login("T", "", "", &["корзина"]);
    trashed.deleted_at = Some(1);
    let mut index = SearchIndex::new([a.clone(), b.clone(), trashed]);

    assert_eq!(index.len(), 2);
    let tags: Vec<_> = index
        .tags()
        .iter()
        .map(|t| (t.name.as_str(), t.count))
        .collect();
    assert_eq!(tags, [("Работа", 2), ("dev", 1)]);

    // Изменение записи.
    if let ItemMeta::Login(m) = &mut a.meta {
        m.title = "A2".into();
        m.tags = vec!["dev".into()];
    }
    index.upsert(a.clone());
    assert_eq!(find(&index, "a2"), ["A2"]);
    let tags: Vec<_> = index
        .tags()
        .iter()
        .map(|t| (t.name.as_str(), t.count))
        .collect();
    assert_eq!(tags, [("dev", 1), ("работа", 1)]);

    // Использование меняет порядок пустого запроса.
    index.record_use(b.id, NOW);
    assert_eq!(find(&index, ""), ["B", "A2"]);

    // В корзину — из индекса.
    a.deleted_at = Some(2);
    index.upsert(a);
    assert_eq!(find(&index, ""), ["B"]);
    index.remove(b.id);
    assert!(index.is_empty());
    assert!(index.tags().is_empty());
}

// --- Теги в тексте заметок ---

#[test]
fn hashtags_are_renamed_in_text() {
    let body = "#Работа и (#работа), но не #работает и не a#работа\n# Работа\n";
    assert_eq!(
        rewrite_hashtags(body, "работа", Some("job")).as_deref(),
        Some("#job и (#job), но не #работает и не a#работа\n# Работа\n")
    );
    assert_eq!(
        rewrite_hashtags(body, "работа", None).as_deref(),
        Some("Работа и (работа), но не #работает и не a#работа\n# Работа\n")
    );
    assert_eq!(rewrite_hashtags(body, "дом", Some("x")), None);
}

#[test]
fn hashtags_in_code_are_kept() {
    let body = "`#tag` и #tag\n```\n#tag\n```\n#tag";
    assert_eq!(
        rewrite_hashtags(body, "tag", Some("new")).as_deref(),
        Some("`#tag` и #new\n```\n#tag\n```\n#new")
    );
}
