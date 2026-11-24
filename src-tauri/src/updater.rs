//! Автообновление из GitHub Releases.
//!
//! Релизная сборка (её собирает CI при каждом изменении `main`) проверяет `latest.json`
//! последнего релиза через 15 секунд после запуска и дальше раз в 4 часа. Новая версия
//! скачивается в фоне, её подпись сверяется с открытым ключом из `tauri.conf.json`.
//! Установка начинается, когда окна приложения спрятаны хотя бы минуту: установщик
//! закрывает приложение, ставит новую версию и запускает её. Пункт меню трея позволяет
//! проверить и установить обновление сразу.
//!
//! В тестовых сборках (артефакты CI из веток, локальная сборка) автообновление выключено,
//! иначе сборка ветки, поставленная для проверки, тут же заменилась бы релизом.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use tauri::{AppHandle, Manager, Wry, menu::MenuItem};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::window_ctl;

/// Автообновление включено: переменную задаёт CI при сборке релиза.
pub const ENABLED: bool = option_env!("XONKYPASS_AUTO_UPDATE").is_some();

/// Идентификатор пункта меню трея.
pub const MENU_ID: &str = "update";

const FIRST_CHECK_DELAY: Duration = Duration::from_secs(15);
const CHECK_INTERVAL: Duration = Duration::from_secs(4 * 60 * 60);
const IDLE_POLL: Duration = Duration::from_secs(15);
/// Сколько проверок подряд окна должны быть спрятаны перед установкой (~1 минута).
const IDLE_POLLS_BEFORE_INSTALL: u32 = 4;

/// Состояние обновления, хранится в `app.state()`.
pub struct Updates {
    menu_item: MenuItem<Wry>,
    busy: AtomicBool,
    ready: Mutex<Option<Ready>>,
}

/// Скачанное и проверенное обновление.
struct Ready {
    update: Update,
    bytes: Vec<u8>,
}

enum Status<'a> {
    UpToDate,
    Checking,
    Downloading(&'a str),
    Ready(&'a str),
    Installing(&'a str),
    Failed,
}

/// Создаёт пункт меню трея и регистрирует состояние обновлений.
pub fn create_menu_item(app: &AppHandle) -> tauri::Result<MenuItem<Wry>> {
    let (text, enabled) = if ENABLED {
        ("Проверить обновления", true)
    } else {
        ("Тестовая сборка: без автообновления", false)
    };
    let item = MenuItem::with_id(app, MENU_ID, text, enabled, None::<&str>)?;
    app.manage(Updates {
        menu_item: item.clone(),
        busy: AtomicBool::new(false),
        ready: Mutex::new(None),
    });
    Ok(item)
}

/// Запускает фоновые проверки обновлений.
pub fn start(app: &AppHandle) {
    if !ENABLED {
        tracing::info!("тестовая сборка: автообновление выключено");
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK_DELAY).await;
        loop {
            check(&app).await;
            if has_ready(&app) {
                wait_until_idle(&app).await;
                install_ready(&app);
            }
            tokio::time::sleep(CHECK_INTERVAL).await;
        }
    });
}

/// Пункт меню: установить скачанное обновление или проверить и сразу поставить новое.
pub fn on_menu_click(app: &AppHandle) {
    if !ENABLED {
        return;
    }
    if has_ready(app) {
        install_ready(app);
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        check(&app).await;
        install_ready(&app);
    });
}

/// Проверяет наличие новой версии и скачивает её.
async fn check(app: &AppHandle) {
    let updates = app.state::<Updates>();
    if has_ready(app) || updates.busy.swap(true, Ordering::SeqCst) {
        return;
    }
    updates.set_status(Status::Checking);

    match download(app).await {
        Ok(Some(ready)) => {
            tracing::info!(version = %ready.update.version, "обновление скачано и проверено");
            updates.set_status(Status::Ready(&ready.update.version));
            *updates.lock_ready() = Some(ready);
        }
        Ok(None) => {
            tracing::info!("обновлений нет");
            updates.set_status(Status::UpToDate);
        }
        Err(err) => {
            tracing::warn!(%err, "не удалось проверить или скачать обновление");
            updates.set_status(Status::Failed);
        }
    }
    updates.busy.store(false, Ordering::SeqCst);
}

async fn download(app: &AppHandle) -> tauri_plugin_updater::Result<Option<Ready>> {
    let Some(update) = app.updater()?.check().await? else {
        return Ok(None);
    };
    tracing::info!(
        current = %update.current_version,
        new = %update.version,
        "найдено обновление, загрузка"
    );
    app.state::<Updates>()
        .set_status(Status::Downloading(&update.version));
    // `download` сразу проверяет подпись: файл с неверной подписью сюда не попадёт.
    let bytes = update.download(|_, _| {}, || {}).await?;
    Ok(Some(Ready { update, bytes }))
}

/// Ждёт, пока окна приложения не будут спрятаны около минуты.
async fn wait_until_idle(app: &AppHandle) {
    let mut idle_polls = 0;
    while idle_polls < IDLE_POLLS_BEFORE_INSTALL {
        tokio::time::sleep(IDLE_POLL).await;
        idle_polls = if window_ctl::any_visible(app) {
            0
        } else {
            idle_polls + 1
        };
    }
}

fn has_ready(app: &AppHandle) -> bool {
    app.state::<Updates>().lock_ready().is_some()
}

/// Устанавливает скачанное обновление. На Windows установщик закрывает приложение
/// и после установки запускает новую версию.
fn install_ready(app: &AppHandle) {
    let updates = app.state::<Updates>();
    let Some(ready) = updates.lock_ready().take() else {
        return;
    };
    tracing::info!(version = %ready.update.version, "установка обновления");
    updates.set_status(Status::Installing(&ready.update.version));
    if let Err(err) = ready.update.install(&ready.bytes) {
        tracing::error!(%err, "не удалось запустить установку обновления");
        updates.set_status(Status::Failed);
    }
}

impl Updates {
    fn lock_ready(&self) -> std::sync::MutexGuard<'_, Option<Ready>> {
        self.ready.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn set_status(&self, status: Status<'_>) {
        let (text, enabled) = match status {
            Status::UpToDate => ("Обновлений нет — проверить снова".to_owned(), true),
            Status::Checking => ("Проверка обновлений…".to_owned(), false),
            Status::Downloading(v) => (format!("Загрузка версии {v}…"), false),
            Status::Ready(v) => (format!("Установить версию {v}"), true),
            Status::Installing(v) => (format!("Установка версии {v}…"), false),
            Status::Failed => ("Ошибка обновления — повторить".to_owned(), true),
        };
        let result = self
            .menu_item
            .set_text(text)
            .and_then(|()| self.menu_item.set_enabled(enabled));
        if let Err(err) = result {
            tracing::warn!(%err, "не удалось обновить пункт меню");
        }
    }
}
