//! Управление окнами `panel` и `palette`.
//!
//! Оба окна создаются при старте скрытыми (см. `tauri.conf.json`) и дальше только
//! показываются и прячутся — так появление получается мгновенным.

use tauri::{AppHandle, Manager, Runtime, WebviewWindow, Window};

use crate::platform;
use crate::settings::Theme;

/// Панель со списком записей (у края экрана).
pub const PANEL: &str = "panel";
/// Палитра поиска и быстрого добавления (по центру экрана).
pub const PALETTE: &str = "palette";

/// Включился ли у окон фон Mica. Без него интерфейс рисует непрозрачный фон сам.
pub struct Backdrop {
    pub mica: bool,
}

/// Подготовка окон после создания: стиль, тема и фон Mica.
pub fn prepare(app: &AppHandle, theme: Theme) {
    let mut mica = true;
    for label in [PANEL, PALETTE] {
        let Some(window) = app.get_webview_window(label) else {
            tracing::error!(label, "окно не найдено");
            mica = false;
            continue;
        };
        platform::hide_from_alt_tab(&window);
        set_theme(&window, theme);
        if let Err(err) = platform::appearance::apply_mica(platform::window_handle(&window)) {
            tracing::info!(label, %err, "фон Mica недоступен");
            mica = false;
        }
    }
    app.manage(Backdrop { mica });
}

/// Тема окон: от неё зависят оттенок Mica и `prefers-color-scheme` в интерфейсе.
pub fn apply_theme(app: &AppHandle, theme: Theme) {
    for label in [PANEL, PALETTE] {
        if let Some(window) = app.get_webview_window(label) {
            set_theme(&window, theme);
        }
    }
}

fn set_theme(window: &WebviewWindow, theme: Theme) {
    let theme = match theme {
        Theme::System => None,
        Theme::Light => Some(tauri::Theme::Light),
        Theme::Dark => Some(tauri::Theme::Dark),
    };
    log_err(window.label(), "set_theme", window.set_theme(theme));
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

/// Видно ли сейчас хоть одно окно приложения.
pub fn any_visible(app: &AppHandle) -> bool {
    [PANEL, PALETTE]
        .into_iter()
        .filter_map(|label| app.get_webview_window(label))
        .any(|window| window.is_visible().unwrap_or(false))
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
