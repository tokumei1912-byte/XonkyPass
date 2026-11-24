//! Код, зависящий от Windows (Win32 API).
//!
//! На других платформах — пустые заглушки, чтобы проект собирался и проверялся
//! в Linux (CI, разработка). Работает приложение только на Windows.

use std::fmt;

use serde::Serialize;

#[cfg(windows)]
pub mod clipboard;
#[cfg(not(windows))]
#[path = "clipboard_stub.rs"]
pub mod clipboard;

#[cfg(windows)]
pub mod appearance;
#[cfg(not(windows))]
mod desktop_stub;
#[cfg(windows)]
pub mod dialog;
#[cfg(windows)]
pub mod shell;
#[cfg(not(windows))]
pub use desktop_stub::{appearance, dialog, shell};

#[cfg(windows)]
mod window_style;

/// Цвета акцента Windows для светлой и тёмной темы (`#rrggbb`): на светлом фоне
/// темнее, на тёмном светлее.
#[derive(Debug, Clone, Serialize)]
pub struct Accent {
    pub light: String,
    pub dark: String,
}

#[cfg(windows)]
pub use window_style::hide_from_alt_tab;

#[cfg(not(windows))]
pub fn hide_from_alt_tab(_window: &tauri::WebviewWindow) {}

/// HWND окна как число (0 — нет), чтобы его можно было хранить и передавать между потоками.
pub fn window_handle(window: &tauri::WebviewWindow) -> isize {
    #[cfg(windows)]
    {
        match window.hwnd() {
            Ok(hwnd) => hwnd.0 as isize,
            Err(err) => {
                tracing::warn!(label = window.label(), %err, "нет HWND окна");
                0
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        0
    }
}

pub type PlatformResult<T> = Result<T, PlatformError>;

/// Ошибка системного вызова: текст для журнала и интерфейса.
#[derive(Debug)]
pub struct PlatformError(String);

impl PlatformError {
    #[cfg(not(windows))]
    pub fn unsupported() -> Self {
        Self("доступно только в Windows".into())
    }
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(windows)]
impl From<windows::core::Error> for PlatformError {
    fn from(err: windows::core::Error) -> Self {
        Self(err.to_string())
    }
}
