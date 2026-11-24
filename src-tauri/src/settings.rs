//! Настройки приложения: `%APPDATA%\XonkyPass\settings.json`.
//!
//! Секретов здесь нет, поэтому файл не шифруется. Неизвестные поля пропускаются,
//! отсутствующие берутся по умолчанию, испорченный файл заменяется настройками
//! по умолчанию (с записью в журнал).

use std::io;
use std::ops::RangeInclusive;
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
    /// Панель у края экрана.
    pub dock: DockSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            clipboard_clear_secs: 20,
            theme: Theme::System,
            dock: DockSettings::default(),
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

/// Где живёт панель: у края экрана или обычным окном.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DockMode {
    #[default]
    Left,
    Right,
    Top,
    /// Обычное окно: в панели задач и Alt+Tab, не поверх всех окон, само не прячется.
    Window,
}

impl DockMode {
    pub fn docked(self) -> bool {
        self != Self::Window
    }
}

/// Положение панели в режиме окна: угол — в физических пикселях экрана, размер — в
/// логических (не зависит от масштаба монитора).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowPlacement {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DockSettings {
    pub mode: DockMode,
    /// Монитор (`\\.\DISPLAY1`); `None` — основной.
    pub monitor: Option<String>,
    /// Ширина панели слева или справа, логические пиксели.
    pub side_width: u32,
    /// Размер панели сверху, логические пиксели.
    pub top_width: u32,
    pub top_height: u32,
    /// Сколько курсор держится у края, прежде чем панель выедет; `None` — панель
    /// не выезжает от наведения, только из трея.
    pub hover_delay_ms: Option<u32>,
    pub animate: bool,
    /// Тонкая полоска на краю, откуда выезжает панель.
    pub indicator: bool,
    /// Последнее положение в режиме окна.
    pub window: Option<WindowPlacement>,
}

impl Default for DockSettings {
    fn default() -> Self {
        Self {
            mode: DockMode::Left,
            monitor: None,
            side_width: 400,
            top_width: 720,
            top_height: 520,
            hover_delay_ms: Some(200),
            animate: true,
            indicator: false,
            window: None,
        }
    }
}

pub const SIDE_WIDTH: RangeInclusive<u32> = 320..=640;
pub const TOP_WIDTH: RangeInclusive<u32> = 480..=1600;
pub const TOP_HEIGHT: RangeInclusive<u32> = 360..=1200;
pub const HOVER_DELAY_MS: RangeInclusive<u32> = 50..=1000;
pub const WINDOW_WIDTH: RangeInclusive<u32> = 320..=4000;
pub const WINDOW_HEIGHT: RangeInclusive<u32> = 400..=4000;

fn clamp(value: u32, range: &RangeInclusive<u32>) -> u32 {
    value.clamp(*range.start(), *range.end())
}

impl DockSettings {
    /// Значения в допустимых пределах: опечатка в файле не должна ломать панель.
    pub fn clamped(mut self) -> Self {
        self.side_width = clamp(self.side_width, &SIDE_WIDTH);
        self.top_width = clamp(self.top_width, &TOP_WIDTH);
        self.top_height = clamp(self.top_height, &TOP_HEIGHT);
        self.hover_delay_ms = self.hover_delay_ms.map(|v| clamp(v, &HOVER_DELAY_MS));
        if let Some(window) = &mut self.window {
            window.width = clamp(window.width, &WINDOW_WIDTH);
            window.height = clamp(window.height, &WINDOW_HEIGHT);
        }
        self
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), CommandError> {
        if self.clipboard_clear_secs > MAX_CLIPBOARD_CLEAR_SECS {
            return Err(CommandError::new(
                "invalid",
                format!("очистка буфера — не дольше {MAX_CLIPBOARD_CLEAR_SECS} секунд"),
            ));
        }
        if self.dock.clone().clamped() != self.dock {
            return Err(CommandError::new(
                "invalid",
                "размер панели или задержка вне допустимых пределов",
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
    let parsed = serde_json::from_str::<Settings>(&text).map(|mut settings| {
        settings.dock = settings.dock.clamped();
        settings
    });
    match parsed {
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
            dock: DockSettings {
                mode: DockMode::Top,
                monitor: Some(r"\\.\DISPLAY2".into()),
                hover_delay_ms: None,
                window: Some(WindowPlacement {
                    x: -1200,
                    y: 40,
                    width: 420,
                    height: 700,
                }),
                ..DockSettings::default()
            },
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

    #[test]
    fn dock_values_from_file_are_clamped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let json =
            r#"{"theme":"dark","dock":{"mode":"right","side_width":5000,"hover_delay_ms":1}}"#;
        std::fs::write(&path, json).unwrap();
        let loaded = load(&path);
        assert_eq!(loaded.theme, Theme::Dark);
        assert_eq!(loaded.dock.mode, DockMode::Right);
        assert_eq!(loaded.dock.side_width, *SIDE_WIDTH.end());
        assert_eq!(loaded.dock.hover_delay_ms, Some(*HOVER_DELAY_MS.start()));
    }

    #[test]
    fn out_of_range_dock_is_rejected_from_the_interface() {
        let mut settings = Settings::default();
        assert!(settings.validate().is_ok());
        settings.dock.top_height = 10;
        assert_eq!(settings.validate().unwrap_err().code(), "invalid");
    }
}
