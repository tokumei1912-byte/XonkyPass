use std::time::{Duration, Instant};

use super::*;
use crate::dock::geometry::{Monitor, Point, Rect};
use crate::settings::WindowPlacement;

/// Рабочий стол в памяти: запоминает, что с ним делали.
#[derive(Default)]
struct Fake {
    monitors: Vec<Monitor>,
    cursor: Option<Point>,
    buttons: bool,
    fullscreen: bool,
    foreground: bool,
    foreground_ours: bool,
    rect: Option<Rect>,
    visible: bool,
    mode: Option<DockMode>,
    strip: Option<Rect>,
    slides: u32,
}

impl Desktop for Fake {
    fn monitors(&mut self) -> Vec<Monitor> {
        self.monitors.clone()
    }
    fn cursor(&mut self) -> Option<Point> {
        self.cursor
    }
    fn buttons_down(&mut self) -> bool {
        self.buttons
    }
    fn fullscreen(&mut self) -> bool {
        self.fullscreen
    }
    fn is_foreground(&mut self) -> bool {
        self.foreground
    }
    fn foreground_is_ours(&mut self) -> bool {
        self.foreground_ours
    }
    fn window_rect(&mut self) -> Option<Rect> {
        self.rect
    }
    fn set_mode(&mut self, mode: DockMode) {
        self.mode = Some(mode);
    }
    fn place(&mut self, rect: Rect) {
        self.rect = Some(rect);
    }
    fn show(&mut self) {
        self.visible = true;
    }
    fn hide(&mut self) {
        self.visible = false;
        self.foreground = false;
    }
    fn activate(&mut self) {
        self.foreground = true;
    }
    fn slide(&mut self, _from: Rect, to: Rect, _duration: Duration, _accelerate: bool) {
        self.rect = Some(to);
        self.slides += 1;
    }
    fn indicator(&mut self, strip: Option<Rect>) {
        self.strip = strip;
    }
}

fn monitor(id: &str, rect: Rect, scale: f64, primary: bool) -> Monitor {
    Monitor {
        id: id.into(),
        rect,
        work: Rect {
            bottom: rect.bottom - 40,
            ..rect
        },
        scale,
        primary,
    }
}

fn single() -> Vec<Monitor> {
    vec![monitor("D1", Rect::new(0, 0, 1920, 1080), 1.0, true)]
}

fn dual() -> Vec<Monitor> {
    vec![
        monitor("D1", Rect::new(0, 0, 1920, 1080), 1.0, true),
        monitor("D2", Rect::new(1920, 0, 1920, 1080), 1.0, false),
    ]
}

const LEFT_PANEL: Rect = Rect::new(0, 0, 400, 1040);

struct Test {
    ctl: Controller<Fake>,
    now: Instant,
}

impl Test {
    fn with(monitors: Vec<Monitor>, settings: DockSettings) -> Self {
        let now = Instant::now();
        let fake = Fake {
            monitors,
            cursor: Some(Point::new(900, 500)),
            ..Fake::default()
        };
        let mut test = Self {
            ctl: Controller::new(fake, settings, now),
            now,
        };
        test.wait(10);
        test
    }

    fn new(mode: DockMode) -> Self {
        Self::with(
            single(),
            DockSettings {
                mode,
                ..DockSettings::default()
            },
        )
    }

    fn fake(&mut self) -> &mut Fake {
        self.ctl.desk()
    }

    fn cursor(&mut self, x: i32, y: i32) {
        self.fake().cursor = Some(Point::new(x, y));
    }

    fn wait(&mut self, ms: u64) {
        let end = self.now + Duration::from_millis(ms);
        while self.now < end {
            self.now += Duration::from_millis(10);
            self.ctl.tick(self.now);
        }
    }

    fn send(&mut self, command: Command) {
        self.ctl.handle(command, self.now);
    }

    fn visible(&mut self) -> bool {
        self.fake().visible
    }

    /// Открыть наведением на левый край.
    fn hover_open(&mut self) {
        self.cursor(0, 500);
        self.wait(250);
        assert!(self.visible());
        self.cursor(200, 500);
    }

    fn saved_settings(&mut self) -> Vec<DockSettings> {
        self.ctl
            .take_notices()
            .into_iter()
            .filter_map(|n| match n {
                Notice::Settings(s) => Some(s),
                Notice::State(_) => None,
            })
            .collect()
    }
}

#[test]
fn opens_after_resting_at_the_edge_without_taking_focus() {
    let mut t = Test::new(DockMode::Left);
    t.cursor(0, 500);
    t.wait(150);
    assert!(!t.visible());
    t.wait(100);
    assert!(t.visible());
    assert!(!t.fake().foreground);
    assert_eq!(t.fake().rect, Some(LEFT_PANEL));
    assert_eq!(t.fake().slides, 1);
}

#[test]
fn brief_touch_or_pressed_button_does_not_open() {
    let mut t = Test::new(DockMode::Left);
    t.cursor(0, 500);
    t.wait(100);
    t.cursor(50, 500);
    t.wait(20);
    t.cursor(0, 500);
    t.wait(100);
    assert!(!t.visible());

    t.fake().buttons = true;
    t.wait(500);
    assert!(!t.visible());
    t.fake().buttons = false;
    t.wait(250);
    assert!(t.visible());
}

#[test]
fn fullscreen_app_blocks_until_the_cursor_comes_back() {
    let mut t = Test::new(DockMode::Left);
    t.fake().fullscreen = true;
    t.cursor(0, 500);
    t.wait(500);
    assert!(!t.visible());
    t.fake().fullscreen = false;
    t.wait(500);
    assert!(!t.visible(), "курсор не уходил от края");
    t.cursor(100, 500);
    t.wait(20);
    t.cursor(0, 500);
    t.wait(250);
    assert!(t.visible());
}

#[test]
fn hover_panel_hides_when_the_cursor_leaves() {
    let mut t = Test::new(DockMode::Left);
    t.hover_open();
    // В пределах 8 px от панели курсор ещё «на ней».
    t.cursor(405, 500);
    t.wait(1000);
    assert!(t.visible());

    t.cursor(900, 500);
    t.wait(300);
    t.cursor(300, 500);
    t.wait(1000);
    assert!(t.visible(), "курсор вернулся вовремя");

    t.cursor(900, 500);
    t.wait(350);
    assert!(t.visible());
    t.wait(100);
    assert!(!t.visible());
    assert_eq!(t.fake().slides, 2);
}

#[test]
fn reopening_needs_the_cursor_to_leave_the_edge_first() {
    let mut t = Test::new(DockMode::Left);
    t.cursor(0, 500);
    t.send(Command::Show);
    t.send(Command::Hide);
    t.wait(1000);
    assert!(!t.visible());
    t.cursor(100, 500);
    t.wait(20);
    t.cursor(0, 500);
    t.wait(250);
    assert!(t.visible());
}

#[test]
fn clicked_panel_stays_until_focus_goes_elsewhere() {
    let mut t = Test::new(DockMode::Left);
    t.hover_open();
    t.send(Command::Activated(true));
    t.cursor(1500, 500);
    t.wait(2000);
    assert!(t.visible());

    t.send(Command::Activated(false));
    t.wait(50);
    assert!(t.visible());
    t.wait(100);
    assert!(!t.visible());
}

#[test]
fn own_dialog_keeps_the_panel() {
    let mut t = Test::new(DockMode::Left);
    t.send(Command::Show);
    t.fake().foreground_ours = true;
    t.send(Command::Activated(false));
    t.wait(500);
    assert!(t.visible());
}

#[test]
fn pin_and_hold_stop_hiding_by_itself() {
    let mut t = Test::new(DockMode::Left);
    t.send(Command::Pin(true));
    t.hover_open();
    t.cursor(1500, 500);
    t.wait(1000);
    assert!(t.visible());
    t.send(Command::Activated(true));
    t.send(Command::Activated(false));
    t.wait(500);
    assert!(t.visible());
    // Esc прячет и закреплённую панель.
    t.send(Command::Hide);
    assert!(!t.visible());

    t.send(Command::Pin(false));
    t.send(Command::Show);
    t.send(Command::Hold(true));
    t.send(Command::Activated(false));
    t.wait(500);
    assert!(t.visible());
    t.send(Command::Hold(false));
    t.send(Command::Activated(true));
    t.send(Command::Activated(false));
    t.wait(200);
    assert!(!t.visible());
}

#[test]
fn tray_click_toggles() {
    let mut t = Test::new(DockMode::Left);
    t.send(Command::Toggle);
    assert!(t.visible());
    assert!(t.fake().foreground);

    // Щелчок по трею: сначала панель теряет фокус и прячется сама...
    t.fake().foreground = false;
    t.send(Command::Activated(false));
    t.wait(150);
    assert!(!t.visible());
    // ...потом приходит сам щелчок — показывать снова не надо.
    t.send(Command::Toggle);
    assert!(!t.visible());

    t.wait(500);
    t.send(Command::Toggle);
    assert!(t.visible());

    // Щелчок пришёл раньше, чем панель успела спрятаться.
    t.fake().foreground = false;
    t.send(Command::Activated(false));
    t.wait(30);
    t.send(Command::Toggle);
    assert!(!t.visible());
    t.wait(500);
    assert!(!t.visible());
}

#[test]
fn window_mode_behaves_like_a_normal_window() {
    let mut t = Test::new(DockMode::Window);
    assert_eq!(t.fake().mode, Some(DockMode::Window));
    t.cursor(0, 500);
    t.wait(1000);
    assert!(!t.visible(), "у окна нет выезда от края");

    t.send(Command::Toggle);
    assert!(t.visible());
    assert_eq!(t.fake().rect, Some(Rect::new(760, 200, 400, 640)));
    assert_eq!(t.fake().slides, 0);

    // Ушли в другое окно — панель остаётся; щелчок по трею выводит её вперёд.
    t.fake().foreground = false;
    t.send(Command::Activated(false));
    t.wait(1000);
    assert!(t.visible());
    t.send(Command::Toggle);
    assert!(t.visible());
    assert!(t.fake().foreground);
}

#[test]
fn no_slide_at_an_inner_edge_or_when_animation_is_off() {
    let settings = DockSettings {
        mode: DockMode::Right,
        monitor: Some("D1".into()),
        ..DockSettings::default()
    };
    let mut t = Test::with(dual(), settings);
    t.send(Command::Show);
    assert_eq!(t.fake().slides, 0);
    assert_eq!(t.fake().rect, Some(Rect::new(1520, 0, 400, 1040)));
    // У края между мониторами наведение не срабатывает.
    t.cursor(1919, 500);
    t.send(Command::Hide);
    t.wait(1000);
    assert!(!t.visible());

    let settings = DockSettings {
        animate: false,
        ..DockSettings::default()
    };
    let mut t = Test::with(single(), settings);
    t.send(Command::Show);
    t.send(Command::Hide);
    assert_eq!(t.fake().slides, 0);
}

#[test]
fn hover_can_be_turned_off() {
    let settings = DockSettings {
        hover_delay_ms: None,
        ..DockSettings::default()
    };
    let mut t = Test::with(single(), settings);
    assert_eq!(t.ctl.next_tick(), SLOW_TICK);
    t.cursor(0, 500);
    t.wait(1000);
    assert!(!t.visible());
    t.send(Command::Show);
    assert!(t.visible());
}

#[test]
fn dropping_near_an_edge_docks_there() {
    let mut t = Test::new(DockMode::Window);
    t.send(Command::Show);
    t.send(Command::SizeMoveStart);
    t.fake().rect = Some(Rect::new(1500, 300, 400, 640));
    t.cursor(1912, 320);
    t.send(Command::SizeMoveEnd { resized: false });

    let saved = t.saved_settings();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].mode, DockMode::Right);
    // Основной монитор так и остаётся «основным», без имени.
    assert_eq!(saved[0].monitor, None);
    assert_eq!(t.fake().mode, Some(DockMode::Right));
    assert_eq!(t.fake().rect, Some(Rect::new(1520, 0, 400, 1040)));
}

#[test]
fn dropping_on_another_monitor_remembers_it() {
    let mut t = Test::with(dual(), DockSettings::default());
    t.send(Command::Show);
    t.send(Command::SizeMoveStart);
    t.fake().rect = Some(Rect::new(1925, 300, 400, 1040));
    t.cursor(1930, 320);
    t.send(Command::SizeMoveEnd { resized: false });
    let saved = t.saved_settings();
    assert_eq!(saved[0].mode, DockMode::Left);
    assert_eq!(saved[0].monitor.as_deref(), Some("D2"));
    assert_eq!(t.fake().rect, Some(Rect::new(1920, 0, 400, 1040)));
}

#[test]
fn dragging_away_from_the_edge_makes_a_window() {
    let mut t = Test::new(DockMode::Left);
    t.send(Command::Show);
    t.send(Command::SizeMoveStart);
    t.fake().rect = Some(Rect::new(700, 100, 400, 1040));
    t.cursor(900, 120);
    t.send(Command::SizeMoveEnd { resized: false });

    let saved = t.saved_settings();
    assert_eq!(saved[0].mode, DockMode::Window);
    assert_eq!(
        saved[0].window,
        Some(WindowPlacement {
            x: 700,
            y: 100,
            width: 400,
            height: 1040
        })
    );
    assert_eq!(t.fake().mode, Some(DockMode::Window));
    assert_eq!(t.fake().rect, Some(Rect::new(700, 100, 400, 1040)));
    // Окно само не прячется.
    t.cursor(1500, 900);
    t.wait(1000);
    assert!(t.visible());
}

#[test]
fn clicking_the_header_keeps_the_panel_docked() {
    let mut t = Test::new(DockMode::Left);
    t.send(Command::Show);
    t.send(Command::SizeMoveStart);
    // Курсор далеко от края, но окно не сдвинулось (или сдвинулось на пару пикселей).
    t.fake().rect = Some(Rect::new(6, 4, 400, 1040));
    t.cursor(300, 12);
    t.send(Command::SizeMoveEnd { resized: false });
    assert!(t.saved_settings().is_empty());
    assert_eq!(t.fake().mode, Some(DockMode::Left));
    assert_eq!(t.fake().rect, Some(LEFT_PANEL));
}

#[test]
fn small_drag_snaps_back_to_the_same_edge() {
    let mut t = Test::new(DockMode::Left);
    t.send(Command::Show);
    t.send(Command::SizeMoveStart);
    t.fake().rect = Some(Rect::new(12, 30, 400, 1040));
    t.cursor(8, 40);
    t.send(Command::SizeMoveEnd { resized: false });
    assert!(t.saved_settings().is_empty());
    assert_eq!(t.fake().rect, Some(LEFT_PANEL));
}

#[test]
fn hiding_waits_for_the_drag_to_finish() {
    let mut t = Test::new(DockMode::Left);
    t.hover_open();
    t.send(Command::SizeMoveStart);
    t.cursor(1500, 500);
    t.wait(1000);
    assert!(t.visible());
}

#[test]
fn resizing_the_inner_edge_changes_the_width() {
    let mut t = Test::new(DockMode::Left);
    t.send(Command::Show);
    t.send(Command::SizeMoveStart);
    // Тянули ещё и сверху — высота всё равно вернётся к рабочей области.
    t.fake().rect = Some(Rect::new(0, 200, 520, 840));
    t.send(Command::SizeMoveEnd { resized: true });
    let saved = t.saved_settings();
    assert_eq!(saved[0].side_width, 520);
    assert_eq!(t.fake().rect, Some(Rect::new(0, 0, 520, 1040)));
}

#[test]
fn panel_moves_to_primary_when_its_monitor_is_gone() {
    let settings = DockSettings {
        mode: DockMode::Left,
        monitor: Some("D2".into()),
        ..DockSettings::default()
    };
    let mut t = Test::with(dual(), settings);
    t.send(Command::Show);
    assert_eq!(t.fake().rect, Some(Rect::new(1920, 0, 400, 1040)));
    t.fake().monitors = single();
    t.send(Command::DisplayChanged);
    assert_eq!(t.fake().rect, Some(LEFT_PANEL));
    // Монитор вернулся — панель тоже.
    t.fake().monitors = dual();
    t.wait(2100);
    assert_eq!(t.fake().rect, Some(Rect::new(1920, 0, 400, 1040)));
}

#[test]
fn indicator_shows_while_hidden_and_not_over_fullscreen_apps() {
    let settings = DockSettings {
        indicator: true,
        ..DockSettings::default()
    };
    let mut t = Test::with(single(), settings);
    let strip = Some(Rect::new(0, 0, 3, 1040));
    assert_eq!(t.fake().strip, strip);
    t.send(Command::Show);
    assert_eq!(t.fake().strip, None);
    t.send(Command::Hide);
    assert_eq!(t.fake().strip, strip);

    t.fake().fullscreen = true;
    t.wait(600);
    assert_eq!(t.fake().strip, None);
    t.fake().fullscreen = false;
    t.wait(600);
    assert_eq!(t.fake().strip, strip);
}

#[test]
fn state_changes_are_reported_once() {
    let mut t = Test::new(DockMode::Left);
    let _ = t.ctl.take_notices();
    t.send(Command::Show);
    t.send(Command::Show);
    t.send(Command::Pin(true));
    t.send(Command::Pin(true));
    let states: Vec<_> = t
        .ctl
        .take_notices()
        .into_iter()
        .filter_map(|n| match n {
            Notice::State(s) => Some((s.visible, s.pinned)),
            Notice::Settings(_) => None,
        })
        .collect();
    assert_eq!(states, vec![(true, false), (true, true)]);
}
