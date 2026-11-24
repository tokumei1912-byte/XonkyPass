//! Управление окнами `panel` и `palette`.
//!
//! Оба окна создаются при старте скрытыми (см. `tauri.conf.json`) и дальше только
//! показываются и прячутся — так появление получается мгновенным.

use tauri::{AppHandle, Manager, Runtime, WebviewWindow, Window};

use crate::platform;

/// Панель со списком записей (у края экрана).
pub const PANEL: &str = "panel";
/// Палитра поиска и быстрого добавления (по центру экрана).
pub const PALETTE: &str = "palette";

/// Подготовка окон после создания.
pub fn prepare(app: &AppHandle) {
    for label in [PANEL, PALETTE] {
        match app.get_webview_window(label) {
            Some(window) => platform::hide_from_alt_tab(&window),
            None => tracing::error!(label, "окно не найдено"),
        }
    }
}

pub fn show_panel(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(PANEL) {
        show(&window);
    }
}

/// Палитра появляется по центру монитора (позже — того, где курсор).
pub fn show_palette(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(PALETTE) {
        log_err(window.label(), "center", window.center());
        show(&window);
    }
}

pub fn toggle_panel(app: &AppHandle) {
    let Some(window) = app.get_webview_window(PANEL) else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        log_err(window.label(), "hide", window.hide());
    } else {
        show(&window);
    }
}

fn show(window: &WebviewWindow) {
    log_err(window.label(), "unminimize", window.unminimize());
    log_err(window.label(), "show", window.show());
    // Некоторые стили окна Tauri пересчитывает при показе — повторяем настройку.
    platform::hide_from_alt_tab(window);
    log_err(window.label(), "set_focus", window.set_focus());
}

pub fn hide<R: Runtime>(window: &Window<R>) {
    log_err(window.label(), "hide", window.hide());
}

fn log_err(label: &str, action: &str, result: tauri::Result<()>) {
    if let Err(err) = result {
        tracing::warn!(label, action, %err, "ошибка операции с окном");
    }
}
