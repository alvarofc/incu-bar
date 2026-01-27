//! Tauri IPC commands for the frontend

use serde::{Deserialize, Serialize};
use tauri::{command, AppHandle, Emitter, Manager, Runtime, State};
use tauri_plugin_store::StoreExt;
use tauri_plugin_autostart::AutoLaunchManager;
use tauri_plugin_notification::NotificationExt;

use crate::browser_cookies::BrowserCookieSource;
use crate::debug_settings;
use crate::login::{self, AuthStatus, LoginResult};
use crate::providers::{ProviderId, ProviderRegistry, ProviderStatus, UsageSnapshot};
use crate::storage::install_origin;
use crate::storage::widget_snapshot;
use crate::tray;

struct LoadingGuard {
    app: AppHandle,
    active: bool,
}

impl LoadingGuard {
    fn new(app: &AppHandle) -> Self {
        if let Err(err) = tray::set_loading_state(app, true) {
            tracing::warn!("Failed to set loading state: {}", err);
        }
        Self {
            app: app.clone(),
            active: true,
        }
    }

    fn finish(&mut self) {
        if self.active {
            if let Err(err) = tray::set_loading_state(&self.app, false) {
                tracing::warn!("Failed to clear loading state: {}", err);
            }
            self.active = false;
        }
    }
}

impl Drop for LoadingGuard {
    fn drop(&mut self) {
        self.finish();
    }
}

fn emit_refreshing(app: &AppHandle, provider_id: ProviderId, is_refreshing: bool) {
    let _ = app.emit(
        "refreshing-provider",
        serde_json::json!({
            "providerId": provider_id,
            "isRefreshing": is_refreshing,
        }),
    );
}

/// Settings structure matching frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub refresh_interval_seconds: u32,
    pub enabled_providers: Vec<ProviderId>,
    pub provider_order: Vec<ProviderId>,
    pub display_mode: String,
    pub menu_bar_display_mode: String,
    pub menu_bar_display_text_enabled: bool,
    pub menu_bar_display_text_mode: String,
    pub usage_bar_display_mode: String,
    pub show_notifications: bool,
    pub launch_at_login: bool,
    pub show_credits: bool,
    pub show_cost: bool,
    pub show_extra_usage: bool,
    pub debug_file_logging: bool,
    pub debug_keep_cli_sessions_alive: bool,
    pub debug_random_blink: bool,
    pub redact_personal_info: bool,
}

const SETTINGS_STORE_PATH: &str = "settings.json";
const SETTINGS_STORE_KEY: &str = "app_settings";

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            refresh_interval_seconds: 300,
            enabled_providers: vec![ProviderId::Claude, ProviderId::Codex, ProviderId::Cursor],
            provider_order: vec![ProviderId::Claude, ProviderId::Codex, ProviderId::Cursor],
            display_mode: "merged".to_string(),
            menu_bar_display_mode: "session".to_string(),
            menu_bar_display_text_enabled: false,
            menu_bar_display_text_mode: "percent".to_string(),
            usage_bar_display_mode: "remaining".to_string(),
            show_notifications: true,
            launch_at_login: false,
            show_credits: true,
            show_cost: true,
            show_extra_usage: true,
            debug_file_logging: false,
            debug_keep_cli_sessions_alive: false,
            debug_random_blink: false,
            redact_personal_info: false,
        }
    }
}

/// Refresh a single provider's usage data
#[command]
pub async fn refresh_provider(
    provider_id: ProviderId,
    registry: State<'_, ProviderRegistry>,
    app: AppHandle,
) -> Result<UsageSnapshot, String> {
    tracing::info!("refresh_provider: starting for {:?}", provider_id);
    let start = std::time::Instant::now();

    // Check if provider is authenticated before attempting refresh
    // Convert ProviderId to string using serde serialization (snake_case)
    let provider_id_str = serde_json::to_string(&provider_id)
        .unwrap_or_default()
        .trim_matches('"')
        .to_string();
    let auth_status = login::check_auth_status(&provider_id_str).await;
    tracing::info!("refresh_provider: auth check for {:?} took {:?}, authenticated: {}", 
        provider_id, start.elapsed(), auth_status.authenticated);
    
    if !auth_status.authenticated {
        let error_msg = auth_status
            .error
            .unwrap_or_else(|| "Not authenticated".to_string());
        tracing::info!(
            "refresh_provider: skipping {:?}: {}",
            provider_id,
            error_msg
        );
        let usage = UsageSnapshot::error(format!("Not authenticated: {}", error_msg));
        
        let _ = app.emit(
            "usage-updated",
            serde_json::json!({
                "providerId": provider_id,
                "usage": usage.clone(),
            }),
        );
        
        return Err(format!("Not authenticated: {}", error_msg));
    }

    let mut loading_guard = LoadingGuard::new(&app);
    emit_refreshing(&app, provider_id, true);

    tracing::info!("refresh_provider: fetching status for {:?}", provider_id);
    let status_start = std::time::Instant::now();
    let status = registry.fetch_status(&provider_id).await.ok();
    tracing::info!("refresh_provider: status fetch for {:?} took {:?}", provider_id, status_start.elapsed());
    
    tracing::info!("refresh_provider: fetching usage for {:?}", provider_id);
    let usage_start = std::time::Instant::now();
    let usage_result = registry.fetch_usage(&provider_id).await;
    tracing::info!("refresh_provider: usage fetch for {:?} took {:?}", provider_id, usage_start.elapsed());

    loading_guard.finish();
    emit_refreshing(&app, provider_id, false);
    
    tracing::info!("refresh_provider: total time for {:?}: {:?}", provider_id, start.elapsed());

    loading_guard.finish();
    emit_refreshing(&app, provider_id, false);

    match usage_result {
        Ok(usage) => {
            let _ = app.emit(
                "usage-updated",
                serde_json::json!({
                    "providerId": provider_id,
                    "usage": usage,
                }),
            );

            let _ = app.emit(
                "status-updated",
                serde_json::json!({
                    "providerId": provider_id,
                    "status": status,
                }),
            );

            if let Err(err) = widget_snapshot::write_widget_snapshot(provider_id, &usage) {
                tracing::warn!("Failed to write widget snapshot: {}", err);
            }
            tray::handle_usage_update(&app, provider_id, usage.clone())
                .map_err(|e| e.to_string())?;

            Ok(usage)
        }
        Err(e) => {
            let message = e.to_string();
            let usage = UsageSnapshot::error(message.clone());

            let _ = app.emit(
                "refresh-failed",
                serde_json::json!({
                    "providerId": provider_id,
                    "usage": usage.clone(),
                }),
            );

            let _ = app.emit(
                "status-updated",
                serde_json::json!({
                    "providerId": provider_id,
                    "status": status,
                }),
            );

            let _ = app.emit(
                "usage-updated",
                serde_json::json!({
                    "providerId": provider_id,
                    "usage": usage.clone(),
                }),
            );

            if let Err(err) = widget_snapshot::write_widget_snapshot(provider_id, &usage) {
                tracing::warn!("Failed to write widget snapshot: {}", err);
            }
            if let Err(e) = tray::handle_usage_update(&app, provider_id, usage) {
                tracing::warn!("Failed to update tray icon: {}", e);
            }

            Err(message)
        }
    }
}

/// Refresh all enabled providers
#[command]
pub async fn refresh_all_providers(
    registry: State<'_, ProviderRegistry>,
    app: AppHandle,
) -> Result<(), String> {
    tracing::debug!("Refreshing all providers");

    let providers = registry.get_enabled_providers().await;

    let mut loading_guard = LoadingGuard::new(&app);

    for provider_id in &providers {
        emit_refreshing(&app, *provider_id, true);
    }

    for provider_id in providers {
        let status = registry.fetch_status(&provider_id).await.ok();
        match registry.fetch_usage(&provider_id).await {
            Ok(usage) => {
                let _ = app.emit(
                    "usage-updated",
                    serde_json::json!({
                        "providerId": provider_id,
                        "usage": usage,
                    }),
                );
                let _ = app.emit(
                    "status-updated",
                    serde_json::json!({
                        "providerId": provider_id,
                        "status": status,
                    }),
                );
                if let Err(err) = widget_snapshot::write_widget_snapshot(provider_id, &usage) {
                    tracing::warn!("Failed to write widget snapshot: {}", err);
                }
                if let Err(e) = tray::handle_usage_update(&app, provider_id, usage.clone()) {
                    tracing::warn!("Failed to update tray icon: {}", e);
                }
            }
            Err(e) => {
                tracing::warn!("Failed to refresh {:?}: {}", provider_id, e);
                let usage = UsageSnapshot::error(e.to_string());
                let _ = app.emit(
                    "usage-updated",
                    serde_json::json!({
                        "providerId": provider_id,
                        "usage": usage.clone(),
                    }),
                );
                let _ = app.emit(
                    "refresh-failed",
                    serde_json::json!({
                        "providerId": provider_id,
                        "usage": usage.clone(),
                    }),
                );
                let _ = app.emit(
                    "status-updated",
                    serde_json::json!({
                        "providerId": provider_id,
                        "status": status,
                    }),
                );
                if let Err(err) = widget_snapshot::write_widget_snapshot(provider_id, &usage) {
                    tracing::warn!("Failed to write widget snapshot: {}", err);
                }
                if let Err(e) = tray::handle_usage_update(&app, provider_id, usage) {
                    tracing::warn!("Failed to update tray icon: {}", e);
                }
            }
        }
        emit_refreshing(&app, provider_id, false);
    }

    loading_guard.finish();

    Ok(())
}

/// Get usage for a single provider (cached)
#[command]
pub async fn get_provider_usage(
    provider_id: ProviderId,
    registry: State<'_, ProviderRegistry>,
) -> Result<Option<UsageSnapshot>, String> {
    Ok(registry.get_cached_usage(&provider_id).await)
}

/// Get all cached usage data
#[command]
pub async fn get_all_usage(
    registry: State<'_, ProviderRegistry>,
) -> Result<std::collections::HashMap<ProviderId, UsageSnapshot>, String> {
    Ok(registry.get_all_cached_usage().await)
}

/// Poll provider status/incident data (only for enabled providers)
#[command]
pub async fn poll_provider_statuses(
    registry: State<'_, ProviderRegistry>,
) -> Result<std::collections::HashMap<ProviderId, Option<ProviderStatus>>, String> {
    // Only poll enabled providers to avoid wasting resources on disabled ones
    let enabled_providers = registry.get_enabled_providers().await;
    let mut statuses = std::collections::HashMap::new();
    
    for provider_id in enabled_providers {
        // Check if provider is authenticated before polling status
        let provider_id_str = serde_json::to_string(&provider_id)
            .unwrap_or_default()
            .trim_matches('"')
            .to_string();
        let auth_status = login::check_auth_status(&provider_id_str).await;
        
        if !auth_status.authenticated {
            statuses.insert(provider_id, None);
            continue;
        }
        
        let status = registry.fetch_status(&provider_id).await.ok();
        statuses.insert(provider_id, status);
    }
    Ok(statuses)
}

/// Enable or disable a provider
#[command]
pub async fn set_provider_enabled(
    provider_id: ProviderId,
    enabled: bool,
    registry: State<'_, ProviderRegistry>,
    app: AppHandle,
) -> Result<(), String> {
    registry.set_enabled(&provider_id, enabled).await;
    tray::set_provider_disabled(&app, provider_id, !enabled).map_err(|e| e.to_string())?;
    tray::set_blinking_state(&app, !enabled).map_err(|e| e.to_string())?;
    Ok(())
}

/// Get current settings
#[command]
pub async fn get_settings(app: AppHandle) -> Result<AppSettings, String> {
    load_settings(app).await
}

async fn load_settings<R: Runtime>(app: AppHandle<R>) -> Result<AppSettings, String> {
    let store = app
        .store(SETTINGS_STORE_PATH)
        .map_err(|e| e.to_string())?;
    if let Some(value) = store.get(SETTINGS_STORE_KEY) {
        serde_json::from_value(value).map_err(|e| e.to_string())
    } else {
        Ok(AppSettings::default())
    }
}

/// Save settings
#[command]
pub async fn save_settings(settings: AppSettings, app: AppHandle) -> Result<(), String> {
    save_settings_inner(settings, app).await
}

async fn save_settings_inner<R: Runtime>(settings: AppSettings, app: AppHandle<R>) -> Result<(), String> {
    let store = app
        .store(SETTINGS_STORE_PATH)
        .map_err(|e| e.to_string())?;
    let value = serde_json::to_value(&settings).map_err(|e| e.to_string())?;
    store.set(SETTINGS_STORE_KEY, value);
    store.save().map_err(|e| e.to_string())?;
    tracing::debug!(
        "Saving settings: AppSettings {{ refresh_interval_seconds: {}, enabled_providers: {:?}, provider_order: {:?}, display_mode: {}, menu_bar_display_mode: {}, menu_bar_display_text_enabled: {}, menu_bar_display_text_mode: {}, usage_bar_display_mode: {}, show_notifications: {}, launch_at_login: {}, show_credits: {}, show_cost: {}, show_extra_usage: {}, debug_file_logging: {}, debug_keep_cli_sessions_alive: {}, debug_random_blink: {}, redact_personal_info: {} }}",
        settings.refresh_interval_seconds,
        settings.enabled_providers,
        settings.provider_order,
        settings.display_mode,
        settings.menu_bar_display_mode,
        settings.menu_bar_display_text_enabled,
        settings.menu_bar_display_text_mode,
        settings.usage_bar_display_mode,
        settings.show_notifications,
        settings.launch_at_login,
        settings.show_credits,
        settings.show_cost,
        settings.show_extra_usage,
        settings.debug_file_logging,
        settings.debug_keep_cli_sessions_alive,
        settings.debug_random_blink,
        settings.redact_personal_info
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::test::{mock_builder, mock_context, noop_assets};

    fn build_test_app() -> tauri::App<tauri::test::MockRuntime> {
        mock_builder()
            .plugin(tauri_plugin_store::Builder::default().build())
            .build(mock_context(noop_assets()))
            .expect("failed to build app")
    }

    #[test]
    fn get_settings_defaults_when_store_empty() {
        let app = build_test_app();
        let handle = app.handle();

        let store = handle.store(SETTINGS_STORE_PATH).expect("store init");
        store.clear();
        store.save().expect("store save");

        let settings = tauri::async_runtime::block_on(load_settings(handle.clone()))
            .expect("get settings");
        assert_eq!(settings.refresh_interval_seconds, AppSettings::default().refresh_interval_seconds);
        assert_eq!(settings.enabled_providers, AppSettings::default().enabled_providers);
    }

    #[test]
    fn save_and_get_settings_roundtrip() {
        let app = build_test_app();
        let handle = app.handle();

        let mut settings = AppSettings::default();
        settings.refresh_interval_seconds = 120;
        settings.display_mode = "separate".to_string();
        settings.enabled_providers = vec![ProviderId::Claude, ProviderId::Cursor];

        tauri::async_runtime::block_on(save_settings_inner(settings.clone(), handle.clone()))
            .expect("save settings");

        let loaded = tauri::async_runtime::block_on(load_settings(handle.clone()))
            .expect("get settings");

        assert_eq!(loaded.refresh_interval_seconds, settings.refresh_interval_seconds);
        assert_eq!(loaded.display_mode, settings.display_mode);
        assert_eq!(loaded.enabled_providers, settings.enabled_providers);
    }
}

#[command]
pub async fn save_menu_bar_display_settings(
    app: AppHandle,
    menu_bar_display_mode: String,
    menu_bar_display_text_enabled: bool,
    menu_bar_display_text_mode: String,
    usage_bar_display_mode: String,
) -> Result<(), String> {
    tray::set_display_text_for_provider(
        &app,
        &menu_bar_display_mode,
        menu_bar_display_text_enabled,
        &menu_bar_display_text_mode,
        usage_bar_display_mode == "used",
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[command]
pub async fn set_debug_file_logging(enabled: bool) -> Result<(), String> {
    debug_settings::set_file_logging(enabled);
    Ok(())
}

#[command]
pub async fn set_debug_keep_cli_sessions_alive(enabled: bool) -> Result<(), String> {
    debug_settings::set_keep_cli_sessions_alive(enabled);
    Ok(())
}

#[command]
pub async fn set_debug_random_blink(enabled: bool) -> Result<(), String> {
    debug_settings::set_random_blink(enabled);
    Ok(())
}

#[command]
pub async fn set_redact_personal_info(enabled: bool) -> Result<(), String> {
    debug_settings::set_redact_personal_info(enabled);
    Ok(())
}

/// Send a test notification
#[command]
pub async fn send_test_notification(app: AppHandle) -> Result<(), String> {
    app.notification()
        .builder()
        .title("IncuBar")
        .body("Notifications are enabled")
        .show()
        .map_err(|e| e.to_string())
}

/// Export diagnostics bundle for support
#[command]
pub async fn export_support_bundle(
    app: AppHandle,
    payload: serde_json::Value,
) -> Result<String, String> {
    let file_name = format!(
        "incubar-support-{}.json",
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    );
    let export_dir = app
        .path()
        .download_dir()
        .ok()
        .or_else(|| app.path().app_data_dir().ok())
        .ok_or_else(|| "Could not determine export directory".to_string())?;

    if let Err(err) = std::fs::create_dir_all(&export_dir) {
        return Err(format!("Failed to create export directory: {}", err));
    }

    let bundle = serde_json::json!({
        "generatedAt": chrono::Utc::now().to_rfc3339(),
        "payload": payload,
    });
    let contents =
        serde_json::to_string_pretty(&bundle).map_err(|e| format!("Encode failed: {}", e))?;
    let path = export_dir.join(file_name);
    std::fs::write(&path, contents)
        .map_err(|e| format!("Failed to write support bundle: {}", e))?;
    Ok(path.to_string_lossy().to_string())
}

#[command]
pub async fn get_install_origin() -> Result<String, String> {
    install_origin::read_or_record_install_origin().map_err(|e| e.to_string())
}

// ============== Login Commands ==============

/// Start login flow for a provider
#[command]
pub async fn start_login(provider_id: String, app: AppHandle) -> Result<LoginResult, String> {
    tracing::info!("Starting login for provider: {}", provider_id);

    // Emit login started event
    let _ = app.emit(
        "login-started",
        serde_json::json!({
            "providerId": provider_id,
        }),
    );

    let result = match provider_id.as_str() {
        "claude" => login::run_claude_login().await.map_err(|e| e.to_string())?,
        "codex" => login::run_codex_login().await.map_err(|e| e.to_string())?,
        "cursor" => {
            // Open Cursor login window
            tray::create_cursor_login_window(&app).map_err(|e| e.to_string())?;
            return Ok(LoginResult {
                success: true,
                message: "Cursor login window opened. Complete login in the browser, then copy your cookies.".to_string(),
                provider_id: "cursor".to_string(),
            });
        }
        "factory" => {
            return Ok(LoginResult {
                success: true,
                message: "Factory uses browser cookies. Use Import from Browser or paste cookies manually.".to_string(),
                provider_id: "factory".to_string(),
            });
        }
        "augment" => {
            return Ok(LoginResult {
                success: true,
                message: "Augment uses browser cookies. Use Import from Browser or paste cookies manually.".to_string(),
                provider_id: "augment".to_string(),
            });
        }
        "amp" => {
            return Ok(LoginResult {
                success: true,
                message:
                    "Amp uses browser cookies. Use Import from Browser or paste cookies manually."
                        .to_string(),
                provider_id: "amp".to_string(),
            });
        }
        "kimi" => {
            return Ok(LoginResult {
                success: true,
                message:
                    "Kimi uses browser cookies. Use Import from Browser or paste cookies manually."
                        .to_string(),
                provider_id: "kimi".to_string(),
            });
        }
        "minimax" => {
            return Ok(LoginResult {
                success: true,
                message: "MiniMax uses browser cookies. Use Import from Browser or paste cookies manually.".to_string(),
                provider_id: "minimax".to_string(),
            });
        }
        "opencode" => {
            return Ok(LoginResult {
                success: true,
                message: "OpenCode uses browser cookies. Use Import from Browser or paste cookies manually.".to_string(),
                provider_id: "opencode".to_string(),
            });
        }
        "copilot" => login::run_copilot_login()
            .await
            .map_err(|e| e.to_string())?,
        "gemini" => login::run_gemini_login().await.map_err(|e| e.to_string())?,
        _ => {
            return Ok(LoginResult {
                success: false,
                message: format!("Login not supported for provider: {}", provider_id),
                provider_id,
            });
        }
    };

    // Emit login completed event
    let _ = app.emit(
        "login-completed",
        serde_json::json!({
            "providerId": provider_id,
            "success": result.success,
            "message": result.message,
        }),
    );

    Ok(result)
}

/// Check authentication status for a provider
#[command]
pub async fn check_auth(provider_id: String) -> Result<AuthStatus, String> {
    Ok(login::check_auth_status(&provider_id).await)
}

/// Check authentication status for all providers
#[command]
pub async fn check_all_auth() -> Result<std::collections::HashMap<String, AuthStatus>, String> {
    let providers = vec![
        "claude",
        "codex",
        "cursor",
        "factory",
        "augment",
        "amp",
        "copilot",
        "gemini",
        "zai",
        "kimi",
        "kimi_k2",
        "minimax",
        "opencode",
        "synthetic",
        "antigravity",
        "kiro",
    ];
    let mut results = std::collections::HashMap::new();

    for provider_id in providers {
        let status = login::check_auth_status(provider_id).await;
        results.insert(provider_id.to_string(), status);
    }

    Ok(results)
}

/// Store Cursor session cookies (for manual cookie paste)
#[command]
pub async fn store_cursor_cookies(cookie_header: String) -> Result<LoginResult, String> {
    match login::store_cursor_session(cookie_header).await {
        Ok(()) => Ok(LoginResult {
            success: true,
            message: "Cursor cookies saved successfully".to_string(),
            provider_id: "cursor".to_string(),
        }),
        Err(e) => Ok(LoginResult {
            success: false,
            message: format!("Failed to save Cursor cookies: {}", e),
            provider_id: "cursor".to_string(),
        }),
    }
}

/// Store Factory session cookies (for manual cookie paste)
#[command]
pub async fn store_factory_cookies(cookie_header: String) -> Result<LoginResult, String> {
    match login::store_factory_session(cookie_header).await {
        Ok(()) => Ok(LoginResult {
            success: true,
            message: "Factory cookies saved successfully".to_string(),
            provider_id: "factory".to_string(),
        }),
        Err(e) => Ok(LoginResult {
            success: false,
            message: format!("Failed to save Factory cookies: {}", e),
            provider_id: "factory".to_string(),
        }),
    }
}

/// Store Augment session cookies (for manual cookie paste)
#[command]
pub async fn store_augment_cookies(cookie_header: String) -> Result<LoginResult, String> {
    match login::store_augment_session(cookie_header).await {
        Ok(()) => Ok(LoginResult {
            success: true,
            message: "Augment cookies saved successfully".to_string(),
            provider_id: "augment".to_string(),
        }),
        Err(e) => Ok(LoginResult {
            success: false,
            message: format!("Failed to save Augment cookies: {}", e),
            provider_id: "augment".to_string(),
        }),
    }
}

/// Store Kimi session cookies (for manual cookie paste)
#[command]
pub async fn store_kimi_cookies(cookie_header: String) -> Result<LoginResult, String> {
    match login::store_kimi_session(cookie_header).await {
        Ok(()) => Ok(LoginResult {
            success: true,
            message: "Kimi cookies saved successfully".to_string(),
            provider_id: "kimi".to_string(),
        }),
        Err(e) => Ok(LoginResult {
            success: false,
            message: format!("Failed to save Kimi cookies: {}", e),
            provider_id: "kimi".to_string(),
        }),
    }
}

/// Store MiniMax session cookies (for manual cookie paste)
#[command]
pub async fn store_minimax_cookies(cookie_header: String) -> Result<LoginResult, String> {
    match login::store_minimax_session(cookie_header).await {
        Ok(()) => Ok(LoginResult {
            success: true,
            message: "MiniMax cookies saved successfully".to_string(),
            provider_id: "minimax".to_string(),
        }),
        Err(e) => Ok(LoginResult {
            success: false,
            message: format!("Failed to save MiniMax cookies: {}", e),
            provider_id: "minimax".to_string(),
        }),
    }
}

/// Store Amp session cookies (for manual cookie paste)
#[command]
pub async fn store_amp_cookies(cookie_header: String) -> Result<LoginResult, String> {
    let session_cookie = extract_amp_session_cookie(&cookie_header)?;
    match login::store_amp_session(session_cookie).await {
        Ok(()) => Ok(LoginResult {
            success: true,
            message: "Amp cookies saved successfully".to_string(),
            provider_id: "amp".to_string(),
        }),
        Err(e) => Ok(LoginResult {
            success: false,
            message: format!("Failed to save Amp cookies: {}", e),
            provider_id: "amp".to_string(),
        }),
    }
}

/// Store OpenCode session cookies (for manual cookie paste)
#[command]
pub async fn store_opencode_cookies(cookie_header: String) -> Result<LoginResult, String> {
    match login::store_opencode_session(cookie_header).await {
        Ok(()) => Ok(LoginResult {
            success: true,
            message: "OpenCode cookies saved successfully".to_string(),
            provider_id: "opencode".to_string(),
        }),
        Err(e) => Ok(LoginResult {
            success: false,
            message: format!("Failed to save OpenCode cookies: {}", e),
            provider_id: "opencode".to_string(),
        }),
    }
}

/// Store Codex session cookies (for web dashboard extras)
#[command]
pub async fn store_codex_cookies(cookie_header: String) -> Result<LoginResult, String> {
    match login::store_codex_session(cookie_header).await {
        Ok(()) => Ok(LoginResult {
            success: true,
            message: "Codex cookies saved successfully".to_string(),
            provider_id: "codex".to_string(),
        }),
        Err(e) => Ok(LoginResult {
            success: false,
            message: format!("Failed to save Codex cookies: {}", e),
            provider_id: "codex".to_string(),
        }),
    }
}

/// Open the Cursor login window (WebView-based login)
#[command]
pub async fn open_cursor_login(app: AppHandle) -> Result<(), String> {
    tray::create_cursor_login_window(&app).map_err(|e| e.to_string())
}

/// Close the Cursor login window
#[command]
pub async fn close_cursor_login(app: AppHandle) -> Result<(), String> {
    tray::close_cursor_login_window(&app).map_err(|e| e.to_string())
}

/// Import Cursor cookies from system browsers (Chrome, Safari, etc.)
/// This is the recommended method - reads cookies directly from installed browsers
#[command]
pub async fn import_cursor_browser_cookies(app: AppHandle) -> Result<LoginResult, String> {
    tracing::info!("Importing Cursor cookies from system browsers");

    match crate::browser_cookies::import_cursor_cookies_from_browser().await {
        Ok(result) => {
            tracing::info!(
                "Successfully imported {} cookies from {}",
                result.cookie_count,
                result.browser_name
            );

            // Store the cookies
            match login::store_cursor_session(result.cookie_header).await {
                Ok(()) => {
                    // Emit login completed event
                    let _ = app.emit("login-completed", serde_json::json!({
                        "providerId": "cursor",
                        "success": true,
                        "message": format!("Imported {} cookies from {}", result.cookie_count, result.browser_name),
                    }));

                    Ok(LoginResult {
                        success: true,
                        message: format!(
                            "Imported {} cookies from {}! Cursor is now connected.",
                            result.cookie_count, result.browser_name
                        ),
                        provider_id: "cursor".to_string(),
                    })
                }
                Err(e) => Ok(LoginResult {
                    success: false,
                    message: format!("Failed to save imported cookies: {}", e),
                    provider_id: "cursor".to_string(),
                }),
            }
        }
        Err(e) => {
            tracing::warn!("Failed to import browser cookies: {}", e);
            Ok(LoginResult {
                success: false,
                message: format!(
                    "Could not import cookies from browsers: {}. Make sure you're logged into cursor.com in Chrome or Safari, then try again.",
                    e
                ),
                provider_id: "cursor".to_string(),
            })
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserCookieSourceRequest {
    pub source: String,
}

fn parse_cookie_source(source: &str) -> Result<BrowserCookieSource, String> {
    match source {
        "chrome" => Ok(BrowserCookieSource::Chrome),
        "safari" => Ok(BrowserCookieSource::Safari),
        "firefox" => Ok(BrowserCookieSource::Firefox),
        "arc" => Ok(BrowserCookieSource::Arc),
        "edge" => Ok(BrowserCookieSource::Edge),
        "brave" => Ok(BrowserCookieSource::Brave),
        "opera" => Ok(BrowserCookieSource::Opera),
        "manual" => Err("Manual cookie import requires pasting a Cookie header".to_string()),
        _ => Err(format!("Unsupported cookie source: {}", source)),
    }
}

fn extract_amp_session_cookie(cookie_header: &str) -> Result<String, String> {
    let mut parts = Vec::new();

    for part in cookie_header.split(';') {
        let trimmed = part.trim();
        if let Some((name, value)) = trimmed.split_once('=') {
            if name.trim() == "session" {
                let value = value.trim();
                if !value.is_empty() {
                    parts.push(format!("session={}", value));
                }
            }
        }
    }

    if parts.is_empty() {
        Err("No Amp session cookie found".to_string())
    } else {
        Ok(parts.join("; "))
    }
}

#[command]
pub async fn import_cursor_browser_cookies_from_source(
    app: AppHandle,
    source: BrowserCookieSourceRequest,
) -> Result<LoginResult, String> {
    tracing::info!("Importing Cursor cookies from {:?}", source.source);

    let parsed = parse_cookie_source(source.source.trim())?;

    match crate::browser_cookies::import_cursor_cookies_from_browser_source(parsed).await {
        Ok(result) => {
            tracing::info!(
                "Successfully imported {} cookies from {}",
                result.cookie_count,
                result.browser_name
            );

            match login::store_cursor_session(result.cookie_header).await {
                Ok(()) => {
                    let _ = app.emit("login-completed", serde_json::json!({
                        "providerId": "cursor",
                        "success": true,
                        "message": format!("Imported {} cookies from {}", result.cookie_count, result.browser_name),
                    }));

                    Ok(LoginResult {
                        success: true,
                        message: format!(
                            "Imported {} cookies from {}! Cursor is now connected.",
                            result.cookie_count, result.browser_name
                        ),
                        provider_id: "cursor".to_string(),
                    })
                }
                Err(e) => Ok(LoginResult {
                    success: false,
                    message: format!("Failed to save imported cookies: {}", e),
                    provider_id: "cursor".to_string(),
                }),
            }
        }
        Err(e) => {
            tracing::warn!("Failed to import browser cookies: {}", e);
            Ok(LoginResult {
                success: false,
                message: format!(
                    "Could not import cookies from {}: {}. Make sure you're logged into cursor.com and try again.",
                    parsed.as_label(),
                    e
                ),
                provider_id: "cursor".to_string(),
            })
        }
    }
}

/// Import Factory cookies from system browsers (Chrome, Safari, etc.)
#[command]
pub async fn import_factory_browser_cookies(app: AppHandle) -> Result<LoginResult, String> {
    tracing::info!("Importing Factory cookies from system browsers");

    match crate::browser_cookies::import_factory_cookies_from_browser().await {
        Ok(result) => {
            tracing::info!(
                "Successfully imported {} cookies from {}",
                result.cookie_count,
                result.browser_name
            );

            match login::store_factory_session(result.cookie_header).await {
                Ok(()) => {
                    let _ = app.emit("login-completed", serde_json::json!({
                        "providerId": "factory",
                        "success": true,
                        "message": format!("Imported {} cookies from {}", result.cookie_count, result.browser_name),
                    }));

                    Ok(LoginResult {
                        success: true,
                        message: format!(
                            "Imported {} cookies from {}! Factory is now connected.",
                            result.cookie_count, result.browser_name
                        ),
                        provider_id: "factory".to_string(),
                    })
                }
                Err(e) => Ok(LoginResult {
                    success: false,
                    message: format!("Failed to save imported cookies: {}", e),
                    provider_id: "factory".to_string(),
                }),
            }
        }
        Err(e) => {
            tracing::warn!("Failed to import Factory browser cookies: {}", e);
            Ok(LoginResult {
                success: false,
                message: format!(
                    "Could not import cookies from browsers: {}. Make sure you're logged into app.factory.ai in Chrome or Safari, then try again.",
                    e
                ),
                provider_id: "factory".to_string(),
            })
        }
    }
}

#[command]
pub async fn import_factory_browser_cookies_from_source(
    app: AppHandle,
    source: BrowserCookieSourceRequest,
) -> Result<LoginResult, String> {
    tracing::info!("Importing Factory cookies from {:?}", source.source);

    let parsed = parse_cookie_source(source.source.trim())?;

    match crate::browser_cookies::import_factory_cookies_from_browser_source(parsed).await {
        Ok(result) => {
            tracing::info!(
                "Successfully imported {} cookies from {}",
                result.cookie_count,
                result.browser_name
            );

            match login::store_factory_session(result.cookie_header).await {
                Ok(()) => {
                    let _ = app.emit("login-completed", serde_json::json!({
                        "providerId": "factory",
                        "success": true,
                        "message": format!("Imported {} cookies from {}", result.cookie_count, result.browser_name),
                    }));

                    Ok(LoginResult {
                        success: true,
                        message: format!(
                            "Imported {} cookies from {}! Factory is now connected.",
                            result.cookie_count, result.browser_name
                        ),
                        provider_id: "factory".to_string(),
                    })
                }
                Err(e) => Ok(LoginResult {
                    success: false,
                    message: format!("Failed to save imported cookies: {}", e),
                    provider_id: "factory".to_string(),
                }),
            }
        }
        Err(e) => {
            tracing::warn!("Failed to import Factory browser cookies: {}", e);
            Ok(LoginResult {
                success: false,
                message: format!(
                    "Could not import cookies from {}: {}. Make sure you're logged into app.factory.ai and try again.",
                    parsed.as_label(),
                    e
                ),
                provider_id: "factory".to_string(),
            })
        }
    }
}

/// Import Augment cookies from system browsers (Chrome, Safari, etc.)
#[command]
pub async fn import_augment_browser_cookies(app: AppHandle) -> Result<LoginResult, String> {
    tracing::info!("Importing Augment cookies from system browsers");

    match crate::browser_cookies::import_augment_cookies_from_browser().await {
        Ok(result) => {
            tracing::info!(
                "Successfully imported {} cookies from {}",
                result.cookie_count,
                result.browser_name
            );

            match login::store_augment_session(result.cookie_header).await {
                Ok(()) => {
                    let _ = app.emit("login-completed", serde_json::json!({
                        "providerId": "augment",
                        "success": true,
                        "message": format!("Imported {} cookies from {}", result.cookie_count, result.browser_name),
                    }));

                    Ok(LoginResult {
                        success: true,
                        message: format!(
                            "Imported {} cookies from {}! Augment is now connected.",
                            result.cookie_count, result.browser_name
                        ),
                        provider_id: "augment".to_string(),
                    })
                }
                Err(e) => Ok(LoginResult {
                    success: false,
                    message: format!("Failed to save imported cookies: {}", e),
                    provider_id: "augment".to_string(),
                }),
            }
        }
        Err(e) => {
            tracing::warn!("Failed to import Augment browser cookies: {}", e);
            Ok(LoginResult {
                success: false,
                message: format!(
                    "Could not import cookies from browsers: {}. Make sure you're logged into app.augmentcode.com in Chrome or Safari, then try again.",
                    e
                ),
                provider_id: "augment".to_string(),
            })
        }
    }
}

#[command]
pub async fn import_augment_browser_cookies_from_source(
    app: AppHandle,
    source: BrowserCookieSourceRequest,
) -> Result<LoginResult, String> {
    tracing::info!("Importing Augment cookies from {:?}", source.source);

    let parsed = parse_cookie_source(source.source.trim())?;

    match crate::browser_cookies::import_augment_cookies_from_browser_source(parsed).await {
        Ok(result) => {
            tracing::info!(
                "Successfully imported {} cookies from {}",
                result.cookie_count,
                result.browser_name
            );

            match login::store_augment_session(result.cookie_header).await {
                Ok(()) => {
                    let _ = app.emit("login-completed", serde_json::json!({
                        "providerId": "augment",
                        "success": true,
                        "message": format!("Imported {} cookies from {}", result.cookie_count, result.browser_name),
                    }));

                    Ok(LoginResult {
                        success: true,
                        message: format!(
                            "Imported {} cookies from {}! Augment is now connected.",
                            result.cookie_count, result.browser_name
                        ),
                        provider_id: "augment".to_string(),
                    })
                }
                Err(e) => Ok(LoginResult {
                    success: false,
                    message: format!("Failed to save imported cookies: {}", e),
                    provider_id: "augment".to_string(),
                }),
            }
        }
        Err(e) => {
            tracing::warn!("Failed to import Augment browser cookies: {}", e);
            Ok(LoginResult {
                success: false,
                message: format!(
                    "Could not import cookies from {}: {}. Make sure you're logged into app.augmentcode.com and try again.",
                    parsed.as_label(),
                    e
                ),
                provider_id: "augment".to_string(),
            })
        }
    }
}

/// Import Kimi cookies from system browsers (Chrome, Safari, etc.)
#[command]
pub async fn import_kimi_browser_cookies(app: AppHandle) -> Result<LoginResult, String> {
    tracing::info!("Importing Kimi cookies from system browsers");

    match crate::browser_cookies::import_kimi_cookies_from_browser().await {
        Ok(result) => {
            tracing::info!(
                "Successfully imported {} cookies from {}",
                result.cookie_count,
                result.browser_name
            );

            match login::store_kimi_session(result.cookie_header).await {
                Ok(()) => {
                    let _ = app.emit("login-completed", serde_json::json!({
                        "providerId": "kimi",
                        "success": true,
                        "message": format!("Imported {} cookies from {}", result.cookie_count, result.browser_name),
                    }));

                    Ok(LoginResult {
                        success: true,
                        message: format!(
                            "Imported {} cookies from {}! Kimi is now connected.",
                            result.cookie_count, result.browser_name
                        ),
                        provider_id: "kimi".to_string(),
                    })
                }
                Err(e) => Ok(LoginResult {
                    success: false,
                    message: format!("Failed to save imported cookies: {}", e),
                    provider_id: "kimi".to_string(),
                }),
            }
        }
        Err(e) => {
            tracing::warn!("Failed to import Kimi browser cookies: {}", e);
            Ok(LoginResult {
                success: false,
                message: format!(
                    "Could not import cookies from browsers: {}. Make sure you're logged into kimi.moonshot.cn in Chrome or Safari, then try again.",
                    e
                ),
                provider_id: "kimi".to_string(),
            })
        }
    }
}

#[command]
pub async fn import_kimi_browser_cookies_from_source(
    app: AppHandle,
    source: BrowserCookieSourceRequest,
) -> Result<LoginResult, String> {
    tracing::info!("Importing Kimi cookies from {:?}", source.source);

    let parsed = parse_cookie_source(source.source.trim())?;

    match crate::browser_cookies::import_kimi_cookies_from_browser_source(parsed).await {
        Ok(result) => {
            tracing::info!(
                "Successfully imported {} cookies from {}",
                result.cookie_count,
                result.browser_name
            );

            match login::store_kimi_session(result.cookie_header).await {
                Ok(()) => {
                    let _ = app.emit("login-completed", serde_json::json!({
                        "providerId": "kimi",
                        "success": true,
                        "message": format!("Imported {} cookies from {}", result.cookie_count, result.browser_name),
                    }));

                    Ok(LoginResult {
                        success: true,
                        message: format!(
                            "Imported {} cookies from {}! Kimi is now connected.",
                            result.cookie_count, result.browser_name
                        ),
                        provider_id: "kimi".to_string(),
                    })
                }
                Err(e) => Ok(LoginResult {
                    success: false,
                    message: format!("Failed to save imported cookies: {}", e),
                    provider_id: "kimi".to_string(),
                }),
            }
        }
        Err(e) => {
            tracing::warn!("Failed to import Kimi browser cookies: {}", e);
            Ok(LoginResult {
                success: false,
                message: format!(
                    "Could not import cookies from {}: {}. Make sure you're logged into kimi.moonshot.cn and try again.",
                    parsed.as_label(),
                    e
                ),
                provider_id: "kimi".to_string(),
            })
        }
    }
}

/// Import MiniMax cookies from system browsers (Chrome, Safari, etc.)
#[command]
pub async fn import_minimax_browser_cookies(app: AppHandle) -> Result<LoginResult, String> {
    tracing::info!("Importing MiniMax cookies from system browsers");

    match crate::browser_cookies::import_minimax_cookies_from_browser().await {
        Ok(result) => {
            tracing::info!(
                "Successfully imported {} cookies from {}",
                result.cookie_count,
                result.browser_name
            );

            match login::store_minimax_session(result.cookie_header).await {
                Ok(()) => {
                    let _ = app.emit("login-completed", serde_json::json!({
                        "providerId": "minimax",
                        "success": true,
                        "message": format!("Imported {} cookies from {}", result.cookie_count, result.browser_name),
                    }));

                    Ok(LoginResult {
                        success: true,
                        message: format!(
                            "Imported {} cookies from {}! MiniMax is now connected.",
                            result.cookie_count, result.browser_name
                        ),
                        provider_id: "minimax".to_string(),
                    })
                }
                Err(e) => Ok(LoginResult {
                    success: false,
                    message: format!("Failed to save imported cookies: {}", e),
                    provider_id: "minimax".to_string(),
                }),
            }
        }
        Err(e) => {
            tracing::warn!("Failed to import MiniMax browser cookies: {}", e);
            Ok(LoginResult {
                success: false,
                message: format!(
                    "Could not import cookies from browsers: {}. Make sure you're logged into platform.minimax.io in Chrome or Safari, then try again.",
                    e
                ),
                provider_id: "minimax".to_string(),
            })
        }
    }
}

#[command]
pub async fn import_minimax_browser_cookies_from_source(
    app: AppHandle,
    source: BrowserCookieSourceRequest,
) -> Result<LoginResult, String> {
    tracing::info!("Importing MiniMax cookies from {:?}", source.source);

    let parsed = parse_cookie_source(source.source.trim())?;

    match crate::browser_cookies::import_minimax_cookies_from_browser_source(parsed).await {
        Ok(result) => {
            tracing::info!(
                "Successfully imported {} cookies from {}",
                result.cookie_count,
                result.browser_name
            );

            match login::store_minimax_session(result.cookie_header).await {
                Ok(()) => {
                    let _ = app.emit("login-completed", serde_json::json!({
                        "providerId": "minimax",
                        "success": true,
                        "message": format!("Imported {} cookies from {}", result.cookie_count, result.browser_name),
                    }));

                    Ok(LoginResult {
                        success: true,
                        message: format!(
                            "Imported {} cookies from {}! MiniMax is now connected.",
                            result.cookie_count, result.browser_name
                        ),
                        provider_id: "minimax".to_string(),
                    })
                }
                Err(e) => Ok(LoginResult {
                    success: false,
                    message: format!("Failed to save imported cookies: {}", e),
                    provider_id: "minimax".to_string(),
                }),
            }
        }
        Err(e) => {
            tracing::warn!("Failed to import MiniMax browser cookies: {}", e);
            Ok(LoginResult {
                success: false,
                message: format!(
                    "Could not import cookies from {}: {}. Make sure you're logged into platform.minimax.io and try again.",
                    parsed.as_label(),
                    e
                ),
                provider_id: "minimax".to_string(),
            })
        }
    }
}

/// Import Amp cookies from system browsers (Chrome, Safari, etc.)
#[command]
pub async fn import_amp_browser_cookies(app: AppHandle) -> Result<LoginResult, String> {
    tracing::info!("Importing Amp cookies from system browsers");

    match crate::browser_cookies::import_amp_cookies_from_browser().await {
        Ok(result) => {
            tracing::info!(
                "Successfully imported {} cookies from {}",
                result.cookie_count,
                result.browser_name
            );

            let session_cookie = match extract_amp_session_cookie(&result.cookie_header) {
                Ok(cookie) => cookie,
                Err(e) => {
                    return Ok(LoginResult {
                        success: false,
                        message: e,
                        provider_id: "amp".to_string(),
                    });
                }
            };

            match login::store_amp_session(session_cookie).await {
                Ok(()) => {
                    let _ = app.emit("login-completed", serde_json::json!({
                        "providerId": "amp",
                        "success": true,
                        "message": format!("Imported {} cookies from {}", result.cookie_count, result.browser_name),
                    }));

                    Ok(LoginResult {
                        success: true,
                        message: format!(
                            "Imported {} cookies from {}! Amp is now connected.",
                            result.cookie_count, result.browser_name
                        ),
                        provider_id: "amp".to_string(),
                    })
                }
                Err(e) => Ok(LoginResult {
                    success: false,
                    message: format!("Failed to save imported cookies: {}", e),
                    provider_id: "amp".to_string(),
                }),
            }
        }
        Err(e) => {
            tracing::warn!("Failed to import Amp browser cookies: {}", e);
            Ok(LoginResult {
                success: false,
                message: format!(
                    "Could not import cookies from browsers: {}. Make sure you're logged into ampcode.com in Chrome or Safari, then try again.",
                    e
                ),
                provider_id: "amp".to_string(),
            })
        }
    }
}

#[command]
pub async fn import_amp_browser_cookies_from_source(
    app: AppHandle,
    source: BrowserCookieSourceRequest,
) -> Result<LoginResult, String> {
    tracing::info!("Importing Amp cookies from {:?}", source.source);

    let parsed = parse_cookie_source(source.source.trim())?;

    match crate::browser_cookies::import_amp_cookies_from_browser_source(parsed).await {
        Ok(result) => {
            tracing::info!(
                "Successfully imported {} cookies from {}",
                result.cookie_count,
                result.browser_name
            );

            let session_cookie = match extract_amp_session_cookie(&result.cookie_header) {
                Ok(cookie) => cookie,
                Err(e) => {
                    return Ok(LoginResult {
                        success: false,
                        message: e,
                        provider_id: "amp".to_string(),
                    });
                }
            };

            match login::store_amp_session(session_cookie).await {
                Ok(()) => {
                    let _ = app.emit("login-completed", serde_json::json!({
                        "providerId": "amp",
                        "success": true,
                        "message": format!("Imported {} cookies from {}", result.cookie_count, result.browser_name),
                    }));

                    Ok(LoginResult {
                        success: true,
                        message: format!(
                            "Imported {} cookies from {}! Amp is now connected.",
                            result.cookie_count, result.browser_name
                        ),
                        provider_id: "amp".to_string(),
                    })
                }
                Err(e) => Ok(LoginResult {
                    success: false,
                    message: format!("Failed to save imported cookies: {}", e),
                    provider_id: "amp".to_string(),
                }),
            }
        }
        Err(e) => {
            tracing::warn!("Failed to import Amp browser cookies: {}", e);
            Ok(LoginResult {
                success: false,
                message: format!(
                    "Could not import cookies from {}: {}. Make sure you're logged into ampcode.com and try again.",
                    parsed.as_label(),
                    e
                ),
                provider_id: "amp".to_string(),
            })
        }
    }
}

/// Import OpenCode cookies from system browsers (Chrome, Safari, etc.)
#[command]
pub async fn import_opencode_browser_cookies(app: AppHandle) -> Result<LoginResult, String> {
    tracing::info!("Importing OpenCode cookies from system browsers");

    match crate::browser_cookies::import_opencode_cookies_from_browser().await {
        Ok(result) => {
            tracing::info!(
                "Successfully imported {} cookies from {}",
                result.cookie_count,
                result.browser_name
            );

            if !crate::providers::opencode::cookie_header_has_auth(&result.cookie_header) {
                return Ok(LoginResult {
                    success: false,
                    message: "Imported cookies did not include OpenCode auth cookie".to_string(),
                    provider_id: "opencode".to_string(),
                });
            }

            match login::store_opencode_session(result.cookie_header).await {
                Ok(()) => {
                    let _ = app.emit("login-completed", serde_json::json!({
                        "providerId": "opencode",
                        "success": true,
                        "message": format!("Imported {} cookies from {}", result.cookie_count, result.browser_name),
                    }));

                    Ok(LoginResult {
                        success: true,
                        message: format!(
                            "Imported {} cookies from {}! OpenCode is now connected.",
                            result.cookie_count, result.browser_name
                        ),
                        provider_id: "opencode".to_string(),
                    })
                }
                Err(e) => Ok(LoginResult {
                    success: false,
                    message: format!("Failed to save imported cookies: {}", e),
                    provider_id: "opencode".to_string(),
                }),
            }
        }
        Err(e) => {
            tracing::warn!("Failed to import OpenCode browser cookies: {}", e);
            Ok(LoginResult {
                success: false,
                message: format!(
                    "Could not import cookies from browsers: {}. Make sure you're logged into opencode.ai in Chrome or Safari, then try again.",
                    e
                ),
                provider_id: "opencode".to_string(),
            })
        }
    }
}

#[command]
pub async fn import_opencode_browser_cookies_from_source(
    app: AppHandle,
    source: BrowserCookieSourceRequest,
) -> Result<LoginResult, String> {
    tracing::info!("Importing OpenCode cookies from {:?}", source.source);

    let parsed = parse_cookie_source(source.source.trim())?;

    match crate::browser_cookies::import_opencode_cookies_from_browser_source(parsed).await {
        Ok(result) => {
            tracing::info!(
                "Successfully imported {} cookies from {}",
                result.cookie_count,
                result.browser_name
            );

            if !crate::providers::opencode::cookie_header_has_auth(&result.cookie_header) {
                return Ok(LoginResult {
                    success: false,
                    message: "Imported cookies did not include OpenCode auth cookie".to_string(),
                    provider_id: "opencode".to_string(),
                });
            }

            match login::store_opencode_session(result.cookie_header).await {
                Ok(()) => {
                    let _ = app.emit("login-completed", serde_json::json!({
                        "providerId": "opencode",
                        "success": true,
                        "message": format!("Imported {} cookies from {}", result.cookie_count, result.browser_name),
                    }));

                    Ok(LoginResult {
                        success: true,
                        message: format!(
                            "Imported {} cookies from {}! OpenCode is now connected.",
                            result.cookie_count, result.browser_name
                        ),
                        provider_id: "opencode".to_string(),
                    })
                }
                Err(e) => Ok(LoginResult {
                    success: false,
                    message: format!("Failed to save imported cookies: {}", e),
                    provider_id: "opencode".to_string(),
                }),
            }
        }
        Err(e) => {
            tracing::warn!("Failed to import OpenCode browser cookies: {}", e);
            Ok(LoginResult {
                success: false,
                message: format!(
                    "Could not import cookies from {}: {}. Make sure you're logged into opencode.ai and try again.",
                    parsed.as_label(),
                    e
                ),
                provider_id: "opencode".to_string(),
            })
        }
    }
}

/// Import Codex cookies from system browsers (Chrome, Safari, etc.)
#[command]
pub async fn import_codex_browser_cookies(app: AppHandle) -> Result<LoginResult, String> {
    tracing::info!("Importing Codex cookies from system browsers");

    match crate::browser_cookies::import_codex_cookies_from_browser().await {
        Ok(result) => {
            tracing::info!(
                "Successfully imported {} cookies from {}",
                result.cookie_count,
                result.browser_name
            );

            match login::store_codex_session(result.cookie_header).await {
                Ok(()) => {
                    let _ = app.emit("login-completed", serde_json::json!({
                        "providerId": "codex",
                        "success": true,
                        "message": format!("Imported {} cookies from {}", result.cookie_count, result.browser_name),
                    }));

                    Ok(LoginResult {
                        success: true,
                        message: format!(
                            "Imported {} cookies from {}! Codex web extras are now connected.",
                            result.cookie_count, result.browser_name
                        ),
                        provider_id: "codex".to_string(),
                    })
                }
                Err(e) => Ok(LoginResult {
                    success: false,
                    message: format!("Failed to save imported cookies: {}", e),
                    provider_id: "codex".to_string(),
                }),
            }
        }
        Err(e) => {
            tracing::warn!("Failed to import Codex browser cookies: {}", e);
            Ok(LoginResult {
                success: false,
                message: format!(
                    "Could not import cookies from browsers: {}. Make sure you're logged into chatgpt.com in Chrome or Safari, then try again.",
                    e
                ),
                provider_id: "codex".to_string(),
            })
        }
    }
}

#[command]
pub async fn import_codex_browser_cookies_from_source(
    app: AppHandle,
    source: BrowserCookieSourceRequest,
) -> Result<LoginResult, String> {
    tracing::info!("Importing Codex cookies from {:?}", source.source);

    let parsed = parse_cookie_source(source.source.trim())?;

    match crate::browser_cookies::import_codex_cookies_from_browser_source(parsed).await {
        Ok(result) => {
            tracing::info!(
                "Successfully imported {} cookies from {}",
                result.cookie_count,
                result.browser_name
            );

            match login::store_codex_session(result.cookie_header).await {
                Ok(()) => {
                    let _ = app.emit("login-completed", serde_json::json!({
                        "providerId": "codex",
                        "success": true,
                        "message": format!("Imported {} cookies from {}", result.cookie_count, result.browser_name),
                    }));

                    Ok(LoginResult {
                        success: true,
                        message: format!(
                            "Imported {} cookies from {}! Codex web extras are now connected.",
                            result.cookie_count, result.browser_name
                        ),
                        provider_id: "codex".to_string(),
                    })
                }
                Err(e) => Ok(LoginResult {
                    success: false,
                    message: format!("Failed to save imported cookies: {}", e),
                    provider_id: "codex".to_string(),
                }),
            }
        }
        Err(e) => {
            tracing::warn!("Failed to import Codex browser cookies: {}", e);
            Ok(LoginResult {
                success: false,
                message: format!(
                    "Could not import cookies from {}: {}. Make sure you're logged into chatgpt.com and try again.",
                    parsed.as_label(),
                    e
                ),
                provider_id: "codex".to_string(),
            })
        }
    }
}

/// Extract cookies from the Cursor login window automatically
/// This is called after the user logs in to cursor.com in the webview
/// NOTE: Due to Tauri limitations with HTTP-only cookies, this may not work reliably.
/// Prefer using import_cursor_browser_cookies() instead.
#[command]
pub async fn extract_cursor_cookies(app: AppHandle) -> Result<LoginResult, String> {
    tracing::info!("Attempting to extract cookies from Cursor login window");

    // First try the webview cookie extraction
    match tray::extract_cursor_cookies(&app).await {
        Ok(Some(cookie_header)) if !cookie_header.is_empty() => {
            // Store the cookies
            match login::store_cursor_session(cookie_header).await {
                Ok(()) => {
                    // Close the login window
                    let _ = tray::close_cursor_login_window(&app);

                    // Emit login completed event
                    let _ = app.emit(
                        "login-completed",
                        serde_json::json!({
                            "providerId": "cursor",
                            "success": true,
                            "message": "Cursor cookies extracted from webview!",
                        }),
                    );

                    return Ok(LoginResult {
                        success: true,
                        message: "Cursor cookies extracted and saved!".to_string(),
                        provider_id: "cursor".to_string(),
                    });
                }
                Err(e) => {
                    tracing::warn!("Failed to save webview cookies: {}", e);
                }
            }
        }
        Ok(_) => {
            tracing::info!("No cookies from webview, trying browser import...");
        }
        Err(e) => {
            tracing::warn!("Webview cookie extraction failed: {}", e);
        }
    }

    // Fallback: try importing from system browsers
    tracing::info!("Falling back to browser cookie import");
    import_cursor_browser_cookies(app).await
}

// ============== Copilot Device Flow Commands ==============

/// Response for Copilot device code request
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopilotDeviceCodeResponse {
    pub user_code: String,
    pub verification_uri: String,
    pub device_code: String,
    pub expires_in: i32,
    pub interval: i32,
}

/// Request a device code for Copilot login (step 1)
/// Returns the user code to display and device code for polling
#[command]
pub async fn copilot_request_device_code() -> Result<CopilotDeviceCodeResponse, String> {
    tracing::info!("Requesting Copilot device code");

    let client = reqwest::Client::new();
    let response = client
        .post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body("client_id=Iv1.b507a08c87ecfe98&scope=read:user")
        .send()
        .await
        .map_err(|e| format!("Failed to request device code: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Device code request failed ({}): {}", status, body));
    }

    let device_code: login::DeviceCodeResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse device code response: {}", e))?;

    tracing::info!(
        "Got device code. User code: {}",
        debug_settings::redact_value(&device_code.user_code)
    );

    Ok(CopilotDeviceCodeResponse {
        user_code: device_code.user_code,
        verification_uri: device_code.verification_uri,
        device_code: device_code.device_code,
        expires_in: device_code.expires_in,
        interval: device_code.interval,
    })
}

/// Poll for Copilot access token (step 2)
/// Called after user has entered the code on GitHub
#[command]
pub async fn copilot_poll_for_token(
    device_code: String,
    app: AppHandle,
) -> Result<LoginResult, String> {
    tracing::info!("Polling for Copilot access token");

    let client = reqwest::Client::new();
    let interval = 5u64; // Default polling interval
    let max_attempts = 60; // 5 minutes max

    for attempt in 0..max_attempts {
        if attempt > 0 {
            tokio::time::sleep(tokio::time::Duration::from_secs(interval)).await;
        }

        let response = client
            .post("https://github.com/login/oauth/access_token")
            .header("Accept", "application/json")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(format!(
                "client_id=Iv1.b507a08c87ecfe98&device_code={}&grant_type=urn:ietf:params:oauth:grant-type:device_code",
                device_code
            ))
            .send()
            .await;

        let response = match response {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Token poll request failed: {}", e);
                continue;
            }
        };

        let body = response.text().await.unwrap_or_default();

        // Try to parse as error first
        if let Ok(error_resp) = serde_json::from_str::<serde_json::Value>(&body) {
            if let Some(error) = error_resp.get("error").and_then(|e| e.as_str()) {
                match error {
                    "authorization_pending" => {
                        tracing::debug!("Authorization pending (attempt {})", attempt + 1);
                        continue;
                    }
                    "slow_down" => {
                        tracing::debug!("Slow down requested");
                        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                        continue;
                    }
                    "expired_token" => {
                        return Ok(LoginResult {
                            success: false,
                            message: "Device code expired. Please try again.".to_string(),
                            provider_id: "copilot".to_string(),
                        });
                    }
                    "access_denied" => {
                        return Ok(LoginResult {
                            success: false,
                            message: "Access denied. Please authorize the app.".to_string(),
                            provider_id: "copilot".to_string(),
                        });
                    }
                    _ => {
                        let desc = error_resp
                            .get("error_description")
                            .and_then(|d| d.as_str())
                            .unwrap_or("Unknown error");
                        return Ok(LoginResult {
                            success: false,
                            message: format!("Login failed: {} - {}", error, desc),
                            provider_id: "copilot".to_string(),
                        });
                    }
                }
            }

            // Check for access token
            if let Some(access_token) = error_resp.get("access_token").and_then(|t| t.as_str()) {
                tracing::info!("Copilot login successful!");

                // Store the token
                let data_dir =
                    dirs::data_dir().ok_or_else(|| "Could not find data directory".to_string())?;
                let session_dir = data_dir.join("IncuBar");
                tokio::fs::create_dir_all(&session_dir)
                    .await
                    .map_err(|e| format!("Failed to create session directory: {}", e))?;

                let token_path = session_dir.join("copilot-token.json");
                let content = serde_json::json!({
                    "access_token": access_token,
                    "saved_at": chrono::Utc::now().to_rfc3339(),
                });

                tokio::fs::write(&token_path, serde_json::to_string_pretty(&content).unwrap())
                    .await
                    .map_err(|e| format!("Failed to save token: {}", e))?;

                tracing::info!("Saved Copilot token to {:?}", token_path);

                // Emit login completed event
                let _ = app.emit(
                    "login-completed",
                    serde_json::json!({
                        "providerId": "copilot",
                        "success": true,
                        "message": "Copilot login successful!",
                    }),
                );

                return Ok(LoginResult {
                    success: true,
                    message: "Copilot login successful! Token saved.".to_string(),
                    provider_id: "copilot".to_string(),
                });
            }
        }
    }

    Ok(LoginResult {
        success: false,
        message: "Polling timed out. Please try again.".to_string(),
        provider_id: "copilot".to_string(),
    })
}

// ============== Autostart Commands ==============

/// Get current autostart (launch at login) status
#[command]
pub async fn get_autostart_enabled(app: AppHandle) -> Result<bool, String> {
    let autostart_manager = app.state::<AutoLaunchManager>();
    autostart_manager
        .is_enabled()
        .map_err(|e| format!("Failed to check autostart status: {}", e))
}

/// Enable or disable autostart (launch at login)
#[command]
pub async fn set_autostart_enabled(app: AppHandle, enabled: bool) -> Result<(), String> {
    let autostart_manager = app.state::<AutoLaunchManager>();

    if enabled {
        autostart_manager
            .enable()
            .map_err(|e| format!("Failed to enable autostart: {}", e))?;
        tracing::info!("Autostart enabled");
    } else {
        autostart_manager
            .disable()
            .map_err(|e| format!("Failed to disable autostart: {}", e))?;
        tracing::info!("Autostart disabled");
    }

    Ok(())
}
