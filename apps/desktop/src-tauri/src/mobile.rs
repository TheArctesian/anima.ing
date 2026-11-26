//! The mobile application (MOB-002, MOB-031; mobile plan M3-4). This file wires; it holds no rules.

use tauri::RunEvent;

use crate::{cloud, phone_browser, phone_link};

pub(crate) mod account_session;
mod metrics;
mod preview;
mod runner;
pub(crate) mod tokens;

#[tauri::mobile_entry_point]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_runner::init())
        .plugin(tauri_plugin_workspace_preview::init())
        .plugin(tauri_plugin_keyboard::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_account_session::init())
        .manage(cloud::Cloud::default())
        .setup(|app| {
            phone_link::manage(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            tokens::access_token_read,
            tokens::access_token_write,
            tokens::access_token_clear,
            phone_link::link_join,
            phone_link::link_status,
            phone_link::hub_listener_check,
            phone_link::link_leave,
            cloud::cloud_sign_in_start,
            cloud::cloud_sign_in_poll,
            cloud::cloud_sign_in_resume,
            cloud::cloud_sign_out,
            metrics::mobile_metrics_enrolled,
            metrics::mobile_metrics_enrol,
            metrics::mobile_metrics_push,
            cloud::in_app::cloud_sign_in_session,
            cloud::cloud_hub,
            cloud::cloud_hub_access,
            cloud::cloud_hub_usage,
            phone_browser::system_browser_open,
            runner::runner_capabilities_read,
            preview::mobile_preview_open,
            preview::mobile_preview_close,
        ])
        .build(tauri::generate_context!())
        .expect("the Tauri application failed to start")
        .run(|app, event| {
            if matches!(event, RunEvent::Resumed) {
                phone_link::resume(app);
            }
        });
}
