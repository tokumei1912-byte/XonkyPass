//! Пути к файлам приложения.

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

/// Имя папки приложения в `%APPDATA%` и `%LOCALAPPDATA%`.
const APP_DIR: &str = "XonkyPass";

/// Папка журналов: `%LOCALAPPDATA%\XonkyPass\logs`.
pub fn log_dir(app: &AppHandle) -> tauri::Result<PathBuf> {
    Ok(app.path().local_data_dir()?.join(APP_DIR).join("logs"))
}
