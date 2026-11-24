//! Панель у края экрана.

use tauri::{AppHandle, Manager};

use crate::dock::{self, Command, DockState, MonitorInfo};
use crate::state::AppState;
use crate::window_ctl;

/// Спрятать панель (Esc). Закреплённую тоже.
#[tauri::command]
pub fn dock_hide(app: AppHandle) {
    window_ctl::hide_panel(&app);
}

#[tauri::command]
pub fn dock_pin(app: AppHandle, pinned: bool) -> DockState {
    let settings = app.state::<AppState>().settings();
    dock::pin(&app, pinned, &settings.dock)
}

/// Открыт диалог или идёт редактирование: панель не прячется сама, даже если
/// пользователь ушёл в другое окно.
#[tauri::command]
pub fn dock_hold(app: AppHandle, hold: bool) {
    dock::send(&app, Command::Hold(hold));
}

#[tauri::command]
pub fn dock_state(app: AppHandle) -> DockState {
    let settings = app.state::<AppState>().settings();
    dock::state(&app, &settings.dock)
}

#[tauri::command]
pub fn dock_monitors() -> Vec<MonitorInfo> {
    dock::monitors()
}
