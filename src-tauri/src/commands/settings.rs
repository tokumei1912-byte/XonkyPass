//! Настройки приложения.

use tauri::{AppHandle, Emitter};

use crate::error::CommandError;
use crate::settings::Settings;
use crate::state::blocking;
use crate::{dock, events, window_ctl};

#[tauri::command]
pub async fn settings_get(app: AppHandle) -> Result<Settings, CommandError> {
    blocking(&app, |_, state| Ok(state.settings())).await
}

/// Сохраняет настройки целиком и возвращает то, что сохранено.
#[tauri::command]
pub async fn settings_set(app: AppHandle, settings: Settings) -> Result<Settings, CommandError> {
    settings.validate()?;
    blocking(&app, move |app, state| {
        let saved = state.update_settings(|current| *current = settings)?;
        window_ctl::apply_theme(app, saved.theme);
        dock::send(app, dock::Command::Settings(saved.dock.clone()));
        notify(app, &saved);
        Ok(saved)
    })
    .await
}

/// Сообщает окнам о новых настройках: тема нужна обоим, экран настроек показывает
/// положение панели, которое могло смениться перетаскиванием.
pub fn notify(app: &AppHandle, settings: &Settings) {
    if let Err(err) = app.emit(events::SETTINGS_CHANGED, settings) {
        tracing::warn!(%err, "не удалось сообщить окнам о новых настройках");
    }
}
