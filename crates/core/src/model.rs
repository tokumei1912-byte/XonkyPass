//! Записи хранилища: логины и заметки.
//!
//! Запись состоит из двух частей, каждая шифруется отдельно:
//! - [`ItemMeta`] — то, что нужно для списка и поиска (название, логин, сайты, теги);
//! - [`ItemSecret`] — то, что расшифровывается только по явному действию (пароль, 2FA).

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::error::{Error, Invalid, Result};

/// Версия JSON-формата содержимого записи (поле `v` в метаданных).
/// Описывает и метаданные, и секретную часть: они всегда пишутся вместе.
pub(crate) const CONTENT_VERSION: u32 = 1;

/// Ограничения на размер полей (в символах и штуках).
pub mod limits {
    pub const TITLE: usize = 256;
    pub const USERNAME: usize = 512;
    pub const URL: usize = 2048;
    pub const URLS: usize = 32;
    pub const APP: usize = 260;
    pub const APPS: usize = 32;
    pub const TAG: usize = 64;
    pub const TAGS: usize = 32;
    pub const BODY: usize = 1_000_000;
    pub const PASSWORD: usize = 4096;
    pub const TOTP: usize = 2048;
    pub const NOTES: usize = 100_000;
    pub const FIELDS: usize = 64;
    pub const FIELD_NAME: usize = 256;
    pub const FIELD_VALUE: usize = 10_000;
    /// Длина названия заметки, которое берётся из первой строки текста.
    pub const DERIVED_TITLE: usize = 80;
}

/// Идентификатор записи: UUIDv7 (упорядочен по времени создания).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ItemId(Uuid);

impl ItemId {
    pub(crate) fn generate() -> Self {
        ItemId(Uuid::now_v7())
    }

    pub(crate) fn from_bytes(bytes: [u8; 16]) -> Self {
        ItemId(Uuid::from_bytes(bytes))
    }

    pub(crate) fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }
}

impl fmt::Display for ItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0.hyphenated(), f)
    }
}

impl FromStr for ItemId {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Uuid::parse_str(s)
            .map(ItemId)
            .map_err(|_| Error::ItemNotFound)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemKind {
    Login,
    Note,
}

/// Открытая для поиска часть записи.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ItemMeta {
    Login(LoginMeta),
    Note(NoteMeta),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LoginMeta {
    pub title: String,
    pub username: String,
    /// Сайты: домены или адреса, как их ввёл пользователь.
    pub urls: Vec<String>,
    /// Приложения: имена exe.
    pub apps: Vec<String>,
    pub tags: Vec<String>,
    pub favorite: bool,
    /// Есть ли код 2FA. Хранилище заполняет его само по секретной части.
    pub has_totp: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NoteMeta {
    pub title: String,
    /// Текст в Markdown. Лежит в метаданных, чтобы по нему работал поиск.
    pub body: String,
    pub tags: Vec<String>,
    pub favorite: bool,
}

/// Секретная часть записи. Затирается при удалении, не печатается в журналы.
// `default` на каждом поле, а не на структуре: у структуры с `Drop` serde не может
// забрать поля из значения по умолчанию.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct ItemSecret {
    #[serde(default)]
    pub password: String,
    /// Ссылка `otpauth://…` или секрет 2FA; пустая строка — нет 2FA.
    #[serde(default)]
    pub totp: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub fields: Vec<CustomField>,
}

/// Дополнительное поле (ПИН, секретный вопрос и т. п.).
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct CustomField {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub hidden: bool,
}

impl ItemMeta {
    pub fn kind(&self) -> ItemKind {
        match self {
            ItemMeta::Login(_) => ItemKind::Login,
            ItemMeta::Note(_) => ItemKind::Note,
        }
    }

    pub fn title(&self) -> &str {
        match self {
            ItemMeta::Login(m) => &m.title,
            ItemMeta::Note(m) => &m.title,
        }
    }

    pub fn tags(&self) -> &[String] {
        match self {
            ItemMeta::Login(m) => &m.tags,
            ItemMeta::Note(m) => &m.tags,
        }
    }

    pub fn favorite(&self) -> bool {
        match self {
            ItemMeta::Login(m) => m.favorite,
            ItemMeta::Note(m) => m.favorite,
        }
    }

    /// Приводит поля к единому виду и проверяет ограничения.
    pub(crate) fn normalize(&mut self) -> Result<(), Invalid> {
        match self {
            ItemMeta::Login(m) => m.normalize(),
            ItemMeta::Note(m) => m.normalize(),
        }
    }
}

impl LoginMeta {
    fn normalize(&mut self) -> Result<(), Invalid> {
        self.urls = normalize_list(&self.urls, "urls", limits::URL, limits::URLS)?;
        self.apps = normalize_list(&self.apps, "apps", limits::APP, limits::APPS)?;
        self.tags = normalize_tags(&self.tags)?;
        self.username = self.username.trim().to_owned();
        check_len("username", &self.username, limits::USERNAME)?;

        self.title = self.title.trim().to_owned();
        if self.title.is_empty() {
            // Запись, сохранённая с сайта или из приложения, называется по нему.
            self.title = self
                .urls
                .first()
                .or(self.apps.first())
                .cloned()
                .ok_or(Invalid::EmptyTitle)?;
        }
        check_len("title", &self.title, limits::TITLE)
    }
}

impl NoteMeta {
    fn normalize(&mut self) -> Result<(), Invalid> {
        self.tags = normalize_tags(&self.tags)?;
        check_len("body", &self.body, limits::BODY)?;

        self.title = self.title.trim().to_owned();
        if self.title.is_empty() {
            // Быстрая заметка без названия называется по первой строке текста.
            let line = self
                .body
                .lines()
                .map(|line| line.trim().trim_start_matches('#').trim())
                .find(|line| !line.is_empty())
                .ok_or(Invalid::EmptyNote)?;
            self.title = line.chars().take(limits::DERIVED_TITLE).collect();
        }
        check_len("title", &self.title, limits::TITLE)
    }
}

impl ItemSecret {
    /// Секретная часть с одним паролем. Синтаксис `..Default::default()` для этого
    /// типа недоступен: он затирается при удалении (`ZeroizeOnDrop`).
    pub fn with_password(password: impl Into<String>) -> Self {
        let mut secret = Self::default();
        secret.password = password.into();
        secret
    }

    pub(crate) fn normalize(&mut self) -> Result<(), Invalid> {
        check_len("password", &self.password, limits::PASSWORD)?;
        check_len("notes", &self.notes, limits::NOTES)?;

        let totp = self.totp.trim();
        if totp.len() != self.totp.len() {
            self.totp = totp.to_owned();
        }
        check_len("totp", &self.totp, limits::TOTP)?;

        self.fields
            .retain(|f| !f.name.trim().is_empty() || !f.value.is_empty());
        if self.fields.len() > limits::FIELDS {
            return Err(Invalid::TooMany {
                field: "fields",
                max: limits::FIELDS,
            });
        }
        for field in &mut self.fields {
            let name = field.name.trim();
            if name.is_empty() {
                return Err(Invalid::EmptyFieldName);
            }
            if name.len() != field.name.len() {
                field.name = name.to_owned();
            }
            check_len("field_name", &field.name, limits::FIELD_NAME)?;
            check_len("field_value", &field.value, limits::FIELD_VALUE)?;
        }
        Ok(())
    }
}

impl fmt::Debug for ItemSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ItemSecret")
            .field("password", &Redacted(&self.password))
            .field("totp", &Redacted(&self.totp))
            .field("notes", &Redacted(&self.notes))
            .field("fields", &self.fields)
            .finish()
    }
}

impl fmt::Debug for CustomField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CustomField")
            .field("name", &self.name)
            .field("value", &Redacted(&self.value))
            .field("hidden", &self.hidden)
            .finish()
    }
}

/// В отладочном выводе показывает только, пустое ли поле.
struct Redacted<'a>(&'a str);

impl fmt::Debug for Redacted<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.0.is_empty() { "\"\"" } else { "\"***\"" })
    }
}

/// Тег в едином виде: без `#` в начале, пробелы заменены на `-`.
/// Регистр сохраняется; сравниваются теги без учёта регистра.
pub fn normalize_tag(tag: &str) -> String {
    tag.trim()
        .trim_start_matches('#')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
}

/// Тег, введённый отдельно от записи: например, новое имя при переименовании.
pub(crate) fn parse_tag(tag: &str) -> Result<String, Invalid> {
    let tag = normalize_tag(tag);
    if tag.is_empty() {
        return Err(Invalid::EmptyTag);
    }
    check_len("tags", &tag, limits::TAG)?;
    Ok(tag)
}

fn normalize_tags(tags: &[String]) -> Result<Vec<String>, Invalid> {
    let tags: Vec<String> = tags.iter().map(|t| normalize_tag(t)).collect();
    normalize_list(&tags, "tags", limits::TAG, limits::TAGS)
}

/// Убирает пробелы по краям, пустые значения и повторы (без учёта регистра).
fn normalize_list(
    values: &[String],
    field: &'static str,
    max_len: usize,
    max_count: usize,
) -> Result<Vec<String>, Invalid> {
    let mut out: Vec<String> = Vec::with_capacity(values.len());
    let mut seen: Vec<String> = Vec::with_capacity(values.len());
    for value in values {
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        let folded = value.to_lowercase();
        if seen.contains(&folded) {
            continue;
        }
        check_len(field, value, max_len)?;
        seen.push(folded);
        out.push(value.to_owned());
    }
    if out.len() > max_count {
        return Err(Invalid::TooMany {
            field,
            max: max_count,
        });
    }
    Ok(out)
}

fn check_len(field: &'static str, value: &str, max: usize) -> Result<(), Invalid> {
    // Быстрая проверка по байтам: символов не больше, чем байтов.
    if value.len() > max && value.chars().count() > max {
        return Err(Invalid::TooLong { field, max });
    }
    Ok(())
}

/// Метаданные в том виде, в каком они шифруются: `{"v":1,"kind":…,…}`.
#[derive(Serialize)]
struct StoredMetaRef<'a> {
    v: u32,
    #[serde(flatten)]
    meta: &'a ItemMeta,
}

#[derive(Deserialize)]
struct StoredMeta {
    v: u32,
    #[serde(flatten)]
    meta: ItemMeta,
}

pub(crate) fn encode_meta(meta: &ItemMeta) -> Zeroizing<Vec<u8>> {
    let stored = StoredMetaRef {
        v: CONTENT_VERSION,
        meta,
    };
    Zeroizing::new(serde_json::to_vec(&stored).expect("метаданные сериализуются в JSON"))
}

pub(crate) fn decode_meta(bytes: &[u8]) -> Result<ItemMeta> {
    let stored: StoredMeta = serde_json::from_slice(bytes)
        .map_err(|err| Error::Corrupted(format!("метаданные записи: {err}")))?;
    if stored.v != CONTENT_VERSION {
        return Err(Error::UnsupportedVersion);
    }
    Ok(stored.meta)
}

pub(crate) fn encode_secret(secret: &ItemSecret) -> Zeroizing<Vec<u8>> {
    Zeroizing::new(serde_json::to_vec(secret).expect("секреты сериализуются в JSON"))
}

pub(crate) fn decode_secret(bytes: &[u8]) -> Result<ItemSecret> {
    serde_json::from_slice(bytes)
        .map_err(|err| Error::Corrupted(format!("секретная часть записи: {err}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn login(title: &str) -> LoginMeta {
        LoginMeta {
            title: title.into(),
            ..Default::default()
        }
    }

    #[test]
    fn meta_json_matches_plan() {
        let meta = ItemMeta::Login(LoginMeta {
            title: "GitHub".into(),
            username: "user@example.com".into(),
            urls: vec!["github.com".into()],
            apps: vec!["GitHubDesktop.exe".into()],
            tags: vec!["работа".into(), "dev".into()],
            favorite: true,
            has_totp: true,
        });
        let json: serde_json::Value = serde_json::from_slice(&encode_meta(&meta)).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "v": 1,
                "kind": "login",
                "title": "GitHub",
                "username": "user@example.com",
                "urls": ["github.com"],
                "apps": ["GitHubDesktop.exe"],
                "tags": ["работа", "dev"],
                "favorite": true,
                "has_totp": true
            })
        );
        assert_eq!(decode_meta(&encode_meta(&meta)).unwrap(), meta);
    }

    #[test]
    fn note_json_roundtrip() {
        let meta = ItemMeta::Note(NoteMeta {
            title: "Wi-Fi дома".into(),
            body: "# Пароль\nsecret".into(),
            tags: vec!["дом".into()],
            favorite: false,
        });
        let json: serde_json::Value = serde_json::from_slice(&encode_meta(&meta)).unwrap();
        assert_eq!(json["kind"], "note");
        assert_eq!(json["v"], 1);
        assert_eq!(decode_meta(&encode_meta(&meta)).unwrap(), meta);
    }

    #[test]
    fn decode_tolerates_missing_and_unknown_fields() {
        let meta = decode_meta(br#"{"v":1,"kind":"login","title":"X","future":42}"#).unwrap();
        assert_eq!(meta, ItemMeta::Login(login("X")));
    }

    #[test]
    fn decode_rejects_other_versions_and_garbage() {
        assert!(matches!(
            decode_meta(br#"{"v":2,"kind":"login","title":"X"}"#),
            Err(Error::UnsupportedVersion)
        ));
        assert!(matches!(
            decode_meta(br#"{"v":1,"kind":"card","title":"X"}"#),
            Err(Error::Corrupted(_))
        ));
        assert!(matches!(decode_meta(b"\xff"), Err(Error::Corrupted(_))));
        assert!(matches!(
            decode_secret(br#"{"password":5}"#),
            Err(Error::Corrupted(_))
        ));
    }

    fn field(name: &str, value: &str, hidden: bool) -> CustomField {
        CustomField {
            name: name.into(),
            value: value.into(),
            hidden,
        }
    }

    #[test]
    fn secret_roundtrip() {
        let secret = ItemSecret {
            password: "p@ss".into(),
            totp: "otpauth://totp/x?secret=ABC".into(),
            notes: "n".into(),
            fields: vec![field("PIN", "1234", true)],
        };
        assert_eq!(decode_secret(&encode_secret(&secret)).unwrap(), secret);
        // Отсутствующие поля — пустые.
        assert_eq!(decode_secret(b"{}").unwrap(), ItemSecret::default());
    }

    #[test]
    fn secret_debug_is_redacted() {
        let mut secret = ItemSecret::default();
        secret.password = "hunter2".into();
        secret.fields = vec![field("PIN", "1234", true)];
        let debug = format!("{secret:?}");
        assert!(!debug.contains("hunter2"));
        assert!(!debug.contains("1234"));
        assert!(debug.contains("PIN"));
    }

    #[test]
    fn tags_are_normalized() {
        assert_eq!(normalize_tag("  #работа  "), "работа");
        assert_eq!(normalize_tag("мой  проект\tX"), "мой-проект-X");
        assert_eq!(normalize_tag("###"), "");

        let mut meta = ItemMeta::Login(LoginMeta {
            tags: vec![
                "Работа".into(),
                "#работа".into(),
                " ".into(),
                "dev ops".into(),
            ],
            ..login("X")
        });
        meta.normalize().unwrap();
        assert_eq!(meta.tags(), ["Работа", "dev-ops"]);
    }

    #[test]
    fn login_title_falls_back_to_site_or_app() {
        let mut meta = LoginMeta {
            urls: vec!["  ".into(), " github.com ".into()],
            ..login("  ")
        };
        meta.normalize().unwrap();
        assert_eq!(meta.title, "github.com");
        assert_eq!(meta.urls, ["github.com"]);

        let mut meta = LoginMeta {
            apps: vec!["Telegram.exe".into()],
            ..login("")
        };
        meta.normalize().unwrap();
        assert_eq!(meta.title, "Telegram.exe");

        assert_eq!(login(" ").normalize(), Err(Invalid::EmptyTitle));
    }

    #[test]
    fn note_title_falls_back_to_first_line() {
        let mut meta = NoteMeta {
            body: "\n\n## Покупки  \n- молоко".into(),
            ..Default::default()
        };
        meta.normalize().unwrap();
        assert_eq!(meta.title, "Покупки");

        let mut long = NoteMeta {
            body: "я".repeat(500),
            ..Default::default()
        };
        long.normalize().unwrap();
        assert_eq!(long.title.chars().count(), limits::DERIVED_TITLE);

        let mut empty = NoteMeta {
            body: " \n # \n".into(),
            ..Default::default()
        };
        assert_eq!(empty.normalize(), Err(Invalid::EmptyNote));
    }

    #[test]
    fn lists_are_deduplicated_and_limited() {
        let mut meta = LoginMeta {
            urls: vec!["GitHub.com".into(), "github.com".into()],
            ..login("X")
        };
        meta.normalize().unwrap();
        assert_eq!(meta.urls, ["GitHub.com"]);

        let mut meta = LoginMeta {
            tags: (0..=limits::TAGS).map(|i| format!("t{i}")).collect(),
            ..login("X")
        };
        assert_eq!(
            meta.normalize(),
            Err(Invalid::TooMany {
                field: "tags",
                max: limits::TAGS
            })
        );
    }

    #[test]
    fn length_limits_count_characters() {
        // Кириллица — 2 байта на символ, но ограничение в символах.
        let mut ok = login(&"ж".repeat(limits::TITLE));
        assert!(ok.normalize().is_ok());
        let mut too_long = login(&"ж".repeat(limits::TITLE + 1));
        assert_eq!(
            too_long.normalize(),
            Err(Invalid::TooLong {
                field: "title",
                max: limits::TITLE
            })
        );
    }

    #[test]
    fn secret_normalization() {
        let mut secret = ItemSecret {
            password: "  spaces are kept  ".into(),
            totp: "  otpauth://x  ".into(),
            notes: String::new(),
            fields: vec![CustomField::default(), field(" PIN ", "1", true)],
        };
        secret.normalize().unwrap();
        assert_eq!(secret.password, "  spaces are kept  ");
        assert_eq!(secret.totp, "otpauth://x");
        assert_eq!(secret.fields.len(), 1);
        assert_eq!(secret.fields[0].name, "PIN");

        let mut nameless = ItemSecret::default();
        nameless.fields = vec![field(" ", "x", false)];
        assert_eq!(nameless.normalize(), Err(Invalid::EmptyFieldName));
    }

    #[test]
    fn item_id_text_roundtrip() {
        let id = ItemId::generate();
        assert_eq!(id.to_string().parse::<ItemId>().unwrap(), id);
        assert!("nope".parse::<ItemId>().is_err());
        assert_eq!(serde_json::to_string(&id).unwrap(), format!("\"{id}\""));
    }

    #[test]
    fn item_ids_are_time_ordered() {
        let a = ItemId::generate();
        let b = ItemId::generate();
        assert!(a < b);
    }
}
