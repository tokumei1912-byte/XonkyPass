//! Стили окон Win32.

use tauri::WebviewWindow;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetWindowLongPtrW, SetWindowLongPtrW, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
};

/// Убирает окно из Alt+Tab и панели задач (стиль «окно инструментов»).
pub fn hide_from_alt_tab(window: &WebviewWindow) {
    let hwnd = match window.hwnd() {
        // Tauri может использовать другую версию крейта `windows`, поэтому
        // переносим дескриптор через сырой указатель.
        Ok(raw) => HWND(raw.0),
        Err(err) => {
            tracing::warn!(label = window.label(), %err, "нет HWND окна");
            return;
        }
    };

    // SAFETY: `hwnd` — действительный дескриптор окна, принадлежащего этому процессу.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let tool = WS_EX_TOOLWINDOW.0 as isize;
        let app = WS_EX_APPWINDOW.0 as isize;
        let new_style = (style | tool) & !app;
        if new_style != style {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style);
        }
    }
}
