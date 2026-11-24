//! События, которые Rust отправляет окнам.

/// Хранилище заблокировано (вручную, из трея; позже — по таймеру, Win+L, сну).
pub const VAULT_LOCKED: &str = "vault://locked";

/// Буфер обмена очищен: истекло время или хранилище заблокировали.
pub const CLIPBOARD_CLEARED: &str = "clipboard://cleared";

/// Настройки изменились; в событии — новые настройки (тема нужна обоим окнам).
pub const SETTINGS_CHANGED: &str = "settings://changed";

/// Панель показана, спрятана, закреплена или сменила режим; в событии — `DockState`.
pub const DOCK_STATE: &str = "dock://state";
