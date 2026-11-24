//! Настройки приложения: `%APPDATA%\XonkyPass\settings.json`.
//!
//! Секретов здесь нет, поэтому файл не шифруется. Неизвестные поля пропускаются,
//! отсутствующие берутся по умолчанию, испорченный файл заменяется настройками
//! по умолчанию (с записью в журнал).

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::CommandError;

/// Дольше держать пароль в буфере смысла нет, а опечатка в файле не должна
/// оставить его там на сутки.
pub const MAX_CLIPBOARD_CLEAR_SECS: u32 = 600;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Через сколько секунд очищать буфер после копирования; 0 — не очищать.
    pub clipboard_clear_secs: u32,
    pub theme: Theme,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            clipboard_clear_secs: 20,
            theme: Theme::System,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

impl Settings {
    pub fn validate(&self) -> Result<(), CommandError> {
        if self.clipboard_clear_secs > MAX_CLIPBOARD_CLEAR_SECS {
            return Err(CommandError::new(
                "invalid",
                format!("очистка буфера — не дольше {MAX_CLIPBOARD_CLEAR_SECS} секунд"),
            ));
        }
        Ok(())
    }
}

pub fn load(path: &Path) -> Settings {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Settings::default(),
        Err(err) => {
            tracing::warn!(%err, "не удалось прочитать настройки, беру по умолчанию");
            return Settings::default();
        }
    };
    match serde_json::from_str::<Settings>(&text) {
        Ok(settings) if settings.validate().is_ok() => settings,
        Ok(_) => {
            tracing::warn!("недопустимые значения в настройках, беру по умолчанию");
            Settings::default()
        }
        Err(err) => {
            tracing::warn!(%err, "испорченный файл настроек, беру по умолчанию");
            Settings::default()
        }
    }
}

/// Записывает настройки через временный файл, чтобы сбой не оставил файл пустым.
pub fn save(path: &Path, settings: &Settings) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load(&dir.path().join("settings.json")), Settings::default());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("settings.json");
        let settings = Settings {
            clipboard_clear_secs: 45,
            theme: Theme::Dark,
        };
        save(&path, &settings).unwrap();
        assert_eq!(load(&path), settings);
    }

    #[test]
    fn partial_and_unknown_fields_are_tolerated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"theme":"light","future":1}"#).unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.theme, Theme::Light);
        assert_eq!(loaded.clipboard_clear_secs, 20);
    }

    #[test]
    fn broken_or_out_of_range_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{не json").unwrap();
        assert_eq!(load(&path), Settings::default());
        std::fs::write(&path, r#"{"clipboard_clear_secs":86400}"#).unwrap();
        assert_eq!(load(&path), Settings::default());
    }
}
