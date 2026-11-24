//! События, которые Rust отправляет окнам.

/// Хранилище заблокировано (вручную, из трея; позже — по таймеру, Win+L, сну).
pub const VAULT_LOCKED: &str = "vault://locked";

/// Буфер обмена очищен: истекло время или хранилище заблокировали.
pub const CLIPBOARD_CLEARED: &str = "clipboard://cleared";
