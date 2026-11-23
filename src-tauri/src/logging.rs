//! Журналы и обработка паник.
//!
//! Журналы пишутся в `%LOCALAPPDATA%\XonkyPass\logs`, новый файл каждый день,
//! хранятся 7 последних. В журналы никогда не пишутся секреты и содержимое записей.
//! Уровень меняется переменной окружения `XONKYPASS_LOG` (например, `debug`).

use std::backtrace::Backtrace;

use tauri::AppHandle;
use tracing_appender::rolling::{Builder, Rotation};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use crate::paths;

const KEEP_LOG_FILES: usize = 7;

pub fn init(app: &AppHandle) {
    let filter =
        EnvFilter::try_from_env("XONKYPASS_LOG").unwrap_or_else(|_| EnvFilter::new("info"));

    let file_layer = paths::log_dir(app)
        .ok()
        .and_then(|dir| {
            std::fs::create_dir_all(&dir).ok()?;
            Builder::new()
                .rotation(Rotation::DAILY)
                .filename_prefix("xonkypass")
                .filename_suffix("log")
                .max_log_files(KEEP_LOG_FILES)
                .build(dir)
                .ok()
        })
        .map(|appender| fmt::layer().with_ansi(false).with_writer(appender));

    // В отладочной сборке дублируем журнал в консоль.
    let console_layer = cfg!(debug_assertions).then(fmt::layer);

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(console_layer)
        .try_init();

    install_panic_hook();
}

/// Записывает паники в журнал, затем вызывает стандартный обработчик.
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = Backtrace::force_capture();
        tracing::error!("паника: {info}\n{backtrace}");
        default_hook(info);
    }));
}
