//! GitHub Copilot provider implementation
//!
//! Uses GitHub OAuth Device Flow for authentication.
//! Endpoints:
//! - /login/device/code - Request device code
//! - /login/oauth/access_token - Poll for access token
//! - api.github.com/copilot_internal/user - Usage data

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use super::{ProviderFetcher, UsageSnapshot, RateWindow, ProviderIdentity};

/// VS Code's OAuth Client ID for GitHub
const GITHUB_CLIENT_ID: &str = "Iv1.b507a08c87ecfe98";
const GITHUB_SCOPES: &str = "read:user";

const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const ACCESS_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const COPILOT_USER_URL: &str = "https://api.github.com/copilot_internal/user";

pub struct CopilotProvider {
    client: reqwest::Client,
}

impl CopilotProvider {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self { client }
    }

    /// Fetch usage with stored token
    async fn fetch_with_token(&self, token: &str) -> Result<UsageSnapshot, anyhow::Error> {
        let response = self.client
            .get(COPILOT_USER_URL)
            .header("Authorization", format!("token {}", token))
            .header("Accept", "application/json")
            .header("Editor-Version", "vscode/1.96.2")
            .header("Editor-Plugin-Version", "copilot-chat/0.26.7")
            .header("User-Agent", "GitHubCopilotChat/0.26.7")
            .header("X-Github-Api-Version", "2025-04-01")
            .send()
            .await?;

        if response.status() == 401 || response.status() == 403 {
            return Err(anyhow::anyhow!("Authentication required - please login again"));
        }

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Copilot API returned status: {}", response.status()));
        }

        let raw_json = response.text().await?;
        tracing::debug!("Copilot user response: {}", &raw_json);

        let usage: CopilotUsageResponse = serde_json::from_str(&raw_json)?;

        Ok(self.convert_response(usage))
    }

    /// Convert API response to UsageSnapshot
    fn convert_response(&self, usage: CopilotUsageResponse) -> UsageSnapshot {
        // Primary: Premium interactions quota
        let primary = usage.quota_snapshots.premium_interactions.as_ref().map(|q| {
            let used_percent = 100.0 - q.percent_remaining;
            RateWindow {
                used_percent,
                window_minutes: None,
                resets_at: Some(usage.quota_reset_date.clone()),
                reset_description: Some(self.format_reset_description(&usage.quota_reset_date)),
                label: Some("Premium".to_string()),
            }
        });

        // Secondary: Chat quota (if different from premium)
        let secondary = usage.quota_snapshots.chat.as_ref().map(|q| {
            let used_percent = 100.0 - q.percent_remaining;
            RateWindow {
                used_percent,
                window_minutes: None,
                resets_at: Some(usage.quota_reset_date.clone()),
                reset_description: None,
                label: Some("Chat".to_string()),
            }
        });

        // Format plan name
        let plan = Some(capitalize_first(&usage.copilot_plan));

        UsageSnapshot {
            primary,
            secondary,
            tertiary: None,
            credits: None,
            cost: None,
            identity: Some(ProviderIdentity {
                email: None,
                name: None,
                plan,
                organization: None,
            }),
            updated_at: chrono::Utc::now().to_rfc3339(),
            error: None,
        }
    }

    fn format_reset_description(&self, reset_date: &str) -> String {
        if let Ok(date) = chrono::DateTime::parse_from_rfc3339(reset_date) {
            let now = chrono::Utc::now();
            let duration = date.signed_duration_since(now);

            if duration.num_days() < 1 {
                let hours = duration.num_hours().max(1);
                format!("Resets in {}h", hours)
            } else {
                format!("Resets in {} days", duration.num_days())
            }
        } else {
            "Resets monthly".to_string()
        }
    }

    /// Load stored token from session file
    async fn load_stored_token(&self) -> Result<String, anyhow::Error> {
        let session_path = self.get_session_path()?;

        if session_path.exists() {
            let content = tokio::fs::read_to_string(&session_path).await?;
            let session: CopilotSession = serde_json::from_str(&content)?;
            return Ok(session.access_token);
        }

        Err(anyhow::anyhow!("No stored Copilot session found"))
    }

    fn get_session_path(&self) -> Result<std::path::PathBuf, anyhow::Error> {
        let data_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
        Ok(data_dir.join("IncuBar").join("copilot-token.json"))
    }
}

#[async_trait]
impl ProviderFetcher for CopilotProvider {
    fn name(&self) -> &'static str {
        "Copilot"
    }

    fn description(&self) -> &'static str {
        "GitHub Copilot"
    }

    async fn fetch(&self) -> Result<UsageSnapshot, anyhow::Error> {
        tracing::debug!("Fetching Copilot usage");

        // Try to load stored token
        match self.load_stored_token().await {
            Ok(token) => {
                match self.fetch_with_token(&token).await {
                    Ok(usage) => {
                        tracing::debug!("Copilot fetch successful");
                        return Ok(usage);
                    }
                    Err(e) => {
                        tracing::debug!("Copilot fetch with stored token failed: {}", e);
                        return Ok(UsageSnapshot::error(format!("Auth expired: {}", e)));
                    }
                }
            }
            Err(e) => {
                tracing::debug!("No stored Copilot token: {}", e);
            }
        }

        // No token - return error state prompting login
        Ok(UsageSnapshot {
            primary: None,
            secondary: None,
            tertiary: None,
            credits: None,
            cost: None,
            identity: Some(ProviderIdentity {
                email: None,
                name: None,
                plan: Some("Not logged in".to_string()),
                organization: None,
            }),
            updated_at: chrono::Utc::now().to_rfc3339(),
            error: Some("Login required".to_string()),
        })
    }
}

// ============== Device Flow ==============

/// Device code response from GitHub
#[derive(Debug, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: i32,
    pub interval: i32,
}

/// Access token response from GitHub
#[derive(Debug, Deserialize)]
struct AccessTokenResponse {
    access_token: String,
    #[allow(dead_code)]
    token_type: String,
    #[allow(dead_code)]
    scope: String,
}

/// Error response during OAuth polling
#[derive(Debug, Deserialize)]
struct OAuthErrorResponse {
    error: String,
    #[allow(dead_code)]
    error_description: Option<String>,
}

/// Request a device code from GitHub
pub async fn request_device_code() -> Result<DeviceCodeResponse, anyhow::Error> {
    let client = reqwest::Client::new();

    let params = [
        ("client_id", GITHUB_CLIENT_ID),
        ("scope", GITHUB_SCOPES),
    ];

    let response = client
        .post(DEVICE_CODE_URL)
        .header("Accept", "application/json")
        .form(&params)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!("Failed to request device code: {}", response.status()));
    }

    let device_code: DeviceCodeResponse = response.json().await?;
    tracing::info!("Got device code, user should enter: {}", device_code.user_code);

    Ok(device_code)
}

/// Poll for access token after user authorizes
pub async fn poll_for_token(device_code: &str, interval: i32) -> Result<String, anyhow::Error> {
    let client = reqwest::Client::new();

    let params = [
        ("client_id", GITHUB_CLIENT_ID),
        ("device_code", device_code),
        ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
    ];

    let mut current_interval = interval as u64;

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(current_interval)).await;

        let response = client
            .post(ACCESS_TOKEN_URL)
            .header("Accept", "application/json")
            .form(&params)
            .send()
            .await?;

        let body = response.text().await?;

        // Try to parse as error first
        if let Ok(error_resp) = serde_json::from_str::<OAuthErrorResponse>(&body) {
            match error_resp.error.as_str() {
                "authorization_pending" => {
                    tracing::debug!("Authorization pending, continuing to poll...");
                    continue;
                }
                "slow_down" => {
                    current_interval += 5;
                    tracing::debug!("Slowing down, new interval: {}s", current_interval);
                    continue;
                }
                "expired_token" => {
                    return Err(anyhow::anyhow!("Device code expired. Please try again."));
                }
                "access_denied" => {
                    return Err(anyhow::anyhow!("Access denied by user."));
                }
                _ => {
                    return Err(anyhow::anyhow!("OAuth error: {}", error_resp.error));
                }
            }
        }

        // Try to parse as success
        if let Ok(token_resp) = serde_json::from_str::<AccessTokenResponse>(&body) {
            tracing::info!("Successfully obtained access token");
            return Ok(token_resp.access_token);
        }

        return Err(anyhow::anyhow!("Unexpected response from GitHub: {}", body));
    }
}

/// Save token to session file
pub async fn save_token(token: &str) -> Result<(), anyhow::Error> {
    let data_dir = dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
    let session_dir = data_dir.join("IncuBar");

    tokio::fs::create_dir_all(&session_dir).await?;

    let session = serde_json::json!({
        "access_token": token,
        "saved_at": chrono::Utc::now().to_rfc3339(),
    });

    let session_path = session_dir.join("copilot-token.json");
    let content = serde_json::to_string_pretty(&session)?;
    tokio::fs::write(&session_path, content).await?;

    tracing::info!("Saved Copilot token to {:?}", session_path);
    Ok(())
}

// ============== Response Types ==============

#[derive(Debug, Serialize, Deserialize)]
struct CopilotSession {
    access_token: String,
    #[allow(dead_code)]
    saved_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CopilotUsageResponse {
    quota_snapshots: QuotaSnapshots,
    copilot_plan: String,
    #[allow(dead_code)]
    assigned_date: String,
    quota_reset_date: String,
}

#[derive(Debug, Deserialize)]
struct QuotaSnapshots {
    premium_interactions: Option<QuotaSnapshot>,
    chat: Option<QuotaSnapshot>,
}

#[derive(Debug, Deserialize)]
struct QuotaSnapshot {
    #[allow(dead_code)]
    entitlement: f64,
    #[allow(dead_code)]
    remaining: f64,
    percent_remaining: f64,
    #[allow(dead_code)]
    quota_id: String,
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
