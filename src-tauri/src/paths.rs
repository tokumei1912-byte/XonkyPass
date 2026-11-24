//! Пути к файлам приложения.

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

/// Имя папки приложения в `%APPDATA%` и `%LOCALAPPDATA%`.
const APP_DIR: &str = "XonkyPass";

/// Папка журналов: `%LOCALAPPDATA%\XonkyPass\logs`.
pub fn log_dir(app: &AppHandle) -> tauri::Result<PathBuf> {
    Ok(app.path().local_data_dir()?.join(APP_DIR).join("logs"))
}

/// Папка данных: `%APPDATA%\XonkyPass`.
fn data_dir(app: &AppHandle) -> tauri::Result<PathBuf> {
    Ok(app.path().data_dir()?.join(APP_DIR))
}

/// Файл хранилища: `%APPDATA%\XonkyPass\vault.xpv`.
pub fn vault_path(app: &AppHandle) -> tauri::Result<PathBuf> {
    Ok(data_dir(app)?.join("vault.xpv"))
}

/// Настройки: `%APPDATA%\XonkyPass\settings.json`.
pub fn settings_path(app: &AppHandle) -> tauri::Result<PathBuf> {
    Ok(data_dir(app)?.join("settings.json"))
}
