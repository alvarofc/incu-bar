//! IncuBar - AI Usage Tracker
//! 
//! Cross-platform menu bar app for tracking API usage across
//! Claude, Codex, Cursor, and other AI coding assistants.

pub mod browser_cookies;
pub mod commands;
pub mod login;
pub mod providers;
pub mod storage;
pub mod tray;

use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Initialize logging
fn init_logging() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("incubar_tauri=debug".parse().unwrap()))
        .init();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_logging();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--minimized"])))
        .setup(|app| {
            // Initialize the tray icon
            tray::setup_tray(app.handle())?;
            
            // Create the popup window (hidden by default)
            tray::create_popup_window(app.handle())?;

            // Initialize the provider registry
            let registry = providers::ProviderRegistry::new();
            app.manage(registry);

            // Start the refresh timer
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                providers::start_refresh_loop(handle).await;
            });

            tracing::info!("IncuBar initialized successfully");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::refresh_provider,
            commands::refresh_all_providers,
            commands::get_provider_usage,
            commands::get_all_usage,
            commands::set_provider_enabled,
            commands::get_settings,
            commands::save_settings,
            commands::start_login,
            commands::check_auth,
            commands::check_all_auth,
            commands::store_cursor_cookies,
            commands::store_factory_cookies,
            commands::store_augment_cookies,
            commands::store_kimi_cookies,
            commands::store_minimax_cookies,
            commands::open_cursor_login,
            commands::close_cursor_login,
            commands::extract_cursor_cookies,
            commands::import_cursor_browser_cookies,
            commands::import_factory_browser_cookies,
            commands::import_augment_browser_cookies,
            commands::import_kimi_browser_cookies,
            commands::import_minimax_browser_cookies,
            commands::copilot_request_device_code,
            commands::copilot_poll_for_token,
            commands::get_autostart_enabled,
            commands::set_autostart_enabled,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
