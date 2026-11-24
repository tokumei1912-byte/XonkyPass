//! Буфер обмена для текста, который уже есть в интерфейсе (сгенерированный пароль,
//! ключ восстановления). Поля записей копирует `item_copy`.

use tauri::AppHandle;
use zeroize::Zeroizing;

use crate::clipboard::{self, Copied};
use crate::error::CommandError;
use crate::state::blocking;

#[tauri::command]
pub async fn clipboard_copy(app: AppHandle, text: String) -> Result<Copied, CommandError> {
    let text = Zeroizing::new(text);
    if text.is_empty() {
        return Err(CommandError::empty_field());
    }
    blocking(&app, move |app, _| clipboard::copy_secret(app, &text)).await
}

/// «Очистить сейчас»: стирает буфер, если там всё ещё наш секрет.
#[tauri::command]
pub async fn clipboard_clear(app: AppHandle) -> Result<bool, CommandError> {
    blocking(&app, |app, _| Ok(clipboard::clear_ours(app))).await
}
