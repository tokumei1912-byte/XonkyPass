//! Записи: список, карточка, секреты, изменение, корзина, копирование в буфер.

use serde::Deserialize;
use tauri::AppHandle;
use xonkypass_core::{ItemId, ItemList, ItemMeta, ItemSecret, ItemSummary, Scope, Vault};
use zeroize::Zeroizing;

use crate::clipboard::{self, Copied};
use crate::error::CommandError;
use crate::state::{blocking, with_vault};

#[tauri::command]
pub async fn items_list(app: AppHandle, scope: Scope) -> Result<ItemList, CommandError> {
    with_vault(&app, move |vault| Ok(vault.items(scope)?)).await
}

#[tauri::command]
pub async fn item_get(app: AppHandle, id: ItemId) -> Result<ItemSummary, CommandError> {
    with_vault(&app, move |vault| Ok(vault.item(id)?)).await
}

/// Секретная часть записи — только по явному действию (показать, изменить).
#[tauri::command]
pub async fn item_reveal(app: AppHandle, id: ItemId) -> Result<ItemSecret, CommandError> {
    with_vault(&app, move |vault| Ok(vault.reveal(id)?)).await
}

#[tauri::command]
pub async fn item_create(
    app: AppHandle,
    meta: ItemMeta,
    secret: ItemSecret,
) -> Result<ItemSummary, CommandError> {
    with_vault(&app, move |vault| {
        let id = vault.create_item(meta, secret)?;
        Ok(vault.item(id)?)
    })
    .await
}

/// `secret: null` — секреты не менялись (например, переименовали запись).
#[tauri::command]
pub async fn item_update(
    app: AppHandle,
    id: ItemId,
    meta: ItemMeta,
    secret: Option<ItemSecret>,
) -> Result<ItemSummary, CommandError> {
    with_vault(&app, move |vault| {
        vault.update_item(id, meta, secret)?;
        Ok(vault.item(id)?)
    })
    .await
}

#[tauri::command]
pub async fn item_trash(app: AppHandle, id: ItemId) -> Result<(), CommandError> {
    with_vault(&app, move |vault| Ok(vault.trash(id)?)).await
}

#[tauri::command]
pub async fn item_restore(app: AppHandle, id: ItemId) -> Result<(), CommandError> {
    with_vault(&app, move |vault| Ok(vault.restore(id)?)).await
}

/// Удаляет запись из корзины навсегда.
#[tauri::command]
pub async fn item_purge(app: AppHandle, id: ItemId) -> Result<(), CommandError> {
    with_vault(&app, move |vault| Ok(vault.purge(id)?)).await
}

/// Очищает корзину, возвращает число удалённых записей.
#[tauri::command]
pub async fn trash_empty(app: AppHandle) -> Result<usize, CommandError> {
    with_vault(&app, |vault| Ok(vault.empty_trash()?)).await
}

/// Что скопировать из записи: `{ "field": "password" }`, `{ "field": "custom", "index": 0 }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(tag = "field", rename_all = "snake_case")]
pub enum CopyTarget {
    Username,
    Password,
    /// Заметки к логину (секретная часть).
    Notes,
    /// Текст заметки.
    Body,
    /// Дополнительное поле по номеру.
    Custom {
        index: usize,
    },
}

/// Копирует поле записи в буфер, не передавая его в интерфейс.
#[tauri::command]
pub async fn item_copy(
    app: AppHandle,
    id: ItemId,
    target: CopyTarget,
) -> Result<Copied, CommandError> {
    blocking(&app, move |app, state| {
        let text = {
            let mut guard = state.vault();
            let vault = guard.as_mut().ok_or_else(CommandError::locked)?;
            let text = field_text(vault, id, target)?;
            if matches!(target, CopyTarget::Username | CopyTarget::Password)
                && let Err(err) = vault.record_use(id)
            {
                tracing::warn!(%err, "не удалось отметить использование записи");
            }
            text
        };
        if text.is_empty() {
            return Err(CommandError::empty_field());
        }
        clipboard::copy_secret(app, &text)
    })
    .await
}

fn field_text(
    vault: &Vault,
    id: ItemId,
    target: CopyTarget,
) -> Result<Zeroizing<String>, CommandError> {
    let text = match target {
        CopyTarget::Username => match vault.item(id)?.meta {
            ItemMeta::Login(login) => login.username,
            ItemMeta::Note(_) => return Err(CommandError::empty_field()),
        },
        CopyTarget::Body => match vault.item(id)?.meta {
            ItemMeta::Note(note) => note.body,
            ItemMeta::Login(_) => return Err(CommandError::empty_field()),
        },
        // `ItemSecret` затирается при удалении, поэтому поля из него не перемещаются,
        // а забираются через `mem::take`.
        CopyTarget::Password => std::mem::take(&mut vault.reveal(id)?.password),
        CopyTarget::Notes => std::mem::take(&mut vault.reveal(id)?.notes),
        CopyTarget::Custom { index } => {
            let mut secret = vault.reveal(id)?;
            let field = secret
                .fields
                .get_mut(index)
                .ok_or_else(CommandError::empty_field)?;
            std::mem::take(&mut field.value)
        }
    };
    Ok(Zeroizing::new(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_target_json_shape() {
        let parse = |json: &str| serde_json::from_str::<CopyTarget>(json).unwrap();
        assert_eq!(parse(r#"{"field":"password"}"#), CopyTarget::Password);
        assert_eq!(parse(r#"{"field":"username"}"#), CopyTarget::Username);
        assert_eq!(
            parse(r#"{"field":"custom","index":2}"#),
            CopyTarget::Custom { index: 2 }
        );
        assert!(serde_json::from_str::<CopyTarget>(r#"{"field":"totp"}"#).is_err());
    }
}
