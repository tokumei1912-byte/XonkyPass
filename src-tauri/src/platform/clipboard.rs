//! Буфер обмена Win32.
//!
//! Вместе с текстом в буфер кладутся служебные форматы, которые просят Windows
//! не сохранять его в журнал буфера (Win+V), не отправлять в облачный буфер
//! и не показывать программам, следящим за буфером. Очистка проверяет номер
//! последовательности буфера, чтобы не стереть то, что пользователь скопировал сам.

use std::ffi::c_void;
use std::thread;
use std::time::Duration;

use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardSequenceNumber, OpenClipboard,
    RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::core::{Error, PCWSTR, w};
use zeroize::Zeroizing;

use super::PlatformResult;

/// `CF_UNICODETEXT`: текст в UTF-16 с нулём в конце.
const CF_UNICODETEXT: u32 = 13;

/// Буфер может быть ненадолго открыт другой программой.
const OPEN_ATTEMPTS: u32 = 10;
const OPEN_RETRY_DELAY: Duration = Duration::from_millis(20);

/// Кладёт секрет в буфер. Возвращает номер последовательности буфера после записи.
pub fn write_secret(owner: isize, text: &str) -> PlatformResult<u32> {
    let utf16: Zeroizing<Vec<u8>> = Zeroizing::new(
        text.encode_utf16()
            .chain(std::iter::once(0))
            .flat_map(u16::to_le_bytes)
            .collect(),
    );
    {
        let _open = Opened::new(owner)?;
        // SAFETY: буфер открыт этим потоком (`Opened`).
        unsafe { EmptyClipboard() }?;
        if let Err(err) = fill(&utf16) {
            // Без служебных форматов секрет попал бы в журнал Win+V — лучше ничего.
            // SAFETY: буфер всё ещё открыт этим потоком.
            let _ = unsafe { EmptyClipboard() };
            return Err(err);
        }
    }
    Ok(sequence_number())
}

/// Очищает буфер, если с момента записи `seq` в нём ничего не менялось.
pub fn clear_if_unchanged(owner: isize, seq: u32) -> PlatformResult<bool> {
    if sequence_number() != seq {
        return Ok(false);
    }
    let _open = Opened::new(owner)?;
    // Пока ждали буфер, его могли изменить.
    if sequence_number() != seq {
        return Ok(false);
    }
    // SAFETY: буфер открыт этим потоком (`Opened`).
    unsafe { EmptyClipboard() }?;
    Ok(true)
}

fn fill(utf16: &[u8]) -> PlatformResult<()> {
    // Значение DWORD 0 — «нельзя». Для Exclude… важен сам факт наличия формата.
    let no = 0u32.to_le_bytes();
    set(CF_UNICODETEXT, utf16)?;
    set(
        register(w!("ExcludeClipboardContentFromMonitorProcessing"))?,
        &no,
    )?;
    set(register(w!("CanIncludeInClipboardHistory"))?, &no)?;
    set(register(w!("CanUploadToCloudClipboard"))?, &no)?;
    Ok(())
}

fn sequence_number() -> u32 {
    // SAFETY: функция без аргументов, только читает счётчик.
    unsafe { GetClipboardSequenceNumber() }
}

fn register(name: PCWSTR) -> PlatformResult<u32> {
    // SAFETY: `name` — статическая строка UTF-16 с нулём в конце (`w!`).
    let format = unsafe { RegisterClipboardFormatW(name) };
    if format == 0 {
        return Err(Error::from_thread().into());
    }
    Ok(format)
}

/// Копирует данные в глобальную память и отдаёт её буферу.
fn set(format: u32, data: &[u8]) -> PlatformResult<()> {
    // SAFETY: память выделяется здесь же и заполняется ровно на `data.len()` байт.
    // После успешного SetClipboardData ею владеет система, иначе освобождаем сами.
    unsafe {
        let mem = GlobalAlloc(GMEM_MOVEABLE, data.len())?;
        let ptr = GlobalLock(mem).cast::<u8>();
        if ptr.is_null() {
            free(mem);
            return Err(Error::from_thread().into());
        }
        std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, data.len());
        // Когда счётчик блокировок падает до нуля, функция сообщает «ошибку» с кодом 0.
        let _ = GlobalUnlock(mem);
        if let Err(err) = SetClipboardData(format, Some(HANDLE(mem.0))) {
            wipe(mem, data.len());
            free(mem);
            return Err(err.into());
        }
    }
    Ok(())
}

/// Затирает память, которую так и не отдали буферу (в ней может быть секрет).
fn wipe(mem: HGLOBAL, len: usize) {
    // SAFETY: `mem` выделена нами в `set`, её размер не меньше `len`.
    unsafe {
        let ptr = GlobalLock(mem).cast::<u8>();
        if !ptr.is_null() {
            std::ptr::write_bytes(ptr, 0, len);
            let _ = GlobalUnlock(mem);
        }
    }
}

fn free(mem: HGLOBAL) {
    // SAFETY: `mem` выделена нами и системе не передавалась. При успехе GlobalFree
    // возвращает NULL, и windows-rs считает это ошибкой, поэтому результат не нужен.
    let _ = unsafe { GlobalFree(Some(mem)) };
}

/// Открытый буфер обмена; закрывается при выходе из области видимости.
struct Opened;

impl Opened {
    fn new(owner: isize) -> PlatformResult<Self> {
        let owner = (owner != 0).then_some(HWND(owner as *mut c_void));
        let mut attempt = 1;
        loop {
            // SAFETY: `owner` — HWND окна этого процесса (или нет владельца).
            match unsafe { OpenClipboard(owner) } {
                Ok(()) => return Ok(Opened),
                Err(err) if attempt >= OPEN_ATTEMPTS => return Err(err.into()),
                Err(_) => {
                    attempt += 1;
                    thread::sleep(OPEN_RETRY_DELAY);
                }
            }
        }
    }
}

impl Drop for Opened {
    fn drop(&mut self) {
        // SAFETY: буфер открыт этим потоком в `Opened::new`.
        if let Err(err) = unsafe { CloseClipboard() } {
            tracing::warn!(%err, "не удалось закрыть буфер обмена");
        }
    }
}
