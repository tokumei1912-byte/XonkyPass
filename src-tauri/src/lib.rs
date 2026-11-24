//! Приложение XonkyPass: окна, трей, команды IPC и интеграция с Windows.

mod clipboard;
mod commands;
mod error;
mod events;
mod logging;
mod paths;
mod platform;
mod settings;
mod state;
mod tray;
mod updater;
mod window_ctl;

use tauri::{Manager, WindowEvent};

use crate::state::AppState;

/// Точка входа приложения.
pub fn run() {
    tauri::Builder::default()
        // Должен регистрироваться первым: второй запуск просто показывает панель.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tracing::info!("повторный запуск: показываем панель");
            window_ctl::show_panel(app);
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::app::app_info,
            commands::app::appearance,
            commands::app::update_check,
            commands::app::update_install,
            commands::shell::open_url,
            commands::shell::reveal_vault,
            commands::shell::recovery_key_save,
            commands::vault::vault_status,
            commands::vault::vault_create,
            commands::vault::vault_unlock,
            commands::vault::vault_unlock_recovery,
            commands::vault::vault_lock,
            commands::vault::vault_change_password,
            commands::vault::vault_reset_password,
            commands::vault::vault_regenerate_recovery_key,
            commands::items::items_list,
            commands::items::item_get,
            commands::items::item_reveal,
            commands::items::item_create,
            commands::items::item_update,
            commands::items::item_trash,
            commands::items::item_restore,
            commands::items::item_purge,
            commands::items::trash_empty,
            commands::items::item_copy,
            commands::generator::generate_password,
            commands::generator::generate_passphrase,
            commands::generator::password_strength,
            commands::clipboard::clipboard_copy,
            commands::clipboard::clipboard_clear,
            commands::settings::settings_get,
            commands::settings::settings_set,
        ])
        .setup(|app| {
            logging::init(app.handle());
            tracing::info!(version = env!("CARGO_PKG_VERSION"), "запуск XonkyPass");

            let vault_path = paths::vault_path(app.handle())?;
            let settings_path = paths::settings_path(app.handle())?;
            tracing::info!(path = %vault_path.display(), "файл хранилища");
            let settings = settings::load(&settings_path);
            let theme = settings.theme;
            app.manage(AppState::new(vault_path, settings_path, settings));
            app.manage(clipboard::Clipboard::default());

            window_ctl::prepare(app.handle(), theme);
            clipboard::init(app.handle());
            tray::create(app.handle())?;
            updater::start(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| {
            // Окна живут всё время работы приложения: закрытие только прячет их.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                window_ctl::hide(window);
            }
        })
        .run(tauri::generate_context!())
        .expect("не удалось запустить XonkyPass");
}
