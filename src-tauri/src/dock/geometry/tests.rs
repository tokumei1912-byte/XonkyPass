use super::*;

fn monitor(id: &str, rect: Rect, taskbar: i32, scale: f64, primary: bool) -> Monitor {
    Monitor {
        id: id.into(),
        rect,
        work: Rect {
            bottom: rect.bottom - taskbar,
            ..rect
        },
        scale,
        primary,
    }
}

/// Один монитор 1920×1080, панель задач снизу 40 px.
fn single() -> Vec<Monitor> {
    vec![monitor("D1", Rect::new(0, 0, 1920, 1080), 40, 1.0, true)]
}

/// Основной 1920×1080 и справа от него 2560×1440 с масштабом 150 %.
fn dual() -> Vec<Monitor> {
    vec![
        monitor("D1", Rect::new(0, 0, 1920, 1080), 40, 1.0, true),
        monitor("D2", Rect::new(1920, 0, 2560, 1440), 0, 1.5, false),
    ]
}

fn settings(mode: DockMode) -> DockSettings {
    DockSettings {
        mode,
        ..DockSettings::default()
    }
}

fn on(mode: DockMode, monitor: &str) -> DockSettings {
    DockSettings {
        monitor: Some(monitor.into()),
        ..settings(mode)
    }
}

#[test]
fn left_panel_fills_the_work_area_height() {
    let dock = docked(&single(), &settings(DockMode::Left)).unwrap();
    assert_eq!(dock.side, Side::Left);
    assert_eq!(dock.rect, Rect::new(0, 0, 400, 1040));
    assert_eq!(dock.hidden, Rect::new(-400, 0, 400, 1040));
    assert!(dock.outer);
}

#[test]
fn right_and_top_panels() {
    let right = docked(&single(), &settings(DockMode::Right)).unwrap();
    assert_eq!(right.rect, Rect::new(1520, 0, 400, 1040));
    assert_eq!(right.hidden, Rect::new(1920, 0, 400, 1040));

    let top = docked(&single(), &settings(DockMode::Top)).unwrap();
    assert_eq!(top.rect, Rect::new(600, 0, 720, 520));
    assert_eq!(top.hidden, Rect::new(600, -520, 720, 520));
    assert!(top.outer);
}

#[test]
fn window_mode_is_not_docked() {
    assert_eq!(docked(&single(), &settings(DockMode::Window)), None);
    assert_eq!(docked(&[], &settings(DockMode::Left)), None);
}

#[test]
fn size_follows_the_monitor_scale() {
    let dock = docked(&dual(), &on(DockMode::Left, "D2")).unwrap();
    assert_eq!(dock.monitor, 1);
    assert_eq!(dock.rect, Rect::new(1920, 0, 600, 1440));
}

#[test]
fn panel_never_exceeds_the_work_area() {
    let small = vec![monitor("S", Rect::new(0, 0, 800, 600), 40, 1.0, true)];
    let top = DockSettings {
        top_width: 1600,
        top_height: 1200,
        ..settings(DockMode::Top)
    };
    assert_eq!(
        docked(&small, &top).unwrap().rect,
        Rect::new(0, 0, 800, 560)
    );
}

#[test]
fn missing_monitor_falls_back_to_primary() {
    let dock = docked(&dual(), &on(DockMode::Right, "gone")).unwrap();
    assert_eq!(dock.monitor, 0);
}

#[test]
fn edge_between_monitors_is_not_outer() {
    let monitors = dual();
    let right = docked(&monitors, &on(DockMode::Right, "D1")).unwrap();
    assert!(!right.outer);
    assert!(!in_trigger_zone(&monitors, &right, Point::new(1919, 500)));

    let left = docked(&monitors, &on(DockMode::Left, "D2")).unwrap();
    assert!(!left.outer);
    assert!(!in_trigger_zone(&monitors, &left, Point::new(1920, 500)));

    // Правый край второго монитора — внешний.
    let outer = docked(&monitors, &on(DockMode::Right, "D2")).unwrap();
    assert!(outer.outer);
    assert!(in_trigger_zone(&monitors, &outer, Point::new(4479, 700)));
}

#[test]
fn trigger_zone_is_the_outermost_pixels() {
    let monitors = single();
    let dock = docked(&monitors, &settings(DockMode::Left)).unwrap();
    assert!(in_trigger_zone(&monitors, &dock, Point::new(0, 500)));
    assert!(in_trigger_zone(&monitors, &dock, Point::new(1, 500)));
    assert!(!in_trigger_zone(&monitors, &dock, Point::new(2, 500)));
    assert!(!in_trigger_zone(&monitors, &dock, Point::new(-1, 500)));
}

#[test]
fn corners_and_taskbar_do_not_trigger() {
    let monitors = single();
    let dock = docked(&monitors, &settings(DockMode::Left)).unwrap();
    assert!(!in_trigger_zone(&monitors, &dock, Point::new(0, 5)));
    assert!(in_trigger_zone(&monitors, &dock, Point::new(0, 10)));
    // Напротив панели задач панели нет.
    assert!(!in_trigger_zone(&monitors, &dock, Point::new(0, 1050)));
}

#[test]
fn top_zone_is_limited_to_the_panel_width() {
    let monitors = single();
    let dock = docked(&monitors, &settings(DockMode::Top)).unwrap();
    assert!(in_trigger_zone(&monitors, &dock, Point::new(600, 0)));
    assert!(in_trigger_zone(&monitors, &dock, Point::new(1319, 1)));
    assert!(!in_trigger_zone(&monitors, &dock, Point::new(599, 0)));
    assert!(!in_trigger_zone(&monitors, &dock, Point::new(1320, 0)));
}

#[test]
fn partly_covered_edge_triggers_only_where_it_is_outer() {
    // Слева от основного — небольшой монитор, закрывающий верх левого края.
    let monitors = vec![
        monitor("D1", Rect::new(0, 0, 1920, 1080), 40, 1.0, true),
        monitor("D2", Rect::new(-1024, 0, 1024, 500), 0, 1.0, false),
    ];
    let dock = docked(&monitors, &settings(DockMode::Left)).unwrap();
    assert!(!dock.outer);
    assert!(!in_trigger_zone(&monitors, &dock, Point::new(0, 300)));
    assert!(in_trigger_zone(&monitors, &dock, Point::new(0, 800)));
}

#[test]
fn indicator_runs_along_the_panel() {
    let monitors = single();
    let left = docked(&monitors, &settings(DockMode::Left)).unwrap();
    assert_eq!(indicator(&monitors, &left), Some(Rect::new(0, 0, 3, 1040)));
    let top = docked(&monitors, &settings(DockMode::Top)).unwrap();
    assert_eq!(indicator(&monitors, &top), Some(Rect::new(600, 0, 720, 3)));

    let monitors = dual();
    let right = docked(&monitors, &on(DockMode::Right, "D2")).unwrap();
    assert_eq!(
        indicator(&monitors, &right),
        Some(Rect::new(4475, 0, 5, 1440))
    );
}

#[test]
fn dropped_panel_snaps_to_the_nearest_edge() {
    let monitors = dual();
    assert_eq!(snap(&monitors, Point::new(5, 500)), Some((0, Side::Left)));
    assert_eq!(
        snap(&monitors, Point::new(1915, 500)),
        Some((0, Side::Right))
    );
    assert_eq!(snap(&monitors, Point::new(900, 3)), Some((0, Side::Top)));
    assert_eq!(snap(&monitors, Point::new(900, 500)), None);
    // В углу — к ближайшему краю.
    assert_eq!(snap(&monitors, Point::new(5, 3)), Some((0, Side::Top)));
    // 20 логических px при масштабе 150 % — 30 физических.
    assert_eq!(
        snap(&monitors, Point::new(1945, 700)),
        Some((1, Side::Left))
    );
    assert_eq!(snap(&monitors, Point::new(1955, 700)), None);
    // Нижний край не используется.
    assert_eq!(snap(&monitors, Point::new(900, 1075)), None);
}

#[test]
fn window_is_centered_by_default() {
    let rect = window(&single(), &settings(DockMode::Window)).unwrap();
    assert_eq!(rect, Rect::new(760, 200, 400, 640));
}

#[test]
fn saved_window_position_is_kept_while_visible() {
    let mut s = settings(DockMode::Window);
    s.window = Some(WindowPlacement {
        x: 2000,
        y: 100,
        width: 420,
        height: 700,
    });
    // На втором мониторе (150 %) размер в физических пикселях больше.
    assert_eq!(
        window(&dual(), &s).unwrap(),
        Rect::new(2000, 100, 630, 1050)
    );
    // Второй монитор отключили — окно не должно остаться за пределами экрана.
    assert_eq!(
        window(&single(), &s).unwrap(),
        Rect::new(760, 200, 400, 640)
    );
}

#[test]
fn placement_is_stored_in_logical_size() {
    let monitors = dual();
    let p = placement(&monitors, &Rect::new(2000, 100, 600, 900));
    assert_eq!(
        p,
        WindowPlacement {
            x: 2000,
            y: 100,
            width: 400,
            height: 600
        }
    );
}

#[test]
fn rect_spanning_monitors_belongs_to_the_bigger_part() {
    let monitors = dual();
    assert_eq!(for_rect(&monitors, &Rect::new(1800, 0, 400, 400)), Some(1));
    assert_eq!(for_rect(&monitors, &Rect::new(1700, 0, 400, 400)), Some(0));
    assert_eq!(for_rect(&monitors, &Rect::new(-5000, 0, 10, 10)), Some(0));
}

#[test]
fn resizing_changes_the_stored_size() {
    let m = &single()[0];
    let mut s = settings(DockMode::Left);
    resize(&mut s, m, &Rect::new(0, 0, 500, 1040));
    assert_eq!(s.side_width, 500);
    resize(&mut s, m, &Rect::new(0, 0, 1000, 1040));
    assert_eq!(s.side_width, 640);

    let mut s = settings(DockMode::Top);
    resize(&mut s, m, &Rect::new(500, 0, 900, 600));
    assert_eq!((s.top_width, s.top_height), (900, 600));
}
