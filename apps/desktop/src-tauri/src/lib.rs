mod analytics;
mod commands;
mod context;
mod db;
mod error;
mod memory;
mod models;
mod platform;
mod providers;
mod threading;

use std::sync::{atomic::AtomicBool, Arc, RwLock};

use commands::AppState;
use db::Database;
use platform::RuntimeStatus;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_autostart::ManagerExt;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .macos_launcher(MacosLauncher::LaunchAgent)
                .build(),
        )
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let db = Arc::new(Database::open(data_dir.join("knov.sqlite3"))?);
            let launch_result = if db.settings()?.launch_at_login {
                app.autolaunch().enable()
            } else {
                app.autolaunch().disable()
            };
            if let Err(error) = launch_result {
                eprintln!("launch-at-login state could not be applied: {error}");
            }
            let runtime = Arc::new(RwLock::new(RuntimeStatus::default()));
            let state = AppState {
                db: db.clone(),
                providers: providers::ProviderClient::default(),
                runtime: runtime.clone(),
                refresh_lock: Arc::new(AtomicBool::new(false)),
            };
            platform::start_collector(db.clone(), runtime);
            platform::start_local_metadata_collectors(db.clone());
            // Localhost is an intentionally documented alpha fallback; native messaging
            // remains the production extension transport.
            if let Err(error) = platform::start_ingestion_server(db) {
                eprintln!("extension ingestion endpoint unavailable: {error}");
            }
            commands::start_scheduler(Arc::new(AppState {
                db: state.db.clone(),
                providers: state.providers.clone(),
                runtime: state.runtime.clone(),
                refresh_lock: state.refresh_lock.clone(),
            }));

            let show = MenuItem::with_id(app, "show", "Show Knov", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            TrayIconBuilder::new()
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_dashboard,
            commands::get_activity_history,
            commands::get_activity_icon,
            commands::get_activity_preview,
            commands::open_resource,
            commands::open_application,
            commands::get_profile,
            commands::get_settings,
            commands::get_browser_profiles,
            commands::get_bootstrap_status,
            commands::set_collection_enabled,
            commands::request_accessibility_permission,
            commands::set_browser_profiles,
            commands::start_bootstrap,
            commands::reimport_chrome_history,
            commands::refresh_profile,
            commands::save_profile_correction,
            commands::remove_profile_correction,
            commands::dismiss_profile_inference,
            commands::save_profile_summary,
            commands::save_provider_key,
            commands::remove_provider_key,
            commands::test_provider,
            commands::save_settings,
            commands::dismiss_recommendation,
            commands::chat,
            commands::get_pairing_info,
            commands::install_native_host,
            commands::delete_all_data
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
