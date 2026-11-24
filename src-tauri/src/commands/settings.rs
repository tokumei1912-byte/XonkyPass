//! Настройки приложения.

use tauri::AppHandle;

use crate::error::CommandError;
use crate::settings::{self, Settings};
use crate::state::blocking;

#[tauri::command]
pub async fn settings_get(app: AppHandle) -> Result<Settings, CommandError> {
    blocking(&app, |_, state| Ok(state.settings())).await
}

/// Сохраняет настройки целиком и возвращает то, что сохранено.
#[tauri::command]
pub async fn settings_set(app: AppHandle, settings: Settings) -> Result<Settings, CommandError> {
    settings.validate()?;
    blocking(&app, move |_, state| {
        settings::save(&state.settings_path, &settings).map_err(CommandError::internal)?;
        state.set_settings(settings.clone());
        Ok(settings)
    })
    .await
}
