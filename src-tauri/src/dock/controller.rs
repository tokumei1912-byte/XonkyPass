//! Поведение панели: когда выезжать, когда прятаться, куда пристыковаться.
//!
//! Здесь только решения. Окна, курсор и мониторы — за [`Desktop`] (Win32 в
//! `platform::dock`, в тестах — подделка). Время передаётся снаружи.

use std::time::{Duration, Instant};

use serde::Serialize;

use super::geometry::{self, Docked, Monitor, Point, Rect};
use crate::settings::{DockMode, DockSettings};

pub const SHOW_ANIMATION: Duration = Duration::from_millis(150);
pub const HIDE_ANIMATION: Duration = Duration::from_millis(120);
/// Курсор ушёл с панели, открытой наведением, — столько ждём, не вернётся ли.
pub const LEAVE_DELAY: Duration = Duration::from_millis(400);
/// Насколько (логические px) курсор может выйти за панель и всё ещё быть «на ней».
pub const LEAVE_MARGIN: u32 = 8;
/// После потери фокуса — когда смотреть, кто его получил. Своё окно (например,
/// системный диалог сохранения файла) панель не прячет.
pub const FOCUS_CHECK: Duration = Duration::from_millis(100);
/// Щелчок по значку в трее сначала снимает фокус с панели, и она прячется, а сам
/// щелчок приходит позже. Чтобы он не показал панель снова, сразу после такого
/// скрытия щелчок ничего не делает.
pub const TOGGLE_GUARD: Duration = Duration::from_millis(400);
/// Как часто сверять список мониторов (на случай пропущенного сообщения Windows).
pub const MONITOR_CHECK: Duration = Duration::from_secs(2);
/// Как часто спрашивать, не запущено ли полноэкранное приложение, пока видна полоска.
pub const FULLSCREEN_CHECK: Duration = Duration::from_millis(500);
/// Опрос курсора, пока панель ждёт наведения или следит, не ушёл ли курсор.
pub const FAST_TICK: Duration = Duration::from_millis(40);
pub const SLOW_TICK: Duration = Duration::from_millis(500);

/// Всё, что контроллер делает с экраном.
pub trait Desktop {
    fn monitors(&mut self) -> Vec<Monitor>;
    fn cursor(&mut self) -> Option<Point>;
    fn buttons_down(&mut self) -> bool;
    /// Полноэкранное приложение (игра, видео, презентация) или экран блокировки.
    fn fullscreen(&mut self) -> bool;
    /// Панель — активное окно.
    fn is_foreground(&mut self) -> bool;
    /// Активное окно принадлежит этому приложению.
    fn foreground_is_ours(&mut self) -> bool;
    fn window_rect(&mut self) -> Option<Rect>;
    /// Стиль окна для режима: у края — поверх всех окон и без кнопки на панели задач.
    fn set_mode(&mut self, mode: DockMode);
    fn place(&mut self, rect: Rect);
    /// Показать, не забирая фокус у текущего приложения.
    fn show(&mut self);
    fn hide(&mut self);
    fn activate(&mut self);
    /// Плавно сдвинуть окно; `accelerate` — разгон (уезжает), иначе торможение (выезжает).
    fn slide(&mut self, from: Rect, to: Rect, duration: Duration, accelerate: bool);
    fn indicator(&mut self, strip: Option<Rect>);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Показать и дать фокус (трей, повторный запуск, горячая клавиша).
    Show,
    /// Щелчок по значку в трее.
    Toggle,
    /// Esc или закрытие окна.
    Hide,
    Pin(bool),
    /// Открыт диалог или идёт редактирование: панель не прячется сама.
    Hold(bool),
    Settings(DockSettings),
    /// Окно панели стало активным или перестало.
    Activated(bool),
    /// Окно начали или закончили двигать (или тянуть за край) мышью.
    SizeMoveStart,
    SizeMoveEnd {
        resized: bool,
    },
    /// Сменились мониторы, разрешение или рабочая область.
    DisplayChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DockState {
    pub visible: bool,
    pub pinned: bool,
    pub mode: DockMode,
}

/// Что передать приложению.
#[derive(Debug, Clone, PartialEq)]
pub enum Notice {
    /// Показана, спрятана, закреплена или сменился режим — для интерфейса.
    State(DockState),
    /// Панель перетащили к другому краю или изменили размер — сохранить.
    Settings(DockSettings),
}

/// Как панель открыта: наведением (фокус у другого приложения) или явно (щелчок,
/// трей) — от этого зависит, когда она прячется.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shown {
    Hover,
    Active,
}

pub struct Controller<D> {
    desk: D,
    settings: DockSettings,
    monitors: Vec<Monitor>,
    dock: Option<Docked>,
    visible: bool,
    shown: Shown,
    pinned: bool,
    hold: bool,
    moving: bool,
    /// Где было окно, когда начали его тянуть.
    move_from: Option<Rect>,
    /// После скрытия курсор должен уйти из зоны у края, прежде чем она снова сработает.
    armed: bool,
    at_edge_since: Option<Instant>,
    away_since: Option<Instant>,
    /// Фокус ушёл, проверка — через [`FOCUS_CHECK`].
    focus_lost_at: Option<Instant>,
    last_focus_lost: Option<Instant>,
    auto_hidden_at: Option<Instant>,
    monitors_checked: Instant,
    fullscreen: bool,
    fullscreen_checked: Option<Instant>,
    strip: Option<Rect>,
    reported: Option<DockState>,
    notices: Vec<Notice>,
}

impl<D: Desktop> Controller<D> {
    pub fn new(mut desk: D, settings: DockSettings, now: Instant) -> Self {
        let monitors = desk.monitors();
        let mut this = Self {
            desk,
            settings,
            monitors,
            dock: None,
            visible: false,
            shown: Shown::Active,
            pinned: false,
            hold: false,
            moving: false,
            move_from: None,
            armed: false,
            at_edge_since: None,
            away_since: None,
            focus_lost_at: None,
            last_focus_lost: None,
            auto_hidden_at: None,
            monitors_checked: now,
            fullscreen: false,
            fullscreen_checked: None,
            strip: None,
            reported: None,
            notices: Vec::new(),
        };
        this.desk.set_mode(this.settings.mode);
        this.relayout(now);
        this
    }

    pub fn state(&self) -> DockState {
        DockState {
            visible: self.visible,
            pinned: self.pinned,
            mode: self.settings.mode,
        }
    }

    pub fn take_notices(&mut self) -> Vec<Notice> {
        std::mem::take(&mut self.notices)
    }

    #[cfg(test)]
    pub fn desk(&mut self) -> &mut D {
        &mut self.desk
    }

    /// Через сколько снова вызвать [`Self::tick`].
    pub fn next_tick(&self) -> Duration {
        let watching = (!self.visible && self.hover_enabled())
            || (self.visible && self.shown == Shown::Hover)
            || self.focus_lost_at.is_some();
        if watching { FAST_TICK } else { SLOW_TICK }
    }

    pub fn handle(&mut self, command: Command, now: Instant) {
        match command {
            Command::Show => self.show(Shown::Active, now),
            Command::Toggle => self.toggle(now),
            Command::Hide => self.hide(false, now),
            Command::Pin(pinned) => {
                self.pinned = pinned;
                self.report();
            }
            Command::Hold(hold) => self.hold = hold,
            Command::Settings(settings) => self.apply(settings, now),
            Command::Activated(true) => {
                if self.visible {
                    self.shown = Shown::Active;
                }
                self.focus_lost_at = None;
            }
            Command::Activated(false) => {
                self.last_focus_lost = Some(now);
                if self.visible && self.shown == Shown::Active {
                    self.focus_lost_at = Some(now);
                }
            }
            Command::SizeMoveStart => {
                self.moving = true;
                self.move_from = self.desk.window_rect();
            }
            Command::SizeMoveEnd { resized } => {
                self.moving = false;
                self.moved(resized, now);
            }
            Command::DisplayChanged => self.refresh_monitors(now),
        }
    }

    pub fn tick(&mut self, now: Instant) {
        if now.saturating_duration_since(self.monitors_checked) >= MONITOR_CHECK {
            self.refresh_monitors(now);
        }
        if self.moving {
            return;
        }
        if let Some(at) = self.focus_lost_at
            && now.saturating_duration_since(at) >= FOCUS_CHECK
        {
            self.focus_lost_at = None;
            if self.may_hide_itself() && !self.desk.foreground_is_ours() {
                self.hide(true, now);
            }
        }
        if self.visible {
            self.watch_leave(now);
        } else {
            self.watch_edge(now);
        }
        self.update_indicator(now);
    }

    fn hover_enabled(&self) -> bool {
        self.settings.hover_delay_ms.is_some() && self.dock.is_some()
    }

    /// Прятаться самой может только пристыкованная, не закреплённая панель, пока
    /// в ней нет открытого диалога или редактирования.
    fn may_hide_itself(&self) -> bool {
        self.visible && self.settings.mode.docked() && !self.pinned && !self.hold && !self.moving
    }

    /// Где панель должна стоять, когда видна.
    fn target(&self) -> Option<Rect> {
        match &self.dock {
            Some(dock) => Some(dock.rect),
            None => geometry::window(&self.monitors, &self.settings),
        }
    }

    /// Выезд и заезд — только у внешнего края: иначе панель проехала бы по соседнему
    /// монитору.
    fn slides(&self) -> bool {
        self.settings.animate && self.dock.is_some_and(|d| d.outer)
    }

    fn show(&mut self, how: Shown, now: Instant) {
        if self.visible {
            if how == Shown::Active {
                self.desk.activate();
                self.shown = Shown::Active;
            }
            return;
        }
        let Some(target) = self.target() else {
            return;
        };
        let start = match (&self.dock, self.slides()) {
            (Some(dock), true) => dock.hidden,
            _ => target,
        };
        self.desk.place(start);
        self.desk.show();
        if how == Shown::Active {
            self.desk.activate();
        }
        if start != target {
            self.desk.slide(start, target, SHOW_ANIMATION, false);
        }
        self.visible = true;
        self.shown = how;
        self.away_since = None;
        self.focus_lost_at = None;
        self.at_edge_since = None;
        self.armed = false;
        self.update_indicator(now);
        self.report();
    }

    fn hide(&mut self, by_itself: bool, now: Instant) {
        if !self.visible {
            return;
        }
        if self.slides()
            && let Some(dock) = self.dock
        {
            let from = self.desk.window_rect().unwrap_or(dock.rect);
            self.desk.slide(from, dock.hidden, HIDE_ANIMATION, true);
        }
        self.desk.hide();
        self.visible = false;
        self.away_since = None;
        self.focus_lost_at = None;
        self.armed = false;
        self.auto_hidden_at = by_itself.then_some(now);
        self.update_indicator(now);
        self.report();
    }

    fn toggle(&mut self, now: Instant) {
        let recent = |at: Option<Instant>| {
            at.is_some_and(|at| now.saturating_duration_since(at) < TOGGLE_GUARD)
        };
        if self.visible {
            // Панель была впереди (фокус ушёл только что — на сам значок трея): спрятать.
            // Видна, но за другими окнами — вывести вперёд.
            if self.desk.is_foreground() || recent(self.last_focus_lost) {
                self.hide(false, now);
            } else {
                self.show(Shown::Active, now);
            }
        } else if !recent(self.auto_hidden_at) {
            self.show(Shown::Active, now);
        }
    }

    /// Курсор у края: если продержался там нужное время — выехать.
    fn watch_edge(&mut self, now: Instant) {
        let (Some(dock), Some(delay)) = (self.dock, self.settings.hover_delay_ms) else {
            return;
        };
        let at_edge = self
            .desk
            .cursor()
            .is_some_and(|p| geometry::in_trigger_zone(&self.monitors, &dock, p));
        if !at_edge {
            self.armed = true;
            self.at_edge_since = None;
            return;
        }
        if !self.armed {
            return;
        }
        // С зажатой кнопкой пользователь тянет окно или выделяет текст.
        if self.desk.buttons_down() {
            self.at_edge_since = None;
            return;
        }
        let since = *self.at_edge_since.get_or_insert(now);
        if now.saturating_duration_since(since) < Duration::from_millis(u64::from(delay)) {
            return;
        }
        self.at_edge_since = None;
        if self.desk.fullscreen() {
            // Поверх игры или презентации не выезжаем; снова — когда курсор вернётся к краю.
            self.armed = false;
            return;
        }
        self.show(Shown::Hover, now);
    }

    /// Панель открыта наведением: курсор ушёл и не вернулся — спрятать.
    fn watch_leave(&mut self, now: Instant) {
        if self.shown != Shown::Hover || !self.may_hide_itself() {
            self.away_since = None;
            return;
        }
        let (Some(rect), Some(dock)) = (self.target(), self.dock) else {
            return;
        };
        let margin = self
            .monitors
            .get(dock.monitor)
            .map_or(0, |m| m.px(LEAVE_MARGIN));
        let near = self
            .desk
            .cursor()
            .is_some_and(|p| rect.inflate(margin).contains(p));
        if near {
            self.away_since = None;
            return;
        }
        let since = *self.away_since.get_or_insert(now);
        if now.saturating_duration_since(since) >= LEAVE_DELAY {
            self.hide(true, now);
        }
    }

    fn apply(&mut self, settings: DockSettings, now: Instant) {
        if settings.mode != self.settings.mode {
            self.desk.set_mode(settings.mode);
        }
        self.settings = settings;
        self.relayout(now);
    }

    /// Пересчитать положение и, если панель видна, поставить её на место.
    fn relayout(&mut self, now: Instant) {
        self.dock = geometry::docked(&self.monitors, &self.settings);
        if self.visible
            && let Some(target) = self.target()
        {
            self.desk.place(target);
        }
        self.update_indicator(now);
        self.report();
    }

    fn refresh_monitors(&mut self, now: Instant) {
        self.monitors_checked = now;
        let monitors = self.desk.monitors();
        if monitors.is_empty() || monitors == self.monitors {
            return;
        }
        tracing::info!(count = monitors.len(), "мониторы изменились");
        self.monitors = monitors;
        self.relayout(now);
    }

    /// Окно отпустили после перетаскивания или изменения размера.
    fn moved(&mut self, resized: bool, now: Instant) {
        let from = self.move_from.take();
        let Some(rect) = self.desk.window_rect() else {
            return;
        };
        let mut next = self.settings.clone();
        // Клик по заголовку тоже начинает перетаскивание. Если панель почти не сдвинули,
        // настройки не меняются, где бы ни был курсор: ниже она просто вернётся к краю.
        let barely_moved = match (self.dock, from) {
            (Some(dock), Some(from)) => {
                let limit = self
                    .monitors
                    .get(dock.monitor)
                    .map_or(0, |m| m.px(geometry::SNAP));
                (rect.left - from.left).abs() <= limit && (rect.top - from.top).abs() <= limit
            }
            _ => false,
        };
        if resized {
            match self.dock.and_then(|d| self.monitors.get(d.monitor)) {
                Some(monitor) => geometry::resize(&mut next, monitor, &rect),
                None => next.window = Some(geometry::placement(&self.monitors, &rect)),
            }
        } else if !barely_moved {
            let snap = self
                .desk
                .cursor()
                .and_then(|p| geometry::snap(&self.monitors, p));
            match snap {
                Some((index, side)) => {
                    next.mode = side.mode();
                    if geometry::pick(&self.monitors, next.monitor.as_deref()) != Some(index) {
                        next.monitor = Some(self.monitors[index].id.clone());
                    }
                }
                None => {
                    next.mode = DockMode::Window;
                    next.window = Some(geometry::placement(&self.monitors, &rect));
                }
            }
        }
        if next != self.settings {
            self.notices.push(Notice::Settings(next.clone()));
        }
        // Даже без изменений: пристыкованная панель возвращается точно к краю.
        self.apply(next, now);
    }

    fn update_indicator(&mut self, now: Instant) {
        let wanted = self.settings.indicator
            && self.hover_enabled()
            && !self.visible
            && self.dock.is_some_and(|d| d.outer);
        let due = self
            .fullscreen_checked
            .is_none_or(|at| now.saturating_duration_since(at) >= FULLSCREEN_CHECK);
        if wanted && due {
            self.fullscreen = self.desk.fullscreen();
            self.fullscreen_checked = Some(now);
        }
        let strip = match (&self.dock, wanted && !self.fullscreen) {
            (Some(dock), true) => geometry::indicator(&self.monitors, dock),
            _ => None,
        };
        if strip != self.strip {
            self.strip = strip;
            self.desk.indicator(strip);
        }
    }

    fn report(&mut self) {
        let state = self.state();
        if self.reported != Some(state) {
            self.reported = Some(state);
            self.notices.push(Notice::State(state));
        }
    }
}

#[cfg(test)]
mod tests;
