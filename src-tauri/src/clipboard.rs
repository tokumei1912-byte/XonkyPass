//! Копирование секретов в буфер обмена с автоочисткой.
//!
//! Текст кладёт в буфер сам Rust (интерфейс пароль не получает). Через заданное
//! в настройках время буфер очищается, но только если в нём всё ещё то, что положили
//! мы: если пользователь успел скопировать что-то своё, это не трогается.

use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::error::CommandError;
use crate::platform::{self, clipboard as sys};
use crate::state::AppState;
use crate::{events, window_ctl};

#[derive(Default)]
pub struct Clipboard {
    /// Окно-владелец буфера (HWND панели), 0 — без владельца.
    owner: AtomicIsize,
    /// Номер последовательности буфера после нашей записи; `None` — нашего там нет.
    ours: Mutex<Option<u32>>,
}

/// Результат копирования для интерфейса.
#[derive(Debug, Clone, Serialize)]
pub struct Copied {
    /// Через сколько секунд буфер очистится; 0 — не очистится.
    pub clear_after_secs: u32,
}

/// Запоминает окно-владельца. Вызывается после создания окон.
pub fn init(app: &AppHandle) {
    if let Some(panel) = app.get_webview_window(window_ctl::PANEL) {
        let owner = platform::window_handle(&panel);
        app.state::<Clipboard>()
            .owner
            .store(owner, Ordering::Relaxed);
    }
}

/// Кладёт секрет в буфер и заводит таймер очистки. Блокирующая: буфер может быть
/// ненадолго занят другой программой.
pub fn copy_secret(app: &AppHandle, text: &str) -> Result<Copied, CommandError> {
    let clipboard = app.state::<Clipboard>();
    let mut ours = clipboard
        .ours
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let seq = sys::write_secret(clipboard.owner(), text).map_err(CommandError::clipboard)?;
    *ours = Some(seq);
    drop(ours);

    let clear_after_secs = app.state::<AppState>().settings().clipboard_clear_secs;
    if clear_after_secs > 0 {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_secs(clear_after_secs.into())).await;
            let cleared =
                tauri::async_runtime::spawn_blocking(move || clear_if_ours(&app, Some(seq)));
            if let Err(err) = cleared.await {
                tracing::warn!(%err, "сбой задачи очистки буфера");
            }
        });
    }
    Ok(Copied { clear_after_secs })
}

/// Очищает буфер, если там наш секрет. Возвращает `true`, если очистил.
pub fn clear_ours(app: &AppHandle) -> bool {
    clear_if_ours(app, None)
}

/// `expected` — номер конкретной копии: таймер старой копии не трогает более новую,
/// у неё свой таймер.
fn clear_if_ours(app: &AppHandle, expected: Option<u32>) -> bool {
    let clipboard = app.state::<Clipboard>();
    let mut ours = clipboard
        .ours
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let Some(seq) = *ours else {
        return false;
    };
    if expected.is_some_and(|expected| expected != seq) {
        return false;
    }
    *ours = None;
    match sys::clear_if_unchanged(clipboard.owner(), seq) {
        Ok(true) => {
            tracing::debug!("буфер обмена очищен");
            if let Err(err) = app.emit(events::CLIPBOARD_CLEARED, ()) {
                tracing::warn!(%err, "не удалось сообщить об очистке буфера");
            }
            true
        }
        // В буфере уже что-то другое — значит, не наше.
        Ok(false) => false,
        Err(err) => {
            tracing::warn!(%err, "не удалось очистить буфер обмена");
            false
        }
    }
}

impl Clipboard {
    fn owner(&self) -> isize {
        self.owner.load(Ordering::Relaxed)
    }
}
