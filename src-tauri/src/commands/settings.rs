//! Настройки приложения.

use tauri::{AppHandle, Emitter};

use crate::error::CommandError;
use crate::events;
use crate::settings::{self, Settings};
use crate::state::blocking;
use crate::window_ctl;

#[tauri::command]
pub async fn settings_get(app: AppHandle) -> Result<Settings, CommandError> {
    blocking(&app, |_, state| Ok(state.settings())).await
}

/// Сохраняет настройки целиком и возвращает то, что сохранено.
#[tauri::command]
pub async fn settings_set(app: AppHandle, settings: Settings) -> Result<Settings, CommandError> {
    settings.validate()?;
    blocking(&app, move |app, state| {
        settings::save(&state.settings_path, &settings).map_err(CommandError::internal)?;
        state.set_settings(settings.clone());
        window_ctl::apply_theme(app, settings.theme);
        if let Err(err) = app.emit(events::SETTINGS_CHANGED, &settings) {
            tracing::warn!(%err, "не удалось сообщить окнам о новых настройках");
        }
        Ok(settings)
    })
    .await
}
