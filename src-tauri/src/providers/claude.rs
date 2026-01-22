//! Claude provider implementation
//! 
//! Supports three authentication methods in priority order:
//! 1. OAuth API (api.anthropic.com) - requires credentials from ~/.claude/.credentials.json
//! 2. Web API (claude.ai) - requires session cookie from browser
//! 3. CLI fallback (not yet implemented)

use async_trait::async_trait;
use serde::Deserialize;
use std::path::PathBuf;
use super::{ProviderFetcher, UsageSnapshot, RateWindow, ProviderIdentity};

const OAUTH_BASE_URL: &str = "https://api.anthropic.com";
const OAUTH_USAGE_PATH: &str = "/api/oauth/usage";
const OAUTH_BETA_HEADER: &str = "oauth-2025-04-20";

#[allow(dead_code)] // Reserved for future web API implementation
const WEB_BASE_URL: &str = "https://claude.ai/api";

pub struct ClaudeProvider {
    client: reqwest::Client,
}

impl ClaudeProvider {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();
        
        Self { client }
    }

    /// Try to fetch via OAuth API first
    async fn fetch_via_oauth(&self) -> Result<UsageSnapshot, anyhow::Error> {
        let creds = self.load_oauth_credentials().await?;
        
        let response = self.client
            .get(format!("{}{}", OAUTH_BASE_URL, OAUTH_USAGE_PATH))
            .header("Authorization", format!("Bearer {}", creds.access_token))
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .header("anthropic-beta", OAUTH_BETA_HEADER)
            .header("User-Agent", "IncuBar/1.0")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("OAuth API returned status: {}", response.status()));
        }

        let usage_response: OAuthUsageResponse = response.json().await?;
        Ok(self.convert_oauth_response(usage_response, creds.rate_limit_tier))
    }

    /// Load OAuth credentials from Claude's credential file
    async fn load_oauth_credentials(&self) -> Result<ClaudeOAuthCredentials, anyhow::Error> {
        let creds_path = self.get_credentials_path()?;
        
        if !creds_path.exists() {
            return Err(anyhow::anyhow!("Claude credentials file not found at {:?}", creds_path));
        }

        let content = tokio::fs::read_to_string(&creds_path).await?;
        let file: CredentialsFile = serde_json::from_str(&content)?;
        
        let oauth = file.claude_ai_oauth.ok_or_else(|| {
            anyhow::anyhow!("No claudeAiOauth section in credentials file")
        })?;

        // Validate scopes
        if !oauth.scopes.contains(&"user:profile".to_string()) {
            return Err(anyhow::anyhow!("OAuth token missing 'user:profile' scope"));
        }

        Ok(oauth)
    }

    /// Get the path to Claude's credentials file
    fn get_credentials_path(&self) -> Result<PathBuf, anyhow::Error> {
        let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        Ok(home.join(".claude").join(".credentials.json"))
    }

    /// Convert OAuth response to UsageSnapshot
    fn convert_oauth_response(&self, response: OAuthUsageResponse, tier: Option<String>) -> UsageSnapshot {
        let primary = response.five_hour.map(|w| RateWindow {
            used_percent: w.utilization.unwrap_or(0.0),
            window_minutes: Some(300), // 5 hours
            resets_at: w.resets_at.clone(),
            reset_description: w.resets_at.as_ref().and_then(|r| self.format_reset_time(r)),
            label: Some("Session".to_string()),
        });

        let secondary = response.seven_day.map(|w| RateWindow {
            used_percent: w.utilization.unwrap_or(0.0),
            window_minutes: Some(10080), // 7 days
            resets_at: w.resets_at.clone(),
            reset_description: w.resets_at.as_ref().and_then(|r| self.format_reset_time(r)),
            label: Some("Weekly".to_string()),
        });

        let tertiary = response.seven_day_opus.or(response.seven_day_sonnet).map(|w| RateWindow {
            used_percent: w.utilization.unwrap_or(0.0),
            window_minutes: Some(10080),
            resets_at: w.resets_at.clone(),
            reset_description: w.resets_at.as_ref().and_then(|r| self.format_reset_time(r)),
            label: Some("Weekly (Model)".to_string()),
        });

        let plan = tier.map(|t| {
            // Convert tier codes to display names
            if t.contains("max") { "Max".to_string() }
            else if t.contains("pro") { "Pro".to_string() }
            else if t.contains("team") { "Team".to_string() }
            else { t }
        });

        UsageSnapshot {
            primary,
            secondary,
            tertiary,
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

    /// Format a reset time into a human-readable description
    fn format_reset_time(&self, iso_time: &str) -> Option<String> {
        let reset_date = chrono::DateTime::parse_from_rfc3339(iso_time).ok()?;
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
impl ProviderFetcher for ClaudeProvider {
    fn name(&self) -> &'static str {
        "Claude"
    }

    fn description(&self) -> &'static str {
        "Anthropic Claude AI Assistant"
    }

    async fn fetch(&self) -> Result<UsageSnapshot, anyhow::Error> {
        tracing::debug!("Fetching Claude usage");

        // Try OAuth first
        match self.fetch_via_oauth().await {
            Ok(usage) => {
                tracing::debug!("Claude OAuth fetch successful");
                Ok(usage)
            }
            Err(e) => {
                tracing::debug!("Claude OAuth fetch failed: {}", e);
                // Return error snapshot - no mock data
                Err(anyhow::anyhow!("Not authenticated: {}", e))
            }
        }
    }
}

// ---- OAuth Response Types ----

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CredentialsFile {
    claude_ai_oauth: Option<ClaudeOAuthCredentials>,
}

// Note: Some fields below are unused but required for serde deserialization
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClaudeOAuthCredentials {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_at: Option<i64>,
    #[serde(default)]
    scopes: Vec<String>,
    #[serde(default)]
    rate_limit_tier: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct OAuthUsageResponse {
    five_hour: Option<OAuthUsageWindow>,
    seven_day: Option<OAuthUsageWindow>,
    #[serde(default)]
    seven_day_oauth_apps: Option<OAuthUsageWindow>,
    #[serde(default)]
    seven_day_opus: Option<OAuthUsageWindow>,
    #[serde(default)]
    seven_day_sonnet: Option<OAuthUsageWindow>,
    #[serde(default)]
    iguana_necktie: Option<OAuthUsageWindow>,
    #[serde(default)]
    extra_usage: Option<OAuthExtraUsage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct OAuthUsageWindow {
    utilization: Option<f64>,
    resets_at: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct OAuthExtraUsage {
    is_enabled: Option<bool>,
    monthly_limit: Option<f64>,
    used_credits: Option<f64>,
    utilization: Option<f64>,
    currency: Option<String>,
}
