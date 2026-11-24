//! Оформление окон: фон Mica и цвет акцента Windows.

use windows::UI::ViewManagement::{UIColorType, UISettings};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{
    DWMSBT_MAINWINDOW, DWMWA_SYSTEMBACKDROP_TYPE, DwmSetWindowAttribute,
};

use super::{Accent, PlatformError, PlatformResult};

/// Включает фон Mica. DWM знает этот атрибут с Windows 11 22H2; в более старых
/// версиях вызов возвращает ошибку, и окно остаётся с обычным непрозрачным фоном.
pub fn apply_mica(hwnd: isize) -> PlatformResult<()> {
    if hwnd == 0 {
        return Err(PlatformError("нет HWND окна".into()));
    }
    let backdrop = DWMSBT_MAINWINDOW;
    // SAFETY: HWND живого окна; указатель и размер — локальной переменной нужного типа.
    unsafe {
        DwmSetWindowAttribute(
            HWND(hwnd as *mut _),
            DWMWA_SYSTEMBACKDROP_TYPE,
            (&raw const backdrop).cast(),
            size_of_val(&backdrop) as u32,
        )?;
    }
    Ok(())
}

/// Цвет акцента из настроек Windows. Вызывать из потока с COM — главного потока
/// приложения (синхронные команды Tauri выполняются в нём).
pub fn accent() -> PlatformResult<Accent> {
    let settings = UISettings::new()?;
    let hex = |kind| -> PlatformResult<String> {
        let c = settings.GetColorValue(kind)?;
        Ok(format!("#{:02x}{:02x}{:02x}", c.R, c.G, c.B))
    };
    // Те же оттенки, что берёт сама Windows 11 для кнопок и ссылок.
    Ok(Accent {
        light: hex(UIColorType::AccentDark1)?,
        dark: hex(UIColorType::AccentLight2)?,
    })
}
