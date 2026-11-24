//! Иконка в трее и её меню.

use tauri::{
    AppHandle,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use crate::{updater, window_ctl};

const TRAY_ID: &str = "main";
const MENU_OPEN: &str = "open";
const MENU_SEARCH: &str = "search";
const MENU_QUIT: &str = "quit";

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, MENU_OPEN, "Открыть", true, None::<&str>)?;
    let search = MenuItem::with_id(app, MENU_SEARCH, "Поиск", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let update = updater::create_menu_item(app)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "Выход", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &search, &separator, &update, &quit])?;

    let tooltip = format!("XonkyPass {}", app.package_info().version);
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip(tooltip)
        .menu(&menu)
        // Левый клик показывает/прячет панель, меню — по правому.
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            MENU_OPEN => window_ctl::show_panel(app),
            MENU_SEARCH => window_ctl::show_palette(app),
            updater::MENU_ID => updater::on_menu_click(app),
            MENU_QUIT => {
                tracing::info!("выход через меню трея");
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                window_ctl::toggle_panel(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder.build(app)?;
    Ok(())
}
