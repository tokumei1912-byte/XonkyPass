//! Оболочка Windows: ссылки в браузере и файлы в Проводнике.

use std::path::Path;
use std::process::Command;

use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HSTRING, PCWSTR, w};

use super::{PlatformError, PlatformResult};

/// Открывает адрес в браузере по умолчанию. Адрес проверяет вызывающий код.
pub fn open_url(url: &str) -> PlatformResult<()> {
    // SAFETY: все строки живут до конца вызова; окно-владелец не нужно.
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            &HSTRING::from(url),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    // По документации успех — значение больше 32, остальное — код ошибки.
    let code = result.0 as usize;
    if code > 32 {
        Ok(())
    } else {
        Err(PlatformError(format!("ShellExecute вернул {code}")))
    }
}

/// Открывает Проводник с выделенным файлом.
pub fn reveal(path: &Path) -> PlatformResult<()> {
    // Код возврата Проводника ничего не говорит (бывает 1 и при успехе), поэтому не ждём его.
    Command::new("explorer.exe")
        .arg("/select,")
        .arg(path)
        .spawn()
        .map(drop)
        .map_err(|err| PlatformError(format!("не удалось запустить Проводник: {err}")))
}
