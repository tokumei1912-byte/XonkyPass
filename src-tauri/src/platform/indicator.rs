//! Полоска-индикатор у края экрана: тонкое окно цвета акцента поверх всех окон.
//! Щелчки проходят сквозь неё, фокус она не берёт. Окно живёт в своём потоке с
//! циклом сообщений, чтобы рассылки Windows всем окнам не ждали поток панели.

use std::sync::mpsc;
use std::thread;

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::CreateSolidBrush;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, HWND_TOPMOST, LWA_ALPHA, MSG,
    RegisterClassW, SW_HIDE, SWP_NOACTIVATE, SWP_NOOWNERZORDER, SWP_SHOWWINDOW,
    SetLayeredWindowAttributes, SetWindowPos, ShowWindow, TranslateMessage, WNDCLASSW,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::w;

use crate::dock::geometry::Rect;

/// Непрозрачность полоски (из 255): заметна, но не режет глаз.
const ALPHA: u8 = 210;

pub struct Indicator {
    hwnd: isize,
}

impl Indicator {
    /// Создаёт окно-полоску (скрытым). `None` — окно создать не удалось.
    pub fn start(color: (u8, u8, u8)) -> Option<Self> {
        let (created, hwnd) = mpsc::channel();
        let spawned = thread::Builder::new()
            .name("dock-indicator".into())
            .spawn(move || {
                // SAFETY: окно создаётся и обслуживается этим потоком.
                let hwnd = unsafe { create(color) };
                let ok = hwnd.is_some();
                let _ = created.send(hwnd.map_or(0, |h| h.0 as isize));
                if ok {
                    // SAFETY: цикл сообщений потока, которому принадлежит окно.
                    unsafe { pump() };
                }
            });
        if let Err(err) = spawned {
            tracing::warn!(%err, "не удалось запустить поток полоски-индикатора");
            return None;
        }
        let hwnd = hwnd.recv().ok().filter(|&h| h != 0)?;
        Some(Self { hwnd })
    }

    pub fn set(&self, strip: Option<Rect>) {
        let hwnd = HWND(self.hwnd as *mut _);
        // SAFETY: окно полоски живёт до конца работы приложения.
        unsafe {
            match strip {
                Some(r) => {
                    let _ = SetWindowPos(
                        hwnd,
                        Some(HWND_TOPMOST),
                        r.left,
                        r.top,
                        r.width(),
                        r.height(),
                        SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_SHOWWINDOW,
                    );
                }
                None => {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
            }
        }
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // Фон рисует Windows кистью класса окна.
    // SAFETY: аргументы пришли от Windows.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// # Safety
/// Вызывать из потока, который будет обслуживать сообщения окна.
unsafe fn create(color: (u8, u8, u8)) -> Option<HWND> {
    let (r, g, b) = color;
    let class = w!("XonkyPassDockIndicator");
    // SAFETY: регистрация класса и создание окна этого процесса; кисть живёт, пока
    // живёт класс окна (до конца работы приложения).
    unsafe {
        let instance = GetModuleHandleW(None).ok()?;
        let brush = CreateSolidBrush(COLORREF(
            u32::from(r) | (u32::from(g) << 8) | (u32::from(b) << 16),
        ));
        let wc = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            lpszClassName: class,
            hbrBackground: brush,
            ..Default::default()
        };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_LAYERED,
            class,
            w!(""),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .inspect_err(|err| tracing::warn!(%err, "не удалось создать полоску-индикатор"))
        .ok()?;
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), ALPHA, LWA_ALPHA);
        Some(hwnd)
    }
}

/// # Safety
/// Вызывать из потока, которому принадлежит окно.
unsafe fn pump() {
    let mut msg = MSG::default();
    // SAFETY: указатель на локальную переменную; 0 — WM_QUIT, -1 — ошибка.
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
