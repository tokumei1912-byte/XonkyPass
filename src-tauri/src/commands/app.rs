//! Сведения о приложении, оформление окон и обновления.

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::error::CommandError;
use crate::platform::{self, Accent};
use crate::updater;
use crate::window_ctl::Backdrop;

#[derive(Debug, Serialize)]
pub struct AppInfo {
    name: &'static str,
    version: &'static str,
    /// Обновления ставятся сами. В тестовых сборках — нет.
    auto_update: bool,
}

/// Название и версия приложения.
#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        name: "XonkyPass",
        version: env!("CARGO_PKG_VERSION"),
        auto_update: updater::ENABLED,
    }
}

#[derive(Debug, Serialize)]
pub struct Appearance {
    /// Под окном фон Mica: интерфейс рисует полупрозрачные слои вместо сплошного фона.
    mica: bool,
    /// Акцент Windows; `None` — взять стандартный синий.
    accent: Option<Accent>,
}

/// Оформление окон. Синхронная команда: акцент читается через WinRT в главном потоке.
#[tauri::command]
pub fn appearance(app: AppHandle) -> Appearance {
    let accent = platform::appearance::accent()
        .inspect_err(|err| tracing::debug!(%err, "акцент Windows недоступен"))
        .ok();
    Appearance {
        mica: app.state::<Backdrop>().mica,
        accent,
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum UpdateCheck {
    /// Тестовая сборка: обновления выключены.
    Disabled,
    UpToDate,
    /// Проверка уже идёт (по таймеру или из трея).
    Busy,
    /// Новая версия скачана и проверена, её можно ставить.
    Ready {
        version: String,
    },
}

/// Проверяет обновления сейчас и скачивает новую версию, если она есть.
#[tauri::command]
pub async fn update_check(app: AppHandle) -> Result<UpdateCheck, CommandError> {
    if !updater::ENABLED {
        return Ok(UpdateCheck::Disabled);
    }
    Ok(match updater::check(&app).await {
        updater::Checked::UpToDate => UpdateCheck::UpToDate,
        updater::Checked::Busy => UpdateCheck::Busy,
        updater::Checked::Ready(version) => UpdateCheck::Ready { version },
        updater::Checked::Failed => {
            return Err(CommandError::new(
                "update",
                "не удалось проверить обновления",
            ));
        }
    })
}

/// Ставит скачанное обновление: установщик закроет приложение и запустит новую версию.
#[tauri::command]
pub fn update_install(app: AppHandle) {
    updater::install_ready(&app);
}
