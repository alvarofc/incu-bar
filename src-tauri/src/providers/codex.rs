//! Codex (OpenAI) provider implementation
//!
//! Supports two authentication methods:
//! 1. OAuth API (chatgpt.com) - uses credentials from ~/.codex/auth.json
//! 2. OpenAI web cookies (chatgpt.com) - optional extras via browser cookies

use super::{cost_usage, Credits, ProviderFetcher, ProviderId, ProviderIdentity, RateWindow, UsageSnapshot};
use async_trait::async_trait;
use serde::Deserialize;
use std::path::PathBuf;

const DEFAULT_USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";

pub struct CodexProvider {
    client: reqwest::Client,
}

impl CodexProvider {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self { client }
    }

    /// Fetch usage via OAuth API
    async fn fetch_via_oauth(&self) -> Result<UsageSnapshot, anyhow::Error> {
        let auth = self.load_auth_credentials().await?;

        let mut request = self
            .client
            .get(DEFAULT_USAGE_URL)
            .header("Authorization", format!("Bearer {}", auth.access_token))
            .header("Accept", "application/json")
            .header("User-Agent", "IncuBar/1.0");

        if let Some(account_id) = &auth.account_id {
            request = request.header("ChatGPT-Account-Id", account_id);
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Codex API returned status: {}",
                response.status()
            ));
        }

        let usage_response: CodexUsageResponse = response.json().await?;
        Ok(self.convert_response(usage_response))
    }

    async fn fetch_via_cookies(&self) -> Result<UsageSnapshot, anyhow::Error> {
        let cookie_header = self.load_stored_cookies().await?;

        let response = self
            .client
            .get(DEFAULT_USAGE_URL)
            .header("Cookie", cookie_header)
            .header("Accept", "application/json")
            .header("User-Agent", "IncuBar/1.0")
            .send()
            .await?;

        match response.status().as_u16() {
            200 => {}
            401 | 403 => return Err(anyhow::anyhow!("OpenAI session expired")),
            status => {
                return Err(anyhow::anyhow!(
                    "OpenAI web request failed (HTTP {})",
                    status
                ))
            }
        }

        let usage_response: CodexUsageResponse = response.json().await?;
        Ok(self.convert_response(usage_response))
    }

    /// Load OAuth credentials from ~/.codex/auth.json
    async fn load_auth_credentials(&self) -> Result<CodexAuthTokens, anyhow::Error> {
        let auth_path = self.get_auth_path()?;

        if !auth_path.exists() {
            return Err(anyhow::anyhow!(
                "Codex auth file not found at {:?}",
                auth_path
            ));
        }

        let content = tokio::fs::read_to_string(&auth_path).await?;
        let auth_file: CodexAuthFile = serde_json::from_str(&content)?;

        auth_file
            .tokens
            .ok_or_else(|| anyhow::anyhow!("No tokens section in auth file"))
    }

    async fn load_stored_cookies(&self) -> Result<String, anyhow::Error> {
        let session_path = self.get_session_path()?;

        if session_path.exists() {
            let content = tokio::fs::read_to_string(&session_path).await?;
            let session: CodexCookieSession = serde_json::from_str(&content)?;
            return Ok(session.cookie_header);
        }

        Err(anyhow::anyhow!("No stored Codex cookie session found"))
    }

    /// Get the path to Codex auth file
    fn get_auth_path(&self) -> Result<PathBuf, anyhow::Error> {
        // Check CODEX_HOME first, then default to ~/.codex
        if let Ok(codex_home) = std::env::var("CODEX_HOME") {
            return Ok(PathBuf::from(codex_home).join("auth.json"));
        }

        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        Ok(home.join(".codex").join("auth.json"))
    }

    fn get_session_path(&self) -> Result<PathBuf, anyhow::Error> {
        let data_dir =
            dirs::data_dir().ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
        Ok(data_dir.join("IncuBar").join("codex-session.json"))
    }

    /// Convert API response to UsageSnapshot
    fn convert_response(&self, response: CodexUsageResponse) -> UsageSnapshot {
        let primary = response.rate_limit.as_ref().and_then(|rl| {
            rl.primary_window.as_ref().map(|w| {
                let used_percent = w.used_percent.unwrap_or(0) as f64;
                RateWindow {
                    used_percent,
                    window_minutes: w.limit_window_seconds.map(|s| s / 60),
                    resets_at: w.reset_at.map(|ts| {
                        chrono::DateTime::from_timestamp(ts as i64, 0)
                            .map(|dt| dt.to_rfc3339())
                            .unwrap_or_default()
                    }),
                    reset_description: w.reset_at.and_then(|ts| self.format_reset_time(ts as i64)),
                    label: Some("Session".to_string()),
                }
            })
        });

        let secondary = response.rate_limit.as_ref().and_then(|rl| {
            rl.secondary_window.as_ref().map(|w| {
                let used_percent = w.used_percent.unwrap_or(0) as f64;
                RateWindow {
                    used_percent,
                    window_minutes: w.limit_window_seconds.map(|s| s / 60),
                    resets_at: w.reset_at.map(|ts| {
                        chrono::DateTime::from_timestamp(ts as i64, 0)
                            .map(|dt| dt.to_rfc3339())
                            .unwrap_or_default()
                    }),
                    reset_description: w.reset_at.and_then(|ts| self.format_reset_time(ts as i64)),
                    label: Some("Weekly".to_string()),
                }
            })
        });

        let credits = response.credits.as_ref().and_then(|c| {
            if c.unlimited == Some(true) {
                Some(Credits {
                    remaining: f64::INFINITY,
                    total: None,
                    unit: "unlimited".to_string(),
                })
            } else {
                c.balance.map(|b| Credits {
                    remaining: b,
                    total: None,
                    unit: "tokens".to_string(),
                })
            }
        });

        let plan = response.plan_type.map(|p| match p.as_str() {
            "pro" => "Pro".to_string(),
            "plus" => "Plus".to_string(),
            "go" => "Go".to_string(),
            "team" => "Team".to_string(),
            "business" => "Business".to_string(),
            "enterprise" => "Enterprise".to_string(),
            "free" => "Free".to_string(),
            _ => p,
        });

        UsageSnapshot {
            primary,
            secondary,
            tertiary: None,
            credits,
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

    fn format_reset_time(&self, unix_ts: i64) -> Option<String> {
        let reset_date = chrono::DateTime::from_timestamp(unix_ts, 0)?;
        let now = chrono::Utc::now();
        let duration = reset_date.signed_duration_since(now);

        if duration.num_hours() < 1 {
            Some(format!("Resets in {} min", duration.num_minutes().max(1)))
        } else if duration.num_hours() < 24 {
            Some(format!("Resets in {}h", duration.num_hours()))
        } else {
            Some(format!("Resets in {} days", duration.num_days()))
        }
    }
}

#[async_trait]
impl ProviderFetcher for CodexProvider {
    fn name(&self) -> &'static str {
        "Codex"
    }

    fn description(&self) -> &'static str {
        "OpenAI Codex CLI"
    }

    async fn fetch(&self) -> Result<UsageSnapshot, anyhow::Error> {
        tracing::debug!("Fetching Codex usage");

        match self.fetch_via_oauth().await {
            Ok(mut usage) => {
                tracing::debug!("Codex OAuth fetch successful");
                usage.cost = cost_usage::load_cost_snapshot(ProviderId::Codex).await;
                Ok(usage)
            }
            Err(e) => {
                tracing::debug!("Codex OAuth fetch failed: {}", e);
                if let Ok(usage) = self.fetch_via_cookies().await {
                    tracing::debug!("Codex cookie fetch successful");
                    let mut usage = usage;
                    usage.cost = cost_usage::load_cost_snapshot(ProviderId::Codex).await;
                    return Ok(usage);
                }
                Err(anyhow::anyhow!("Not authenticated: {}", e))
            }
        }
    }
}

// ---- Response Types ----

#[derive(Debug, Deserialize)]
struct CodexAuthFile {
    tokens: Option<CodexAuthTokens>,
}

// Note: Some fields below are unused but required for serde deserialization
// Auth file uses snake_case: {"tokens": {"access_token": "...", "account_id": "..."}}
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct CodexAuthTokens {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    id_token: Option<String>,
    #[serde(default)]
    account_id: Option<String>,
}

// API response uses snake_case: {"plan_type": "...", "rate_limit": {...}, "credits": {...}}
#[derive(Debug, Deserialize)]
struct CodexUsageResponse {
    plan_type: Option<String>,
    rate_limit: Option<RateLimitDetails>,
    credits: Option<CreditDetails>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexCookieSession {
    cookie_header: String,
}

#[derive(Debug, Deserialize)]
struct RateLimitDetails {
    primary_window: Option<WindowSnapshot>,
    secondary_window: Option<WindowSnapshot>,
}

#[derive(Debug, Deserialize)]
struct WindowSnapshot {
    used_percent: Option<i32>,
    reset_at: Option<i64>,
    limit_window_seconds: Option<i32>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct CreditDetails {
    has_credits: Option<bool>,
    unlimited: Option<bool>,
    balance: Option<f64>,
}
