//! Состояние приложения: открытое хранилище и настройки.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

use tauri::{AppHandle, Emitter, Manager};
use xonkypass_core::Vault;

use crate::error::CommandError;
use crate::settings::{self, Settings};
use crate::{clipboard, events};

pub struct AppState {
    pub vault_path: PathBuf,
    pub settings_path: PathBuf,
    /// `None` — хранилище заблокировано (или ещё не создано).
    vault: Mutex<Option<Vault>>,
    settings: Mutex<Settings>,
}

impl AppState {
    pub fn new(vault_path: PathBuf, settings_path: PathBuf, settings: Settings) -> Self {
        Self {
            vault_path,
            settings_path,
            vault: Mutex::new(None),
            settings: Mutex::new(settings),
        }
    }

    /// Доступ к хранилищу. Паника в другой команде не должна навсегда закрыть доступ,
    /// поэтому «отравленный» мьютекс просто используется дальше.
    pub fn vault(&self) -> MutexGuard<'_, Option<Vault>> {
        self.vault.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn settings(&self) -> Settings {
        self.settings
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Меняет настройки и записывает файл под одной блокировкой: окно настроек и панель
    /// (после перетаскивания к другому краю) могут сохранять одновременно.
    pub fn update_settings(
        &self,
        change: impl FnOnce(&mut Settings),
    ) -> Result<Settings, CommandError> {
        let mut guard = self.settings.lock().unwrap_or_else(PoisonError::into_inner);
        let mut next = guard.clone();
        change(&mut next);
        next.validate()?;
        settings::save(&self.settings_path, &next).map_err(CommandError::internal)?;
        *guard = next.clone();
        Ok(next)
    }
}

/// Выполняет `f` в пуле блокирующих задач. Argon2 считается до секунды, а SQLite
/// работает с диском: ни то ни другое не должно занимать поток интерфейса.
pub async fn blocking<T, F>(app: &AppHandle, f: F) -> Result<T, CommandError>
where
    T: Send + 'static,
    F: FnOnce(&AppHandle, &AppState) -> Result<T, CommandError> + Send + 'static,
{
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        f(&app, &state)
    })
    .await
    .map_err(CommandError::internal)?
}

/// Как [`blocking`], но с открытым хранилищем; если оно заблокировано — ошибка `locked`.
pub async fn with_vault<T, F>(app: &AppHandle, f: F) -> Result<T, CommandError>
where
    T: Send + 'static,
    F: FnOnce(&mut Vault) -> Result<T, CommandError> + Send + 'static,
{
    blocking(app, move |_, state| {
        let mut guard = state.vault();
        let vault = guard.as_mut().ok_or_else(CommandError::locked)?;
        f(vault)
    })
    .await
}

/// Блокирует хранилище: закрывает базу и затирает ключ, очищает буфер обмена, если
/// там наш секрет, и сообщает окнам. Ждёт, пока закончится текущая команда с
/// хранилищем, поэтому из потока интерфейса вызывается через `spawn_blocking`.
pub fn lock_vault(app: &AppHandle, reason: &'static str) {
    let vault = app.state::<AppState>().vault().take();
    if let Some(vault) = vault {
        vault.lock();
        tracing::info!(reason, "хранилище заблокировано");
    }
    clipboard::clear_ours(app);
    if let Err(err) = app.emit(events::VAULT_LOCKED, ()) {
        tracing::warn!(%err, "не удалось сообщить окнам о блокировке");
    }
}
