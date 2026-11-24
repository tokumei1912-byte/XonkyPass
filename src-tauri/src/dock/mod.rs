//! Панель у края экрана: выезд от наведения, скрытие, закрепление, пристыковка.
//!
//! Решения принимает [`controller::Controller`] в своём потоке, он же двигает окно
//! (Win32 — `platform::dock`). Остальные части приложения шлют ему [`Command`].
//! На других платформах потока нет: [`send`] возвращает `false`, и окном панели
//! управляет Tauri, как раньше.

// Вне Windows поток панели не запускается: контроллер и геометрия нужны только тестам.
#![cfg_attr(not(windows), allow(dead_code))]

pub mod controller;
pub mod geometry;

use std::sync::mpsc::Sender;
use std::sync::{Mutex, PoisonError};

use serde::Serialize;
use tauri::{AppHandle, Manager};

pub use controller::{Command, DockState};

use crate::settings::DockSettings;

/// Связь с потоком панели.
struct Dock {
    commands: Mutex<Sender<Command>>,
    state: Mutex<DockState>,
}

/// Отправляет команду панели. `false` — потока панели нет (не Windows или он не
/// запустился): вызывающий делает то же через Tauri.
pub fn send(app: &AppHandle, command: Command) -> bool {
    app.try_state::<Dock>().is_some_and(|dock| {
        dock.commands
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .send(command)
            .is_ok()
    })
}

/// Последнее известное состояние панели.
pub fn state(app: &AppHandle, settings: &DockSettings) -> DockState {
    match app.try_state::<Dock>() {
        Some(dock) => *dock.state.lock().unwrap_or_else(PoisonError::into_inner),
        None => DockState {
            visible: true,
            pinned: false,
            mode: settings.mode,
        },
    }
}

/// Закрепить или открепить. Возвращает новое состояние сразу, не дожидаясь потока.
pub fn pin(app: &AppHandle, pinned: bool, settings: &DockSettings) -> DockState {
    send(app, Command::Pin(pinned));
    let mut state = state(app, settings);
    state.pinned = pinned;
    if let Some(dock) = app.try_state::<Dock>() {
        *dock.state.lock().unwrap_or_else(PoisonError::into_inner) = state;
    }
    state
}

/// Монитор для списка в настройках.
#[derive(Debug, Clone, Serialize)]
pub struct MonitorInfo {
    pub id: String,
    /// Номер как в параметрах экрана Windows (из имени `\\.\DISPLAY2`).
    pub number: u32,
    pub primary: bool,
    pub width: i32,
    pub height: i32,
    /// Масштаб в процентах.
    pub scale: u32,
}

pub fn monitors() -> Vec<MonitorInfo> {
    let mut list: Vec<_> = crate::platform::dock::monitors()
        .into_iter()
        .enumerate()
        .map(|(i, m)| {
            let digits: String =
                m.id.chars()
                    .rev()
                    .take_while(char::is_ascii_digit)
                    .collect();
            let digits: String = digits.chars().rev().collect();
            MonitorInfo {
                number: digits.parse().unwrap_or(i as u32 + 1),
                primary: m.primary,
                width: m.rect.width(),
                height: m.rect.height(),
                scale: (m.scale * 100.0).round() as u32,
                id: m.id,
            }
        })
        .collect();
    list.sort_by_key(|m| m.number);
    list
}

/// Запускает поток панели. Вызывать из главного потока (в `setup`): перехват
/// сообщений окна ставится из потока, которому окно принадлежит.
#[cfg(windows)]
pub fn start(app: &AppHandle, settings: DockSettings) {
    use std::sync::mpsc;

    use crate::platform;
    use crate::window_ctl::PANEL;

    let Some(window) = app.get_webview_window(PANEL) else {
        tracing::error!("нет окна панели");
        return;
    };
    let hwnd = platform::window_handle(&window);
    if hwnd == 0 {
        return;
    }
    let (commands, receiver) = mpsc::channel();
    if let Err(err) = platform::dock::watch_window(hwnd, commands.clone()) {
        tracing::error!(%err, "панель у края недоступна");
        return;
    }
    let state = DockState {
        visible: false,
        pinned: false,
        mode: settings.mode,
    };
    let handle = app.clone();
    let spawned = std::thread::Builder::new()
        .name("dock".into())
        .spawn(move || runtime::run(&handle, hwnd, settings, &receiver));
    if let Err(err) = spawned {
        tracing::error!(%err, "не удалось запустить поток панели");
        return;
    }
    app.manage(Dock {
        commands: Mutex::new(commands),
        state: Mutex::new(state),
    });
}

#[cfg(not(windows))]
pub fn start(_app: &AppHandle, _settings: DockSettings) {}

#[cfg(windows)]
mod runtime {
    use std::sync::PoisonError;
    use std::sync::mpsc::{Receiver, RecvTimeoutError};
    use std::time::Instant;

    use tauri::{AppHandle, Emitter, Manager};

    use super::Dock;
    use super::controller::{Command, Controller, Notice};
    use crate::commands::settings::notify;
    use crate::events;
    use crate::platform::dock::Win32Desktop;
    use crate::settings::DockSettings;
    use crate::state::AppState;

    pub fn run(app: &AppHandle, hwnd: isize, settings: DockSettings, commands: &Receiver<Command>) {
        let mut controller = Controller::new(Win32Desktop::new(hwnd), settings, Instant::now());
        tracing::info!("панель у края запущена");
        loop {
            match commands.recv_timeout(controller.next_tick()) {
                Ok(command) => controller.handle(command, Instant::now()),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            while let Ok(command) = commands.try_recv() {
                controller.handle(command, Instant::now());
            }
            controller.tick(Instant::now());
            for notice in controller.take_notices() {
                deliver(app, notice);
            }
        }
    }

    fn deliver(app: &AppHandle, notice: Notice) {
        match notice {
            Notice::State(state) => {
                if let Some(dock) = app.try_state::<Dock>() {
                    *dock.state.lock().unwrap_or_else(PoisonError::into_inner) = state;
                }
                if let Err(err) = app.emit(events::DOCK_STATE, state) {
                    tracing::warn!(%err, "не удалось сообщить окну о панели");
                }
            }
            Notice::Settings(dock) => {
                tracing::info!(mode = ?dock.mode, "панель перетащили или изменили размер");
                match app.state::<AppState>().update_settings(|s| s.dock = dock) {
                    Ok(settings) => notify(app, &settings),
                    Err(err) => tracing::warn!(?err, "не удалось сохранить положение панели"),
                }
            }
        }
    }
}
