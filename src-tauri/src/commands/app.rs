//! Сведения о приложении.

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct AppInfo {
    name: &'static str,
    version: &'static str,
}

/// Название и версия приложения.
#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        name: "XonkyPass",
        version: env!("CARGO_PKG_VERSION"),
    }
}
