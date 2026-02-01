//! IncuBar - AI Usage Tracker
//!
//! Cross-platform menu bar app for tracking API usage across
//! Claude, Codex, Cursor, and other AI coding assistants.

pub mod browser_cookies;
pub mod commands;
pub mod debug_settings;
pub mod login;
pub mod providers;
pub mod storage;
pub mod tray;

use tauri::{Emitter, Manager};
#[cfg(target_os = "macos")]
use tauri::ActivationPolicy;
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tauri_plugin_process;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Initialize logging
fn init_logging() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::fmt::layer().with_writer(debug_settings::file_writer()))
        .with(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("incubar_tauri=debug".parse().unwrap()),
        )
        .init();
}

fn format_run_error(error: impl std::fmt::Display) -> String {
    format!("error while running tauri application: {error}")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    eprintln!("IncuBar starting...");
    init_logging();

    let app_result = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            eprintln!("Running setup...");
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(ActivationPolicy::Regular);
            }
            // Create the popup window (hidden by default)
            tray::create_popup_window(app.handle())?;
            eprintln!("Popup window created");

            // Initialize the tray icon after the window exists
            let tray_app = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                if let Err(err) = tray::setup_tray(&tray_app) {
                    tracing::error!("Tray setup failed: {err}");
                }
            });
            eprintln!("Tray setup scheduled");

            // Initialize the provider registry
            let registry = providers::ProviderRegistry::new();
            app.manage(registry);

            // Start the refresh timer
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                providers::start_refresh_loop(handle).await;
            });

            app.global_shortcut()
                .on_shortcut("CmdOrCtrl+R", move |app, _, _| {
                    let _ = app.emit("refresh-requested", ());
                })?;

            tracing::info!("IncuBar initialized successfully");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::refresh_provider,
            commands::refresh_all_providers,
            commands::get_provider_usage,
            commands::get_all_usage,
            commands::poll_provider_statuses,
            commands::set_provider_enabled,
            commands::set_enabled_providers,
            commands::broadcast_settings_updated,
            commands::get_settings,
            commands::save_settings,
            commands::save_menu_bar_display_settings,
            commands::send_test_notification,
            commands::get_install_origin,
            commands::set_debug_file_logging,
            commands::set_debug_keep_cli_sessions_alive,
            commands::set_debug_random_blink,
            commands::set_redact_personal_info,
            commands::export_support_bundle,
            commands::open_settings_window,
            commands::start_login,
            commands::check_auth,
            commands::check_all_auth,
            commands::store_cursor_cookies,
            commands::store_factory_cookies,
            commands::store_augment_cookies,
            commands::store_kimi_cookies,
            commands::store_minimax_cookies,
            commands::store_amp_cookies,
            commands::store_opencode_cookies,
            commands::open_cursor_login,
            commands::close_cursor_login,
            commands::extract_cursor_cookies,
            commands::import_cursor_browser_cookies,
            commands::import_cursor_browser_cookies_from_source,
            commands::import_factory_browser_cookies,
            commands::import_factory_browser_cookies_from_source,
            commands::import_augment_browser_cookies,
            commands::import_augment_browser_cookies_from_source,
            commands::import_kimi_browser_cookies,
            commands::import_kimi_browser_cookies_from_source,
            commands::import_minimax_browser_cookies,
            commands::import_minimax_browser_cookies_from_source,
            commands::import_amp_browser_cookies,
            commands::import_amp_browser_cookies_from_source,
            commands::import_opencode_browser_cookies,
            commands::import_opencode_browser_cookies_from_source,
            commands::copilot_request_device_code,
            commands::copilot_poll_for_token,
            commands::get_autostart_enabled,
            commands::set_autostart_enabled,
        ])
        .build(tauri::generate_context!());

    let app = match app_result {
        Ok(app) => app,
        Err(error) => {
            let message = format_run_error(&error);
            tracing::error!(error = %error, "{message}");
            eprintln!("{message}");
            std::process::exit(1);
        }
    };

    let run_result = app.run(|_app_handle, event| {
        if let tauri::RunEvent::ExitRequested { .. } = event {
            tracing::info!("App exit requested, shutting down background threads");
            tray::request_shutdown();
        }
    });

    if let Err(error) = run_result {
        let message = format_run_error(&error);
        tracing::error!(error = %error, "{message}");
        eprintln!("{message}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::format_run_error;

    #[test]
    fn run_error_message_includes_details() {
        let message = format_run_error("boom");

        assert!(message.contains("error while running tauri application"));
        assert!(message.contains("boom"));
    }
}
