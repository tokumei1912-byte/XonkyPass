//! Геометрия панели у края экрана.
//!
//! Всё в физических пикселях виртуального экрана, как в Win32. У каждого монитора
//! свой масштаб: логические размеры из настроек пересчитываются в пиксели того
//! монитора, где стоит панель.

use crate::settings::{DockMode, DockSettings, WindowPlacement};

/// Ширина зоны у самого края (px), где курсор открывает панель.
pub const EDGE_ZONE: i32 = 2;
/// Отступ от углов экрана (логические px): там системные активные углы.
pub const CORNER: u32 = 10;
/// Брошенная ближе этого к краю (логические px) панель пристыковывается к нему.
pub const SNAP: u32 = 20;
/// Толщина полоски-индикатора (логические px).
pub const INDICATOR: u32 = 3;
/// Размер окна по умолчанию (логические px).
pub const WINDOW_SIZE: (u32, u32) = (400, 640);
/// Высота шапки окна (логические px): она должна остаться на экране, чтобы окно
/// можно было перетащить.
const HEADER: u32 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// Прямоугольник; правая и нижняя границы не входят в него (как `RECT` в Win32).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub const fn new(left: i32, top: i32, width: i32, height: i32) -> Self {
        Self {
            left,
            top,
            right: left + width,
            bottom: top + height,
        }
    }

    pub const fn width(&self) -> i32 {
        self.right - self.left
    }

    pub const fn height(&self) -> i32 {
        self.bottom - self.top
    }

    pub const fn contains(&self, p: Point) -> bool {
        p.x >= self.left && p.x < self.right && p.y >= self.top && p.y < self.bottom
    }

    pub const fn inflate(&self, by: i32) -> Self {
        Self {
            left: self.left - by,
            top: self.top - by,
            right: self.right + by,
            bottom: self.bottom + by,
        }
    }

    /// Тот же размер с левым верхним углом в точке.
    pub const fn at(&self, left: i32, top: i32) -> Self {
        Self::new(left, top, self.width(), self.height())
    }

    /// Площадь пересечения.
    pub fn overlap(&self, other: &Self) -> i64 {
        let w = (self.right.min(other.right) - self.left.max(other.left)).max(0);
        let h = (self.bottom.min(other.bottom) - self.top.max(other.top)).max(0);
        i64::from(w) * i64::from(h)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Monitor {
    /// Имя устройства Windows (`\\.\DISPLAY1`).
    pub id: String,
    /// Весь монитор.
    pub rect: Rect,
    /// Без панели задач.
    pub work: Rect,
    /// Масштаб: DPI / 96.
    pub scale: f64,
    pub primary: bool,
}

impl Monitor {
    /// Логические пиксели в физические этого монитора.
    pub fn px(&self, logical: u32) -> i32 {
        (f64::from(logical) * self.scale).round() as i32
    }

    /// Физические пиксели этого монитора в логические.
    pub fn logical(&self, px: i32) -> u32 {
        logical(px, self.scale)
    }
}

fn logical(px: i32, scale: f64) -> u32 {
    (f64::from(px.max(0)) / scale).round() as u32
}

/// Край, к которому пристыкована панель.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
    Top,
}

impl Side {
    pub fn of(mode: DockMode) -> Option<Self> {
        match mode {
            DockMode::Left => Some(Self::Left),
            DockMode::Right => Some(Self::Right),
            DockMode::Top => Some(Self::Top),
            DockMode::Window => None,
        }
    }

    pub fn mode(self) -> DockMode {
        match self {
            Self::Left => DockMode::Left,
            Self::Right => DockMode::Right,
            Self::Top => DockMode::Top,
        }
    }
}

/// Монитор панели: выбранный в настройках, иначе основной, иначе первый.
pub fn pick(monitors: &[Monitor], id: Option<&str>) -> Option<usize> {
    id.and_then(|id| monitors.iter().position(|m| m.id == id))
        .or_else(|| monitors.iter().position(|m| m.primary))
        .or((!monitors.is_empty()).then_some(0))
}

/// Монитор, на котором точка.
pub fn at_point(monitors: &[Monitor], p: Point) -> Option<usize> {
    monitors.iter().position(|m| m.rect.contains(p))
}

/// Монитор, на который больше всего попадает прямоугольник; если ни на какой — основной.
pub fn for_rect(monitors: &[Monitor], rect: &Rect) -> Option<usize> {
    monitors
        .iter()
        .enumerate()
        .map(|(i, m)| (i, m.rect.overlap(rect)))
        .filter(|&(_, area)| area > 0)
        .max_by_key(|&(_, area)| area)
        .map(|(i, _)| i)
        .or_else(|| pick(monitors, None))
}

/// Пристыкованная панель.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Docked {
    pub side: Side,
    /// Номер монитора в списке, по которому посчитано.
    pub monitor: usize,
    /// Панель на месте.
    pub rect: Rect,
    /// Панель целиком за краем монитора: отсюда выезжает и сюда уезжает.
    pub hidden: Rect,
    /// За краем вдоль всей панели нет другого монитора: выезд виден только на этом
    /// мониторе, а полоска-индикатор показывает настоящий край.
    pub outer: bool,
}

/// Где стоит пристыкованная панель; `None` — режим окна или мониторов нет.
pub fn docked(monitors: &[Monitor], settings: &DockSettings) -> Option<Docked> {
    let side = Side::of(settings.mode)?;
    let index = pick(monitors, settings.monitor.as_deref())?;
    let m = &monitors[index];
    let work = m.work;
    let (rect, hidden) = match side {
        Side::Left | Side::Right => {
            let width = m.px(settings.side_width).min(work.width());
            if side == Side::Left {
                let rect = Rect::new(work.left, work.top, width, work.height());
                (rect, rect.at(m.rect.left - width, rect.top))
            } else {
                let rect = Rect::new(work.right - width, work.top, width, work.height());
                (rect, rect.at(m.rect.right, rect.top))
            }
        }
        Side::Top => {
            let width = m.px(settings.top_width).min(work.width());
            let height = m.px(settings.top_height).min(work.height());
            let left = work.left + (work.width() - width) / 2;
            let rect = Rect::new(left, work.top, width, height);
            (rect, rect.at(left, m.rect.top - height))
        }
    };
    let beyond = beyond(side, m, &rect);
    let outer = !monitors
        .iter()
        .enumerate()
        .any(|(i, other)| i != index && other.rect.overlap(&beyond) > 0);
    Some(Docked {
        side,
        monitor: index,
        rect,
        hidden,
        outer,
    })
}

/// Полоса в 1 px сразу за краем монитора вдоль панели.
fn beyond(side: Side, m: &Monitor, panel: &Rect) -> Rect {
    match side {
        Side::Left => Rect {
            left: m.rect.left - 1,
            right: m.rect.left,
            ..*panel
        },
        Side::Right => Rect {
            left: m.rect.right,
            right: m.rect.right + 1,
            ..*panel
        },
        Side::Top => Rect {
            top: m.rect.top - 1,
            bottom: m.rect.top,
            ..*panel
        },
    }
}

/// Курсор там, откуда выезжает панель: у самого края её монитора, напротив панели,
/// не у углов экрана, и за краем в этом месте нет другого монитора (стык двух
/// мониторов не считается — через него курсор просто переходит дальше).
pub fn in_trigger_zone(monitors: &[Monitor], dock: &Docked, p: Point) -> bool {
    let Some(m) = monitors.get(dock.monitor) else {
        return false;
    };
    if !m.rect.contains(p) {
        return false;
    }
    let corner = m.px(CORNER);
    let r = m.rect;
    let panel = dock.rect;
    let (at_edge, along, beyond) = match dock.side {
        Side::Left => (
            p.x < r.left + EDGE_ZONE,
            p.y >= (r.top + corner).max(panel.top) && p.y < (r.bottom - corner).min(panel.bottom),
            Point::new(r.left - 1, p.y),
        ),
        Side::Right => (
            p.x >= r.right - EDGE_ZONE,
            p.y >= (r.top + corner).max(panel.top) && p.y < (r.bottom - corner).min(panel.bottom),
            Point::new(r.right, p.y),
        ),
        Side::Top => (
            p.y < r.top + EDGE_ZONE,
            p.x >= (r.left + corner).max(panel.left) && p.x < (r.right - corner).min(panel.right),
            Point::new(p.x, r.top - 1),
        ),
    };
    at_edge && along && at_point(monitors, beyond).is_none()
}

/// Полоска-индикатор на краю монитора напротив панели.
pub fn indicator(monitors: &[Monitor], dock: &Docked) -> Option<Rect> {
    let m = monitors.get(dock.monitor)?;
    let thickness = m.px(INDICATOR).max(1);
    let panel = dock.rect;
    Some(match dock.side {
        Side::Left => Rect {
            left: m.rect.left,
            right: m.rect.left + thickness,
            ..panel
        },
        Side::Right => Rect {
            left: m.rect.right - thickness,
            right: m.rect.right,
            ..panel
        },
        Side::Top => Rect {
            top: m.rect.top,
            bottom: m.rect.top + thickness,
            ..panel
        },
    })
}

/// К какому краю пристыковать панель, брошенную с курсором в точке `p`: к ближайшему
/// краю монитора (слева, справа или сверху), если курсор ближе [`SNAP`].
pub fn snap(monitors: &[Monitor], p: Point) -> Option<(usize, Side)> {
    let index = at_point(monitors, p)?;
    let m = &monitors[index];
    let limit = m.px(SNAP);
    [
        (Side::Left, p.x - m.rect.left),
        (Side::Right, m.rect.right - 1 - p.x),
        (Side::Top, p.y - m.rect.top),
    ]
    .into_iter()
    .filter(|&(_, distance)| distance < limit)
    .min_by_key(|&(_, distance)| distance)
    .map(|(side, _)| (index, side))
}

/// Где стоит панель в режиме окна: на сохранённом месте, если его шапка видна на
/// каком-нибудь мониторе; иначе по центру монитора панели.
pub fn window(monitors: &[Monitor], settings: &DockSettings) -> Option<Rect> {
    if let Some(saved) = settings.window {
        let corner = Point::new(saved.x, saved.y);
        if let Some(index) = at_point(monitors, corner).or_else(|| pick(monitors, None)) {
            let m = &monitors[index];
            let rect = Rect::new(saved.x, saved.y, m.px(saved.width), m.px(saved.height));
            let header = Point::new(rect.left + rect.width() / 2, rect.top + m.px(HEADER) / 2);
            if monitors.iter().any(|m| m.work.contains(header)) {
                return Some(rect);
            }
        }
    }
    let m = &monitors[pick(monitors, settings.monitor.as_deref())?];
    let width = m.px(WINDOW_SIZE.0).min(m.work.width());
    let height = m.px(WINDOW_SIZE.1).min(m.work.height());
    Some(Rect::new(
        m.work.left + (m.work.width() - width) / 2,
        m.work.top + (m.work.height() - height) / 2,
        width,
        height,
    ))
}

/// Положение окна для настроек: угол как есть, размер — в логических пикселях
/// монитора, на котором окно.
pub fn placement(monitors: &[Monitor], rect: &Rect) -> WindowPlacement {
    let scale = for_rect(monitors, rect).map_or(1.0, |i| monitors[i].scale);
    WindowPlacement {
        x: rect.left,
        y: rect.top,
        width: logical(rect.width(), scale),
        height: logical(rect.height(), scale),
    }
}

/// Новый размер пристыкованной панели, у которой мышью потянули край.
pub fn resize(settings: &mut DockSettings, m: &Monitor, rect: &Rect) {
    match Side::of(settings.mode) {
        Some(Side::Left | Side::Right) => settings.side_width = m.logical(rect.width()),
        Some(Side::Top) => {
            settings.top_width = m.logical(rect.width());
            settings.top_height = m.logical(rect.height());
        }
        None => return,
    }
    *settings = settings.clone().clamped();
}

#[cfg(test)]
mod tests;
