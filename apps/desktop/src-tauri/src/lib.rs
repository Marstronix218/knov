mod agent;
mod analytics;
mod commands;
mod context;
mod db;
mod discovery;
mod error;
mod memory;
mod models;
mod platform;
mod prediction;
mod providers;
mod revenue;
mod threading;

use std::sync::{atomic::AtomicBool, Arc, RwLock};

use commands::AppState;
use db::Database;
use platform::RuntimeStatus;
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager, RunEvent, WindowEvent, Wry,
};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_autostart::ManagerExt;

/// Launch-at-login passes this so Knov starts collecting without opening a window.
const HIDDEN_LAUNCH_ARG: &str = "--hidden";

struct TrayCollectionItem(CheckMenuItem<Wry>);

/// Keeps the menu-bar checkmark in step with collection changes made in the window.
pub(crate) fn sync_tray_collection(app: &AppHandle, enabled: bool) {
    if let Some(item) = app.try_state::<TrayCollectionItem>() {
        let _ = item.0.set_checked(enabled);
    }
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .macos_launcher(MacosLauncher::LaunchAgent)
                .args([HIDDEN_LAUNCH_ARG])
                .build(),
        )
        // Collection runs in this process, so closing the window hides it instead
        // of quitting. Quit from the menu bar icon or with ⌘Q.
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let db = Arc::new(Database::open(data_dir.join("knov.sqlite3"))?);
            let providers = providers::ProviderClient::default();
            let initial_settings = db.settings()?;
            providers.configure_local(commands::local_model_config(&initial_settings));
            if std::env::args().any(|arg| arg == HIDDEN_LAUNCH_ARG) {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
            let launch_result = if db.settings()?.launch_at_login {
                app.autolaunch().enable()
            } else {
                app.autolaunch().disable()
            };
            if let Err(error) = launch_result {
                eprintln!("launch-at-login state could not be applied: {error}");
            }
            let runtime = Arc::new(RwLock::new(RuntimeStatus::default()));
            if let Err(error) = agent::recover(&db, chrono::Utc::now().timestamp()) {
                eprintln!("interrupted agent actions could not be closed: {error}");
            }
            let agent_host: Arc<dyn agent::ActionHost> =
                Arc::new(agent::SystemHost::new(data_dir.join("drafts")));
            let state = AppState {
                db: db.clone(),
                providers,
                runtime: runtime.clone(),
                refresh_lock: Arc::new(AtomicBool::new(false)),
                prediction_lock: Arc::new(AtomicBool::new(false)),
                agent_host,
                agent_lock: Arc::new(AtomicBool::new(false)),
            };
            platform::start_collector(db.clone(), runtime);
            platform::start_local_metadata_collectors(db.clone());
            #[cfg(feature = "chrome-extension")]
            if let Err(error) = platform::start_ingestion_server(db) {
                eprintln!("optional extension ingestion endpoint unavailable: {error}");
            }
            commands::start_scheduler(Arc::new(AppState {
                db: state.db.clone(),
                providers: state.providers.clone(),
                runtime: state.runtime.clone(),
                refresh_lock: state.refresh_lock.clone(),
                prediction_lock: state.prediction_lock.clone(),
                agent_host: state.agent_host.clone(),
                agent_lock: state.agent_lock.clone(),
            }));

            let show = MenuItem::with_id(app, "show", "Open Knov", true, None::<&str>)?;
            let collecting = CheckMenuItem::with_id(
                app,
                "collect",
                "Collect activity",
                true,
                initial_settings.collection_enabled,
                None::<&str>,
            )?;
            let separator = PredefinedMenuItem::separator(app)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Knov", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &collecting, &separator, &quit])?;
            app.manage(TrayCollectionItem(collecting));
            let mut tray = TrayIconBuilder::with_id("knov")
                .tooltip("Knov")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_main_window(app),
                    "collect" => {
                        let Some(state) = app.try_state::<AppState>() else {
                            return;
                        };
                        if let Ok(mut settings) = state.db.settings() {
                            settings.collection_enabled = !settings.collection_enabled;
                            let enabled = settings.collection_enabled;
                            if state.db.save_settings(&settings).is_ok() {
                                sync_tray_collection(app, enabled);
                            }
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            revenue::revenue_overview,
            revenue::revenue_seed_demo,
            revenue::revenue_create_project,
            revenue::revenue_import_agreement,
            revenue::revenue_update_agreement,
            revenue::revenue_import_file,
            revenue::revenue_add_evidence,
            revenue::revenue_classify_evidence,
            revenue::revenue_delete_source,
            revenue::revenue_analyze,
            revenue::revenue_review,
            revenue::revenue_prepare_action,
            revenue::revenue_save_draft,
            revenue::revenue_approve_draft,
            revenue::revenue_record_outcome,
            revenue::connectors::revenue_connector_status,
            revenue::connectors::revenue_connector_connect,
            revenue::connectors::revenue_connector_sync,
            revenue::connectors::revenue_connector_disconnect,
            revenue::connectors::revenue_extract_document,
            revenue::connectors::revenue_gmail_oauth,
            commands::delete_discovery_interview,
            commands::get_discovery_sessions,
            commands::start_discovery_interview,
            commands::advance_discovery_interview,
            commands::set_discovery_interview_status,
            commands::get_discovered_workflows,
            commands::save_discovered_workflow,
            commands::get_discovery_graph,
            commands::get_discovery_graph_history,
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
            commands::probe_permissions,
            commands::detect_local_models,
            commands::get_tester_summary,
            commands::open_mail_draft,
            commands::request_accessibility_permission,
            commands::set_browser_profiles,
            commands::start_bootstrap,
            commands::start_local_bootstrap,
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
            commands::record_product_event,
            commands::get_predictions_dashboard,
            commands::get_prediction_history,
            commands::generate_predictions,
            commands::record_prediction_feedback,
            commands::get_agent_overview,
            commands::review_goal,
            commands::get_workflows,
            commands::rescan_workflows,
            commands::review_workflow,
            commands::get_skills,
            commands::create_skill,
            commands::update_skill,
            commands::delete_skill,
            commands::preview_skill_run,
            commands::decide_agent_run,
            commands::cancel_agent_run,
            commands::acknowledge_agent_run,
            commands::get_agent_runs,
            commands::get_agent_run,
            commands::rollback_agent_action,
            commands::open_agent_draft,
            commands::get_autonomy,
            commands::set_agent_paused,
            commands::save_autonomy_grant,
            commands::revoke_autonomy_grant,
            commands::respond_autonomy_proposal,
            commands::approve_agent_workspace,
            commands::remove_agent_workspace,
            commands::chat,
            commands::get_pairing_info,
            commands::install_native_host,
            commands::delete_all_data
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // Clicking the Dock icon brings back the hidden window.
            #[cfg(target_os = "macos")]
            if let RunEvent::Reopen {
                has_visible_windows: false,
                ..
            } = event
            {
                show_main_window(app);
            }
            #[cfg(not(target_os = "macos"))]
            let _ = (app, event);
        });
}
