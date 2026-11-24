//! Создание, разблокировка и блокировка хранилища, смена пароля и ключа восстановления.

use serde::Serialize;
use tauri::AppHandle;
use xonkypass_core::kdf::{KdfParams, UNLOCK_TARGET};
use xonkypass_core::{RecoveryKey, UnlockMethod, Vault};
use zeroize::Zeroizing;

use crate::error::CommandError;
use crate::state::{self, blocking, with_vault};

#[derive(Debug, Serialize)]
pub struct VaultStatus {
    /// Есть ли файл хранилища. Нет — показываем первый запуск.
    exists: bool,
    unlocked: bool,
    /// Чем разблокировано; после ключа восстановления нужно задать новый пароль.
    unlocked_with: Option<UnlockMethod>,
    path: String,
}

#[tauri::command]
pub async fn vault_status(app: AppHandle) -> Result<VaultStatus, CommandError> {
    blocking(&app, |_, state| {
        let vault = state.vault();
        Ok(VaultStatus {
            exists: Vault::exists(&state.vault_path),
            unlocked: vault.is_some(),
            unlocked_with: vault.as_ref().map(Vault::unlocked_with),
            path: state.vault_path.display().to_string(),
        })
    })
    .await
}

/// Создаёт хранилище и сразу открывает его. Возвращает ключ восстановления
/// для показа пользователю (один раз).
#[tauri::command]
pub async fn vault_create(app: AppHandle, password: String) -> Result<String, CommandError> {
    let password = Zeroizing::new(password);
    blocking(&app, move |_, state| {
        let mut vault = state.vault();
        let kdf = KdfParams::calibrate(UNLOCK_TARGET);
        let (created, recovery) = Vault::create(&state.vault_path, &password, kdf)?;
        *vault = Some(created);
        tracing::info!(
            m_kib = kdf.m_kib(),
            t = kdf.t(),
            p = kdf.p(),
            "хранилище создано"
        );
        Ok(String::clone(&recovery.to_display()))
    })
    .await
}

#[tauri::command]
pub async fn vault_unlock(app: AppHandle, password: String) -> Result<(), CommandError> {
    let password = Zeroizing::new(password);
    blocking(&app, move |_, state| {
        // Argon2 считается без блокировки: остальные команды не ждут.
        let opened = Vault::unlock(&state.vault_path, &password)?;
        *state.vault() = Some(opened);
        tracing::info!("хранилище разблокировано паролем");
        Ok(())
    })
    .await
}

/// Разблокировка ключом восстановления. После неё интерфейс просит задать новый
/// мастер-пароль (`vault_reset_password`).
#[tauri::command]
pub async fn vault_unlock_recovery(app: AppHandle, key: String) -> Result<(), CommandError> {
    let key = Zeroizing::new(key);
    blocking(&app, move |_, state| {
        let key = RecoveryKey::parse(&key)?;
        let opened = Vault::unlock_with_recovery_key(&state.vault_path, &key)?;
        *state.vault() = Some(opened);
        tracing::info!("хранилище разблокировано ключом восстановления");
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn vault_lock(app: AppHandle) -> Result<(), CommandError> {
    blocking(&app, |app, _| {
        state::lock_vault(app, "вручную");
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn vault_change_password(
    app: AppHandle,
    current: String,
    new: String,
) -> Result<(), CommandError> {
    let (current, new) = (Zeroizing::new(current), Zeroizing::new(new));
    with_vault(&app, move |vault| {
        let kdf = KdfParams::calibrate(UNLOCK_TARGET);
        vault.change_password(&current, &new, kdf)?;
        tracing::info!("мастер-пароль изменён");
        Ok(())
    })
    .await
}

/// Новый мастер-пароль без старого: только после разблокировки ключом восстановления.
#[tauri::command]
pub async fn vault_reset_password(app: AppHandle, new: String) -> Result<(), CommandError> {
    let new = Zeroizing::new(new);
    with_vault(&app, move |vault| {
        let kdf = KdfParams::calibrate(UNLOCK_TARGET);
        vault.reset_password(&new, kdf)?;
        tracing::info!("мастер-пароль задан заново по ключу восстановления");
        Ok(())
    })
    .await
}

/// Новый ключ восстановления; старый перестаёт работать.
#[tauri::command]
pub async fn vault_regenerate_recovery_key(app: AppHandle) -> Result<String, CommandError> {
    with_vault(&app, |vault| {
        let recovery = vault.regenerate_recovery_key()?;
        tracing::info!("создан новый ключ восстановления");
        Ok(String::clone(&recovery.to_display()))
    })
    .await
}
