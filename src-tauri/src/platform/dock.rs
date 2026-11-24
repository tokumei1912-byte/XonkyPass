//! Win32 для панели у края: мониторы, курсор, показ без фокуса, выезд, стиль окна
//! и сообщения окна панели.
//!
//! Окно панели создаёт Tauri, но показывает, прячет и двигает его этот модуль:
//! `show()` в Tauri всегда делает окно активным, а панель, выехавшая от наведения,
//! не должна забирать фокус у приложения, в котором печатают. Методы Tauri,
//! меняющие видимость или стиль окна панели, после этого не вызываются: Tauri
//! помнит окно скрытым и при любой такой правке спрятал бы его.

use std::cell::Cell;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DEFAULT, DWMWCP_DONOTROUND, DwmFlush,
    DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, MAPVK_VK_TO_VSC, MapVirtualKeyW, SendInput, VK_LBUTTON,
    VK_LMENU, VK_MBUTTON, VK_MENU, VK_RBUTTON,
};
use windows::Win32::UI::Shell::{
    DefSubclassProc, ITaskbarList, QUNS_BUSY, QUNS_NOT_PRESENT, QUNS_PRESENTATION_MODE,
    QUNS_RUNNING_D3D_FULL_SCREEN, RemoveWindowSubclass, SHQueryUserNotificationState,
    SetWindowSubclass, TaskbarList,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetCursorPos, GetForegroundWindow, GetWindowLongPtrW, GetWindowRect,
    GetWindowThreadProcessId, HWND_NOTOPMOST, HWND_TOPMOST, IsWindowVisible, MONITORINFOF_PRIMARY,
    SET_WINDOW_POS_FLAGS, SPI_SETWORKAREA, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOOWNERZORDER, SWP_NOSIZE, SWP_NOZORDER, SetForegroundWindow, SetWindowLongPtrW,
    SetWindowPos, ShowWindow, WM_ACTIVATE, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_ENTERSIZEMOVE,
    WM_EXITSIZEMOVE, WM_NCDESTROY, WM_SETTINGCHANGE, WM_SIZING, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
};
use windows::core::BOOL;

use super::indicator::Indicator;
use super::{PlatformError, PlatformResult, appearance};
use crate::dock::Command;
use crate::dock::controller::Desktop;
use crate::dock::geometry::{Monitor, Point, Rect};
use crate::settings::DockMode;

/// Цвет полоски, если акцент Windows прочитать не удалось.
const FALLBACK_ACCENT: (u8, u8, u8) = (0x00, 0x67, 0xc0);

/// Мониторы с рабочими областями и масштабом.
pub fn monitors() -> Vec<Monitor> {
    unsafe extern "system" fn each(monitor: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
        // SAFETY: `data` — указатель на список из `monitors()`, живой всё перечисление.
        let list = unsafe { &mut *(data.0 as *mut Vec<Monitor>) };
        // SAFETY: дескриптор монитора от самой Windows.
        if let Some(monitor) = unsafe { describe(monitor) } {
            list.push(monitor);
        }
        true.into()
    }

    let mut list: Vec<Monitor> = Vec::new();
    // SAFETY: функция обратного вызова получает указатель на `list`, который живёт дольше вызова.
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(each), LPARAM(&raw mut list as isize));
    }
    list
}

/// # Safety
/// `monitor` — действительный дескриптор монитора.
unsafe fn describe(monitor: HMONITOR) -> Option<Monitor> {
    let mut info = MONITORINFOEXW {
        monitorInfo: MONITORINFO {
            cbSize: size_of::<MONITORINFOEXW>() as u32,
            ..Default::default()
        },
        szDevice: [0; 32],
    };
    // SAFETY: структура нужного размера, `cbSize` заполнен.
    if !unsafe { GetMonitorInfoW(monitor, &raw mut info.monitorInfo) }.as_bool() {
        return None;
    }
    let (mut dpi_x, mut dpi_y) = (96, 96);
    // SAFETY: указатели на локальные переменные.
    let scale =
        match unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) } {
            Ok(()) if dpi_x > 0 => f64::from(dpi_x) / 96.0,
            _ => 1.0,
        };
    let name = &info.szDevice;
    let len = name.iter().position(|&c| c == 0).unwrap_or(name.len());
    Some(Monitor {
        id: String::from_utf16_lossy(&name[..len]),
        rect: rect(info.monitorInfo.rcMonitor),
        work: rect(info.monitorInfo.rcWork),
        scale,
        primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
    })
}

fn rect(r: RECT) -> Rect {
    Rect {
        left: r.left,
        top: r.top,
        right: r.right,
        bottom: r.bottom,
    }
}

/// Окно панели для [`crate::dock::controller::Controller`]. Живёт в потоке панели.
pub struct Win32Desktop {
    hwnd: isize,
    pid: u32,
    /// У края — поверх всех окон; в режиме окна — как обычное окно.
    topmost: bool,
    taskbar: Option<ITaskbarList>,
    indicator: Option<Indicator>,
}

impl Win32Desktop {
    pub fn new(hwnd: isize) -> Self {
        // COM нужен списку панели задач и цвету акцента (UISettings).
        // SAFETY: инициализация COM для этого потока; повторная безвредна.
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        Self {
            hwnd,
            // SAFETY: без аргументов.
            pid: unsafe { GetCurrentProcessId() },
            topmost: true,
            taskbar: None,
            indicator: None,
        }
    }

    fn hwnd(&self) -> HWND {
        HWND(self.hwnd as *mut _)
    }

    fn set_pos(&self, insert_after: Option<HWND>, rect: Rect, flags: SET_WINDOW_POS_FLAGS) {
        // SAFETY: окно панели живёт всё время работы приложения.
        let result = unsafe {
            SetWindowPos(
                self.hwnd(),
                insert_after,
                rect.left,
                rect.top,
                rect.width(),
                rect.height(),
                flags | SWP_NOACTIVATE | SWP_NOOWNERZORDER,
            )
        };
        if let Err(err) = result {
            tracing::debug!(%err, "SetWindowPos для панели не удался");
        }
    }

    /// Кнопка на панели задач. Windows обновляет её при показе окна; здесь — когда
    /// режим меняется у уже видимой панели.
    fn sync_taskbar(&mut self) {
        let hwnd = self.hwnd();
        // SAFETY: окно панели живо.
        if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
            return;
        }
        if self.taskbar.is_none() {
            // SAFETY: COM инициализирован в `new` для этого потока.
            let list = unsafe {
                CoCreateInstance::<_, ITaskbarList>(&TaskbarList, None, CLSCTX_INPROC_SERVER)
            }
            .and_then(|list| unsafe { list.HrInit() }.map(|()| list));
            match list {
                Ok(list) => self.taskbar = Some(list),
                Err(err) => {
                    tracing::warn!(%err, "нет доступа к панели задач");
                    return;
                }
            }
        }
        if let Some(list) = &self.taskbar {
            // SAFETY: окно панели живо.
            let result = unsafe {
                if self.topmost {
                    list.DeleteTab(hwnd)
                } else {
                    list.AddTab(hwnd)
                }
            };
            if let Err(err) = result {
                tracing::debug!(%err, "кнопка на панели задач не обновилась");
            }
        }
    }
}

impl Desktop for Win32Desktop {
    fn monitors(&mut self) -> Vec<Monitor> {
        monitors()
    }

    fn cursor(&mut self) -> Option<Point> {
        let mut p = POINT::default();
        // SAFETY: указатель на локальную переменную. На экране блокировки вызов падает.
        unsafe { GetCursorPos(&mut p) }.ok()?;
        Some(Point::new(p.x, p.y))
    }

    fn buttons_down(&mut self) -> bool {
        [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON]
            .into_iter()
            // SAFETY: без указателей. Старший бит — клавиша нажата сейчас.
            .any(|key| unsafe { GetAsyncKeyState(i32::from(key.0)) } < 0)
    }

    fn fullscreen(&mut self) -> bool {
        // SAFETY: без указателей.
        match unsafe { SHQueryUserNotificationState() } {
            Ok(state) => [
                QUNS_BUSY,
                QUNS_RUNNING_D3D_FULL_SCREEN,
                QUNS_PRESENTATION_MODE,
                QUNS_NOT_PRESENT,
            ]
            .contains(&state),
            Err(_) => false,
        }
    }

    fn is_foreground(&mut self) -> bool {
        // SAFETY: без аргументов.
        let foreground = unsafe { GetForegroundWindow() };
        foreground == self.hwnd()
    }

    fn foreground_is_ours(&mut self) -> bool {
        // SAFETY: без аргументов.
        let window = unsafe { GetForegroundWindow() };
        if window.is_invalid() {
            return false;
        }
        let mut pid = 0;
        // SAFETY: указатель на локальную переменную.
        unsafe { GetWindowThreadProcessId(window, Some(&mut pid)) };
        pid == self.pid
    }

    fn window_rect(&mut self) -> Option<Rect> {
        let mut r = RECT::default();
        // SAFETY: указатель на локальную переменную.
        unsafe { GetWindowRect(self.hwnd(), &mut r) }.ok()?;
        Some(rect(r))
    }

    fn set_mode(&mut self, mode: DockMode) {
        let hwnd = self.hwnd();
        self.topmost = mode.docked();
        // SAFETY: окно панели живо; атрибут DWM — локальная переменная нужного типа.
        unsafe {
            // У края — «окно инструментов»: без кнопки на панели задач и в Alt+Tab.
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let tool = WS_EX_TOOLWINDOW.0 as isize;
            let app = WS_EX_APPWINDOW.0 as isize;
            let next = if self.topmost {
                (style | tool) & !app
            } else {
                (style & !tool) | app
            };
            if next != style {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next);
            }
            let layer = if self.topmost {
                HWND_TOPMOST
            } else {
                HWND_NOTOPMOST
            };
            let _ = SetWindowPos(
                hwnd,
                Some(layer),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER,
            );
            // Прижатая к краю сбоку панель без скруглений, как системные панели.
            let corners = match mode {
                DockMode::Left | DockMode::Right => DWMWCP_DONOTROUND,
                DockMode::Top | DockMode::Window => DWMWCP_DEFAULT,
            };
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                (&raw const corners).cast(),
                size_of_val(&corners) as u32,
            );
        }
        self.sync_taskbar();
    }

    fn place(&mut self, rect: Rect) {
        let (layer, flags) = if self.topmost {
            (Some(HWND_TOPMOST), SET_WINDOW_POS_FLAGS(0))
        } else {
            (None, SWP_NOZORDER)
        };
        // При переходе на монитор с другим масштабом Windows сама меняет размер окна
        // (WM_DPICHANGED) — тогда ставим ещё раз, масштаб уже новый.
        for _ in 0..3 {
            self.set_pos(layer, rect, flags);
            if self.window_rect() == Some(rect) {
                break;
            }
        }
    }

    fn show(&mut self) {
        // SAFETY: окно панели живо.
        unsafe {
            let _ = ShowWindow(self.hwnd(), SW_SHOWNOACTIVATE);
        }
    }

    fn hide(&mut self) {
        // SAFETY: окно панели живо.
        unsafe {
            let _ = ShowWindow(self.hwnd(), SW_HIDE);
        }
    }

    fn activate(&mut self) {
        let hwnd = self.hwnd();
        // SAFETY: окно панели живо; SendInput получает массив локальных структур.
        unsafe {
            if SetForegroundWindow(hwnd).as_bool() {
                return;
            }
            // Фоновому процессу Windows не даёт забрать фокус; нажатие Alt снимает
            // запрет (так же делает Tauri в `set_focus`).
            let scan = MapVirtualKeyW(u32::from(VK_MENU.0), MAPVK_VK_TO_VSC) as u16;
            let key = |flags: KEYBD_EVENT_FLAGS| INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_LMENU,
                        wScan: scan,
                        dwFlags: flags,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            let inputs = [
                key(KEYEVENTF_EXTENDEDKEY),
                key(KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP),
            ];
            SendInput(&inputs, size_of::<INPUT>() as i32);
            let _ = SetForegroundWindow(hwnd);
        }
    }

    fn slide(&mut self, from: Rect, to: Rect, duration: Duration, accelerate: bool) {
        let start = Instant::now();
        let total = duration.as_secs_f64().max(0.001);
        loop {
            let t = (start.elapsed().as_secs_f64() / total).min(1.0);
            // Выезд тормозит к концу, заезд разгоняется.
            let k = if accelerate {
                t * t * t
            } else {
                1.0 - (1.0 - t).powi(3)
            };
            let lerp = |a: i32, b: i32| a + (f64::from(b - a) * k).round() as i32;
            let frame = to.at(lerp(from.left, to.left), lerp(from.top, to.top));
            self.set_pos(None, frame, SWP_NOSIZE | SWP_NOZORDER);
            if t >= 1.0 {
                break;
            }
            // Следующий кадр — после очередного обновления экрана.
            // SAFETY: без аргументов.
            if unsafe { DwmFlush() }.is_err() {
                std::thread::sleep(Duration::from_millis(8));
            }
        }
    }

    fn indicator(&mut self, strip: Option<Rect>) {
        if self.indicator.is_none() {
            if strip.is_none() {
                return;
            }
            let color = appearance::accent_rgb().unwrap_or(FALLBACK_ACCENT);
            self.indicator = Indicator::start(color);
        }
        if let Some(indicator) = &self.indicator {
            indicator.set(strip);
        }
    }
}

/// Данные перехвата сообщений окна панели (главный поток).
struct Hook {
    commands: Sender<Command>,
    /// Окно тянули за край, а не за шапку.
    resized: Cell<bool>,
}

const HOOK_ID: usize = 0x5850_4443; // «XPDC»

/// Подписывается на сообщения окна панели: активация, перетаскивание и изменение
/// размера мышью, смена мониторов и рабочей области. Вызывать из главного потока —
/// того, которому принадлежит окно.
pub fn watch_window(hwnd: isize, commands: Sender<Command>) -> PlatformResult<()> {
    let hook = Box::into_raw(Box::new(Hook {
        commands,
        resized: Cell::new(false),
    }));
    // SAFETY: окно принадлежит этому потоку; `hook` освобождается в WM_NCDESTROY.
    let ok = unsafe {
        SetWindowSubclass(
            HWND(hwnd as *mut _),
            Some(hook_proc),
            HOOK_ID,
            hook as usize,
        )
    };
    if !ok.as_bool() {
        // SAFETY: указатель только что получен из `Box::into_raw` и никуда не передан.
        drop(unsafe { Box::from_raw(hook) });
        return Err(PlatformError(
            "не удалось подписаться на сообщения окна панели".into(),
        ));
    }
    Ok(())
}

unsafe extern "system" fn hook_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    if msg == WM_NCDESTROY {
        // SAFETY: `data` — `Box<Hook>` из `watch_window`; после снятия перехвата
        // он больше не используется.
        unsafe {
            let _ = RemoveWindowSubclass(hwnd, Some(hook_proc), HOOK_ID);
            drop(Box::from_raw(data as *mut Hook));
            return DefSubclassProc(hwnd, msg, wparam, lparam);
        }
    }
    // SAFETY: `data` — живой `Box<Hook>` (освобождается только в WM_NCDESTROY).
    let hook = unsafe { &*(data as *const Hook) };
    let command = match msg {
        // Младшее слово: WA_INACTIVE (0) — окно перестало быть активным.
        WM_ACTIVATE => Some(Command::Activated(wparam.0 & 0xffff != 0)),
        WM_ENTERSIZEMOVE => {
            hook.resized.set(false);
            Some(Command::SizeMoveStart)
        }
        WM_SIZING => {
            hook.resized.set(true);
            None
        }
        WM_EXITSIZEMOVE => Some(Command::SizeMoveEnd {
            resized: hook.resized.get(),
        }),
        WM_DISPLAYCHANGE | WM_DPICHANGED => Some(Command::DisplayChanged),
        WM_SETTINGCHANGE if wparam.0 == SPI_SETWORKAREA.0 as usize => Some(Command::DisplayChanged),
        _ => None,
    };
    if let Some(command) = command {
        // Поток панели завершился — сообщать некому.
        let _ = hook.commands.send(command);
    }
    // SAFETY: те же аргументы, что пришли в обработчик.
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}
