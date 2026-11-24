//! Системное окно «Сохранить как».

use std::path::PathBuf;

use windows::Win32::Foundation::{ERROR_CANCELLED, HWND};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoCreateInstance,
    CoInitializeEx, CoTaskMemFree, CoUninitialize,
};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FOS_FORCEFILESYSTEM, FOS_OVERWRITEPROMPT, FileSaveDialog, IFileSaveDialog, SIGDN_FILESYSPATH,
};
use windows::core::{HRESULT, HSTRING, PCWSTR};

use super::{PlatformError, PlatformResult};

/// Что спросить в окне сохранения.
pub struct SaveDialog<'a> {
    pub title: &'a str,
    pub file_name: &'a str,
    /// Название типа файла в списке, например «Текстовый файл».
    pub filter_name: &'a str,
    /// Расширение без точки, например `txt`.
    pub extension: &'a str,
}

/// Показывает окно «Сохранить как» поверх окна `owner` и ждёт выбора.
/// `None` — пользователь закрыл окно.
///
/// Окно живёт в отдельном потоке со своим COM (STA), чтобы не трогать главный
/// поток приложения.
pub fn save_file(owner: isize, options: &SaveDialog<'_>) -> PlatformResult<Option<PathBuf>> {
    std::thread::scope(|scope| {
        scope
            .spawn(|| show_save_dialog(owner, options))
            .join()
            .unwrap_or_else(|_| Err(PlatformError("окно сохранения упало".into())))
    })
}

fn show_save_dialog(owner: isize, options: &SaveDialog<'_>) -> PlatformResult<Option<PathBuf>> {
    // SAFETY: COM инициализируется и освобождается в этом же потоке.
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) }.ok()?;
    let _com = ComGuard;

    let filter_name = HSTRING::from(options.filter_name);
    let pattern = HSTRING::from(format!("*.{}", options.extension));
    let filters = [COMDLG_FILTERSPEC {
        pszName: PCWSTR(filter_name.as_ptr()),
        pszSpec: PCWSTR(pattern.as_ptr()),
    }];

    // SAFETY: обычная работа с COM-объектом диалога; строки живут до конца функции.
    unsafe {
        let dialog: IFileSaveDialog =
            CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)?;
        dialog.SetTitle(&HSTRING::from(options.title))?;
        dialog.SetFileTypes(&filters)?;
        dialog.SetDefaultExtension(&HSTRING::from(options.extension))?;
        dialog.SetFileName(&HSTRING::from(options.file_name))?;
        dialog.SetOptions(dialog.GetOptions()? | FOS_OVERWRITEPROMPT | FOS_FORCEFILESYSTEM)?;

        let owner = (owner != 0).then_some(HWND(owner as *mut _));
        match dialog.Show(owner) {
            Err(err) if err.code() == HRESULT::from_win32(ERROR_CANCELLED.0) => return Ok(None),
            result => result?,
        }

        let name = dialog.GetResult()?.GetDisplayName(SIGDN_FILESYSPATH)?;
        let path = name.to_string();
        CoTaskMemFree(Some(name.0.cast_const().cast()));
        let path = path.map_err(|err| PlatformError(format!("путь не в UTF-16: {err}")))?;
        Ok(Some(PathBuf::from(path)))
    }
}

struct ComGuard;

impl Drop for ComGuard {
    fn drop(&mut self) {
        // SAFETY: парный вызов к успешному `CoInitializeEx` в этом потоке.
        unsafe { CoUninitialize() };
    }
}
