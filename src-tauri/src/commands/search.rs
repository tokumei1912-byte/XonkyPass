//! Поиск записей и теги.

use tauri::AppHandle;
use xonkypass_core::{SearchOptions, SearchResult};

use crate::error::CommandError;
use crate::state::with_vault;

/// Ищет записи вне корзины. Синтаксис запроса: слова, `#тег`, `is:login`,
/// `is:note`, `is:fav`, `has:totp` (см. `xonkypass_core::search`).
#[tauri::command]
pub async fn items_search(
    app: AppHandle,
    query: String,
    options: SearchOptions,
) -> Result<SearchResult, CommandError> {
    with_vault(&app, move |vault| Ok(vault.search(&query, &options))).await
}

/// Переименовывает тег во всех записях; если новый тег уже есть — сливает их.
/// Возвращает число изменённых записей.
#[tauri::command]
pub async fn tag_rename(app: AppHandle, from: String, to: String) -> Result<usize, CommandError> {
    with_vault(&app, move |vault| Ok(vault.rename_tag(&from, &to)?)).await
}

/// Убирает тег из всех записей. Возвращает число изменённых записей.
#[tauri::command]
pub async fn tag_delete(app: AppHandle, tag: String) -> Result<usize, CommandError> {
    with_vault(&app, move |vault| Ok(vault.delete_tag(&tag)?)).await
}
