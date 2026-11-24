//! Приложение XonkyPass: окна, трей, команды IPC и интеграция с Windows.

mod commands;
mod logging;
mod paths;
mod platform;
mod tray;
mod window_ctl;

use tauri::WindowEvent;

/// Точка входа приложения.
pub fn run() {
    tauri::Builder::default()
        // Должен регистрироваться первым: второй запуск просто показывает панель.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tracing::info!("повторный запуск: показываем панель");
            window_ctl::show_panel(app);
        }))
        .invoke_handler(tauri::generate_handler![commands::app_info])
        .setup(|app| {
            logging::init(app.handle());
            tracing::info!(version = env!("CARGO_PKG_VERSION"), "запуск XonkyPass");
            window_ctl::prepare(app.handle());
            tray::create(app.handle())?;
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
