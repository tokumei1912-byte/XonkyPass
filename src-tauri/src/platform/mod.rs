//! Код, зависящий от Windows (Win32 API).
//!
//! На других платформах — пустые заглушки, чтобы проект собирался и проверялся
//! в Linux (CI, разработка). Работает приложение только на Windows.

#[cfg(windows)]
mod window_style;

#[cfg(windows)]
pub use window_style::hide_from_alt_tab;

#[cfg(not(windows))]
pub fn hide_from_alt_tab(_window: &tauri::WebviewWindow) {}
