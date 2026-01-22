//! Tauri IPC commands for the frontend

use serde::{Deserialize, Serialize};
use tauri::{command, AppHandle, State, Emitter, Manager};
use tauri_plugin_autostart::AutoLaunchManager;

use crate::login::{self, AuthStatus, LoginResult};
use crate::providers::{ProviderId, ProviderRegistry, UsageSnapshot};
use crate::tray;

/// Settings structure matching frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub refresh_interval_seconds: u32,
    pub enabled_providers: Vec<ProviderId>,
    pub provider_order: Vec<ProviderId>,
    pub display_mode: String,
    pub show_notifications: bool,
    pub launch_at_login: bool,
    pub show_credits: bool,
    pub show_cost: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            refresh_interval_seconds: 300,
            enabled_providers: vec![
                ProviderId::Claude,
                ProviderId::Codex,
                ProviderId::Cursor,
            ],
            provider_order: vec![
                ProviderId::Claude,
                ProviderId::Codex,
                ProviderId::Cursor,
            ],
            display_mode: "merged".to_string(),
            show_notifications: true,
            launch_at_login: false,
            show_credits: true,
            show_cost: true,
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
    tracing::debug!("Refreshing provider: {:?}", provider_id);

    let usage = registry
        .fetch_usage(&provider_id)
        .await
        .map_err(|e| e.to_string())?;

    // Emit event to frontend
    let _ = app.emit("usage-updated", serde_json::json!({
        "providerId": provider_id,
        "usage": usage,
    }));

    Ok(usage)
}

/// Refresh all enabled providers
#[command]
pub async fn refresh_all_providers(
    registry: State<'_, ProviderRegistry>,
    app: AppHandle,
) -> Result<(), String> {
    tracing::debug!("Refreshing all providers");

    let providers = registry.get_enabled_providers();
    
    for provider_id in providers {
        match registry.fetch_usage(&provider_id).await {
            Ok(usage) => {
                let _ = app.emit("usage-updated", serde_json::json!({
                    "providerId": provider_id,
                    "usage": usage,
                }));
            }
            Err(e) => {
                tracing::warn!("Failed to refresh {:?}: {}", provider_id, e);
                let _ = app.emit("usage-updated", serde_json::json!({
                    "providerId": provider_id,
                    "usage": {
                        "error": e.to_string(),
                        "updatedAt": chrono::Utc::now().to_rfc3339(),
                    },
                }));
            }
        }
    }

    Ok(())
}

/// Get usage for a single provider (cached)
#[command]
pub async fn get_provider_usage(
    provider_id: ProviderId,
    registry: State<'_, ProviderRegistry>,
) -> Result<Option<UsageSnapshot>, String> {
    Ok(registry.get_cached_usage(&provider_id))
}

/// Get all cached usage data
#[command]
pub async fn get_all_usage(
    registry: State<'_, ProviderRegistry>,
) -> Result<std::collections::HashMap<ProviderId, UsageSnapshot>, String> {
    Ok(registry.get_all_cached_usage())
}

/// Enable or disable a provider
#[command]
pub async fn set_provider_enabled(
    provider_id: ProviderId,
    enabled: bool,
    registry: State<'_, ProviderRegistry>,
) -> Result<(), String> {
    registry.set_enabled(&provider_id, enabled);
    Ok(())
}

/// Get current settings
#[command]
pub async fn get_settings() -> Result<AppSettings, String> {
    // TODO: Load from tauri-plugin-store
    Ok(AppSettings::default())
}

/// Save settings
#[command]
pub async fn save_settings(settings: AppSettings) -> Result<(), String> {
    // TODO: Save to tauri-plugin-store
    tracing::debug!("Saving settings: {:?}", settings);
    Ok(())
}

// ============== Login Commands ==============

/// Start login flow for a provider
#[command]
pub async fn start_login(provider_id: String, app: AppHandle) -> Result<LoginResult, String> {
    tracing::info!("Starting login for provider: {}", provider_id);
    
    // Emit login started event
    let _ = app.emit("login-started", serde_json::json!({
        "providerId": provider_id,
    }));
    
    let result = match provider_id.as_str() {
        "claude" => {
            login::run_claude_login().await.map_err(|e| e.to_string())?
        }
        "codex" => {
            login::run_codex_login().await.map_err(|e| e.to_string())?
        }
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
        "kimi" => {
            return Ok(LoginResult {
                success: true,
                message: "Kimi uses browser cookies. Use Import from Browser or paste cookies manually.".to_string(),
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
        "copilot" => {
            login::run_copilot_login().await.map_err(|e| e.to_string())?
        }
        "gemini" => {
            login::run_gemini_login().await.map_err(|e| e.to_string())?
        }
        _ => {
            return Ok(LoginResult {
                success: false,
                message: format!("Login not supported for provider: {}", provider_id),
                provider_id,
            });
        }
    };
    
    // Emit login completed event
    let _ = app.emit("login-completed", serde_json::json!({
        "providerId": provider_id,
        "success": result.success,
        "message": result.message,
    }));
    
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
        "copilot",
        "gemini",
        "zai",
        "kimi",
        "kimi_k2",
        "minimax",
        "synthetic",
        "antigravity",
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
                            result.cookie_count,
                            result.browser_name
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
                            result.cookie_count,
                            result.browser_name
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
                            result.cookie_count,
                            result.browser_name
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
                            result.cookie_count,
                            result.browser_name
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
                            result.cookie_count,
                            result.browser_name
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
                    let _ = app.emit("login-completed", serde_json::json!({
                        "providerId": "cursor",
                        "success": true,
                        "message": "Cursor cookies extracted from webview!",
                    }));
                    
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
    
    tracing::info!("Got device code. User code: {}", device_code.user_code);
    
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
pub async fn copilot_poll_for_token(device_code: String, app: AppHandle) -> Result<LoginResult, String> {
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
                        let desc = error_resp.get("error_description")
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
                let data_dir = dirs::data_dir()
                    .ok_or_else(|| "Could not find data directory".to_string())?;
                let session_dir = data_dir.join("IncuBar");
                tokio::fs::create_dir_all(&session_dir).await
                    .map_err(|e| format!("Failed to create session directory: {}", e))?;
                
                let token_path = session_dir.join("copilot-token.json");
                let content = serde_json::json!({
                    "access_token": access_token,
                    "saved_at": chrono::Utc::now().to_rfc3339(),
                });
                
                tokio::fs::write(&token_path, serde_json::to_string_pretty(&content).unwrap()).await
                    .map_err(|e| format!("Failed to save token: {}", e))?;
                
                tracing::info!("Saved Copilot token to {:?}", token_path);
                
                // Emit login completed event
                let _ = app.emit("login-completed", serde_json::json!({
                    "providerId": "copilot",
                    "success": true,
                    "message": "Copilot login successful!",
                }));
                
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
