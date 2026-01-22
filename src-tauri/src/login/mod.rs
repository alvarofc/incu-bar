//! Login flow implementations for various providers
//!
//! Supports multiple types of login:
//! 1. CLI-based (Claude, Codex, Gemini) - Spawns CLI tool with login command
//! 2. WebView-based (Cursor) - Opens a login window
//! 3. Device Flow (Copilot) - GitHub OAuth device authorization
//! 4. Browser cookie import - Extracts cookies from installed browsers

use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::process::Command;

/// Login result returned to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResult {
    pub success: bool,
    pub message: String,
    pub provider_id: String,
}

/// Login phase for progress updates
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LoginPhase {
    Starting,
    WaitingForBrowser,
    Processing,
    Success,
    Failed,
}

/// Device code response from GitHub
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: i32,
    pub interval: i32,
}

/// Access token response from GitHub
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessTokenResponse {
    pub access_token: Option<String>,
    pub token_type: Option<String>,
    pub scope: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

/// Run Claude CLI login
/// 
/// Claude CLI stores credentials at ~/.claude/.credentials.json
pub async fn run_claude_login() -> Result<LoginResult, anyhow::Error> {
    tracing::info!("Starting Claude login flow");
    
    // Find claude binary
    let claude_path = find_binary("claude").await?;
    
    tracing::debug!("Found claude at: {}", claude_path);
    
    // Run claude /login
    let mut child = Command::new(&claude_path)
        .arg("/login")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to spawn claude: {}", e))?;
    
    // Wait for completion with timeout
    let timeout = tokio::time::Duration::from_secs(120);
    let result = tokio::time::timeout(timeout, child.wait()).await;
    
    match result {
        Ok(Ok(status)) => {
            if status.success() {
                tracing::info!("Claude login successful");
                Ok(LoginResult {
                    success: true,
                    message: "Claude login successful! Credentials saved.".to_string(),
                    provider_id: "claude".to_string(),
                })
            } else {
                let code = status.code().unwrap_or(-1);
                tracing::warn!("Claude login failed with code {}", code);
                Ok(LoginResult {
                    success: false,
                    message: format!("Claude login failed with exit code {}", code),
                    provider_id: "claude".to_string(),
                })
            }
        }
        Ok(Err(e)) => {
            tracing::error!("Claude login error: {}", e);
            Ok(LoginResult {
                success: false,
                message: format!("Claude login error: {}", e),
                provider_id: "claude".to_string(),
            })
        }
        Err(_) => {
            // Timeout - kill the process
            let _ = child.kill().await;
            tracing::warn!("Claude login timed out");
            Ok(LoginResult {
                success: false,
                message: "Claude login timed out after 2 minutes".to_string(),
                provider_id: "claude".to_string(),
            })
        }
    }
}

/// Run Codex CLI login
///
/// Codex CLI stores credentials at ~/.codex/auth.json
pub async fn run_codex_login() -> Result<LoginResult, anyhow::Error> {
    tracing::info!("Starting Codex login flow");
    
    // Find codex binary
    let codex_path = find_binary("codex").await?;
    
    tracing::debug!("Found codex at: {}", codex_path);
    
    // Run codex login
    let mut child = Command::new(&codex_path)
        .arg("login")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to spawn codex: {}", e))?;
    
    // Wait for completion with timeout
    let timeout = tokio::time::Duration::from_secs(120);
    let result = tokio::time::timeout(timeout, child.wait()).await;
    
    match result {
        Ok(Ok(status)) => {
            if status.success() {
                tracing::info!("Codex login successful");
                Ok(LoginResult {
                    success: true,
                    message: "Codex login successful! Credentials saved.".to_string(),
                    provider_id: "codex".to_string(),
                })
            } else {
                let code = status.code().unwrap_or(-1);
                tracing::warn!("Codex login failed with code {}", code);
                Ok(LoginResult {
                    success: false,
                    message: format!("Codex login failed with exit code {}", code),
                    provider_id: "codex".to_string(),
                })
            }
        }
        Ok(Err(e)) => {
            tracing::error!("Codex login error: {}", e);
            Ok(LoginResult {
                success: false,
                message: format!("Codex login error: {}", e),
                provider_id: "codex".to_string(),
            })
        }
        Err(_) => {
            let _ = child.kill().await;
            tracing::warn!("Codex login timed out");
            Ok(LoginResult {
                success: false,
                message: "Codex login timed out after 2 minutes".to_string(),
                provider_id: "codex".to_string(),
            })
        }
    }
}

/// Check authentication status for a provider
pub async fn check_auth_status(provider_id: &str) -> AuthStatus {
    match provider_id {
        "claude" => check_claude_auth().await,
        "codex" => check_codex_auth().await,
        "cursor" => check_cursor_auth().await,
        "copilot" => check_copilot_auth().await,
        "gemini" => check_gemini_auth().await,
        "zai" => check_zai_auth().await,
        "kimi_k2" => check_kimi_k2_auth().await,
        "synthetic" => check_synthetic_auth().await,
        _ => AuthStatus {
            authenticated: false,
            method: None,
            email: None,
            error: Some("Unknown provider".to_string()),
        },
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    pub authenticated: bool,
    pub method: Option<String>,
    pub email: Option<String>,
    pub error: Option<String>,
}

async fn check_claude_auth() -> AuthStatus {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return AuthStatus {
            authenticated: false,
            method: None,
            email: None,
            error: Some("Could not find home directory".to_string()),
        },
    };
    
    let creds_path = home.join(".claude").join(".credentials.json");
    
    if creds_path.exists() {
        // Try to parse and validate
        match tokio::fs::read_to_string(&creds_path).await {
            Ok(content) => {
                if content.contains("claudeAiOauth") && content.contains("accessToken") {
                    AuthStatus {
                        authenticated: true,
                        method: Some("oauth".to_string()),
                        email: None,
                        error: None,
                    }
                } else {
                    AuthStatus {
                        authenticated: false,
                        method: None,
                        email: None,
                        error: Some("Credentials file exists but missing OAuth data".to_string()),
                    }
                }
            }
            Err(e) => AuthStatus {
                authenticated: false,
                method: None,
                email: None,
                error: Some(format!("Could not read credentials: {}", e)),
            },
        }
    } else {
        AuthStatus {
            authenticated: false,
            method: None,
            email: None,
            error: None,
        }
    }
}

async fn check_codex_auth() -> AuthStatus {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return AuthStatus {
            authenticated: false,
            method: None,
            email: None,
            error: Some("Could not find home directory".to_string()),
        },
    };
    
    let auth_path = home.join(".codex").join("auth.json");
    
    if auth_path.exists() {
        match tokio::fs::read_to_string(&auth_path).await {
            Ok(content) => {
                if content.contains("access_token") {
                    AuthStatus {
                        authenticated: true,
                        method: Some("oauth".to_string()),
                        email: None,
                        error: None,
                    }
                } else {
                    AuthStatus {
                        authenticated: false,
                        method: None,
                        email: None,
                        error: Some("Auth file exists but missing access token".to_string()),
                    }
                }
            }
            Err(e) => AuthStatus {
                authenticated: false,
                method: None,
                email: None,
                error: Some(format!("Could not read auth file: {}", e)),
            },
        }
    } else {
        AuthStatus {
            authenticated: false,
            method: None,
            email: None,
            error: None,
        }
    }
}

async fn check_cursor_auth() -> AuthStatus {
    let data_dir = match dirs::data_dir() {
        Some(d) => d,
        None => return AuthStatus {
            authenticated: false,
            method: None,
            email: None,
            error: Some("Could not find data directory".to_string()),
        },
    };
    
    let session_path = data_dir.join("IncuBar").join("cursor-session.json");
    
    if session_path.exists() {
        AuthStatus {
            authenticated: true,
            method: Some("cookies".to_string()),
            email: None,
            error: None,
        }
    } else {
        AuthStatus {
            authenticated: false,
            method: None,
            email: None,
            error: None,
        }
    }
}

async fn check_copilot_auth() -> AuthStatus {
    let data_dir = match dirs::data_dir() {
        Some(d) => d,
        None => return AuthStatus {
            authenticated: false,
            method: None,
            email: None,
            error: Some("Could not find data directory".to_string()),
        },
    };
    
    let token_path = data_dir.join("IncuBar").join("copilot-token.json");
    
    if token_path.exists() {
        match tokio::fs::read_to_string(&token_path).await {
            Ok(content) => {
                if content.contains("access_token") {
                    AuthStatus {
                        authenticated: true,
                        method: Some("device_flow".to_string()),
                        email: None,
                        error: None,
                    }
                } else {
                    AuthStatus {
                        authenticated: false,
                        method: None,
                        email: None,
                        error: Some("Token file exists but missing access token".to_string()),
                    }
                }
            }
            Err(e) => AuthStatus {
                authenticated: false,
                method: None,
                email: None,
                error: Some(format!("Could not read token file: {}", e)),
            },
        }
    } else {
        AuthStatus {
            authenticated: false,
            method: None,
            email: None,
            error: None,
        }
    }
}

async fn check_gemini_auth() -> AuthStatus {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return AuthStatus {
            authenticated: false,
            method: None,
            email: None,
            error: Some("Could not find home directory".to_string()),
        },
    };
    
    let creds_path = home.join(".gemini").join("oauth_creds.json");
    
    if creds_path.exists() {
        match tokio::fs::read_to_string(&creds_path).await {
            Ok(content) => {
                if content.contains("access_token") || content.contains("refresh_token") {
                    AuthStatus {
                        authenticated: true,
                        method: Some("oauth".to_string()),
                        email: None,
                        error: None,
                    }
                } else {
                    AuthStatus {
                        authenticated: false,
                        method: None,
                        email: None,
                        error: Some("Credentials file exists but missing tokens".to_string()),
                    }
                }
            }
            Err(e) => AuthStatus {
                authenticated: false,
                method: None,
                email: None,
                error: Some(format!("Could not read credentials: {}", e)),
            },
        }
    } else {
        AuthStatus {
            authenticated: false,
            method: None,
            email: None,
            error: None,
        }
    }
}

async fn check_zai_auth() -> AuthStatus {
    // z.ai uses Z_AI_API_KEY environment variable
    if std::env::var("Z_AI_API_KEY").is_ok() {
        AuthStatus {
            authenticated: true,
            method: Some("api_key".to_string()),
            email: None,
            error: None,
        }
    } else {
        AuthStatus {
            authenticated: false,
            method: None,
            email: None,
            error: Some("Set Z_AI_API_KEY environment variable".to_string()),
        }
    }
}

async fn check_kimi_k2_auth() -> AuthStatus {
    // Kimi K2 uses KIMI_K2_API_KEY, KIMI_API_KEY, or KIMI_KEY environment variable
    if std::env::var("KIMI_K2_API_KEY").is_ok() 
        || std::env::var("KIMI_API_KEY").is_ok() 
        || std::env::var("KIMI_KEY").is_ok() {
        AuthStatus {
            authenticated: true,
            method: Some("api_key".to_string()),
            email: None,
            error: None,
        }
    } else {
        AuthStatus {
            authenticated: false,
            method: None,
            email: None,
            error: Some("Set KIMI_K2_API_KEY environment variable".to_string()),
        }
    }
}

async fn check_synthetic_auth() -> AuthStatus {
    // Synthetic uses SYNTHETIC_API_KEY environment variable
    if std::env::var("SYNTHETIC_API_KEY").is_ok() {
        AuthStatus {
            authenticated: true,
            method: Some("api_key".to_string()),
            email: None,
            error: None,
        }
    } else {
        AuthStatus {
            authenticated: false,
            method: None,
            email: None,
            error: Some("Set SYNTHETIC_API_KEY environment variable".to_string()),
        }
    }
}

/// Find a binary in PATH or common locations
async fn find_binary(name: &str) -> Result<String, anyhow::Error> {
    // Try which first
    let output = Command::new("which")
        .arg(name)
        .output()
        .await;
    
    if let Ok(output) = output {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Ok(path);
            }
        }
    }
    
    // Try common paths
    let common_paths = [
        format!("/usr/local/bin/{}", name),
        format!("/opt/homebrew/bin/{}", name),
        format!("{}/.local/bin/{}", std::env::var("HOME").unwrap_or_default(), name),
        format!("{}/.npm-global/bin/{}", std::env::var("HOME").unwrap_or_default(), name),
        format!("{}/.nvm/versions/node/*/bin/{}", std::env::var("HOME").unwrap_or_default(), name),
    ];
    
    for path in &common_paths {
        // Handle glob patterns
        if path.contains('*') {
            if let Ok(entries) = glob::glob(path) {
                for entry in entries.flatten() {
                    if entry.exists() {
                        return Ok(entry.to_string_lossy().to_string());
                    }
                }
            }
        } else if std::path::Path::new(path).exists() {
            return Ok(path.clone());
        }
    }
    
    Err(anyhow::anyhow!(
        "{} not found. Please install it first:\n\
        - Claude: Install Claude CLI from https://claude.ai/cli\n\
        - Codex: npm install -g @openai/codex",
        name
    ))
}

/// Store Cursor session cookies
pub async fn store_cursor_session(cookie_header: String) -> Result<(), anyhow::Error> {
    let data_dir = dirs::data_dir().ok_or_else(|| anyhow::anyhow!("Could not find data directory"))?;
    let session_dir = data_dir.join("IncuBar");
    
    // Create directory if needed
    tokio::fs::create_dir_all(&session_dir).await?;
    
    let session_path = session_dir.join("cursor-session.json");
    let content = serde_json::json!({
        "cookieHeader": cookie_header,
        "savedAt": chrono::Utc::now().to_rfc3339(),
    });
    
    tokio::fs::write(&session_path, serde_json::to_string_pretty(&content)?).await?;
    
    tracing::info!("Saved Cursor session to {:?}", session_path);
    Ok(())
}

// ============== Copilot GitHub Device Flow ==============

const COPILOT_CLIENT_ID: &str = "Iv1.b507a08c87ecfe98"; // VS Code Client ID
const COPILOT_SCOPES: &str = "read:user";

/// Copilot device code for returning to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopilotDeviceCode {
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: i32,
}

/// Run Copilot login via GitHub Device Flow
///
/// 1. Request device code from GitHub
/// 2. Return user code to frontend for display
/// 3. Open browser to verification URL
/// 4. Poll for access token
/// 5. Store token to copilot-token.json
pub async fn run_copilot_login() -> Result<LoginResult, anyhow::Error> {
    tracing::info!("Starting Copilot login via GitHub Device Flow");
    
    // Step 1: Request device code
    let client = reqwest::Client::new();
    let device_code_response = client
        .post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!("client_id={}&scope={}", COPILOT_CLIENT_ID, COPILOT_SCOPES))
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to request device code: {}", e))?;
    
    if !device_code_response.status().is_success() {
        let status = device_code_response.status();
        let body = device_code_response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("Device code request failed ({}): {}", status, body));
    }
    
    let device_code: DeviceCodeResponse = device_code_response
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to parse device code response: {}", e))?;
    
    tracing::info!("Got device code. User code: {}", device_code.user_code);
    
    // Step 2: Copy user code to clipboard and open browser
    #[cfg(target_os = "macos")]
    {
        // Copy to clipboard using pbcopy
        let _ = Command::new("sh")
            .arg("-c")
            .arg(format!("echo -n '{}' | pbcopy", device_code.user_code))
            .output()
            .await;
    }
    
    // Open verification URL in browser
    let _ = Command::new("open")
        .arg(&device_code.verification_uri)
        .spawn();
    
    tracing::info!("Opened browser to {}", device_code.verification_uri);
    
    // Step 3: Poll for access token
    let poll_interval = std::cmp::max(device_code.interval, 5) as u64;
    let expires_at = std::time::Instant::now() + std::time::Duration::from_secs(device_code.expires_in as u64);
    
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(poll_interval)).await;
        
        if std::time::Instant::now() > expires_at {
            return Ok(LoginResult {
                success: false,
                message: "Device code expired. Please try again.".to_string(),
                provider_id: "copilot".to_string(),
            });
        }
        
        let token_response = client
            .post("https://github.com/login/oauth/access_token")
            .header("Accept", "application/json")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(format!(
                "client_id={}&device_code={}&grant_type=urn:ietf:params:oauth:grant-type:device_code",
                COPILOT_CLIENT_ID,
                device_code.device_code
            ))
            .send()
            .await;
        
        let token_response = match token_response {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Token poll request failed: {}", e);
                continue;
            }
        };
        
        let token_result: AccessTokenResponse = match token_response.json().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Failed to parse token response: {}", e);
                continue;
            }
        };
        
        // Check for errors
        if let Some(error) = &token_result.error {
            match error.as_str() {
                "authorization_pending" => {
                    tracing::debug!("Authorization pending, continuing to poll...");
                    continue;
                }
                "slow_down" => {
                    tracing::debug!("Slow down requested, adding delay...");
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
                    let desc = token_result.error_description.as_deref().unwrap_or("Unknown error");
                    return Ok(LoginResult {
                        success: false,
                        message: format!("Login failed: {} - {}", error, desc),
                        provider_id: "copilot".to_string(),
                    });
                }
            }
        }
        
        // Got access token!
        if let Some(access_token) = token_result.access_token {
            tracing::info!("Copilot login successful!");
            
            // Store the token
            let data_dir = dirs::data_dir().ok_or_else(|| anyhow::anyhow!("Could not find data directory"))?;
            let session_dir = data_dir.join("IncuBar");
            tokio::fs::create_dir_all(&session_dir).await?;
            
            let token_path = session_dir.join("copilot-token.json");
            let content = serde_json::json!({
                "access_token": access_token,
                "token_type": token_result.token_type,
                "scope": token_result.scope,
                "saved_at": chrono::Utc::now().to_rfc3339(),
            });
            
            tokio::fs::write(&token_path, serde_json::to_string_pretty(&content)?).await?;
            
            tracing::info!("Saved Copilot token to {:?}", token_path);
            
            return Ok(LoginResult {
                success: true,
                message: "Copilot login successful! Token saved.".to_string(),
                provider_id: "copilot".to_string(),
            });
        }
    }
}

// ============== Gemini CLI Login ==============

/// Run Gemini CLI login
///
/// Gemini CLI stores credentials at ~/.gemini/oauth_creds.json
pub async fn run_gemini_login() -> Result<LoginResult, anyhow::Error> {
    tracing::info!("Starting Gemini login flow");
    
    // Find gemini binary
    let gemini_path = find_binary("gemini").await?;
    
    tracing::debug!("Found gemini at: {}", gemini_path);
    
    // Run gemini (it will prompt for OAuth login)
    // Note: Gemini CLI doesn't have a dedicated login command,
    // running any command will trigger auth if not logged in
    let mut child = Command::new(&gemini_path)
        .arg("--help") // Use help as a benign command to trigger auth
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to spawn gemini: {}", e))?;
    
    // Wait for completion with timeout
    let timeout = tokio::time::Duration::from_secs(120);
    let result = tokio::time::timeout(timeout, child.wait()).await;
    
    // Check if credentials file was created
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    let creds_path = home.join(".gemini").join("oauth_creds.json");
    
    match result {
        Ok(Ok(_)) => {
            if creds_path.exists() {
                tracing::info!("Gemini login successful");
                Ok(LoginResult {
                    success: true,
                    message: "Gemini login successful! Credentials saved.".to_string(),
                    provider_id: "gemini".to_string(),
                })
            } else {
                tracing::warn!("Gemini command completed but no credentials found");
                Ok(LoginResult {
                    success: false,
                    message: "Gemini CLI ran but no credentials were saved. Try running 'gemini' manually in terminal.".to_string(),
                    provider_id: "gemini".to_string(),
                })
            }
        }
        Ok(Err(e)) => {
            tracing::error!("Gemini login error: {}", e);
            Ok(LoginResult {
                success: false,
                message: format!("Gemini login error: {}", e),
                provider_id: "gemini".to_string(),
            })
        }
        Err(_) => {
            let _ = child.kill().await;
            tracing::warn!("Gemini login timed out");
            Ok(LoginResult {
                success: false,
                message: "Gemini login timed out after 2 minutes".to_string(),
                provider_id: "gemini".to_string(),
            })
        }
    }
}
