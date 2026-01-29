//! Gemini provider implementation
//!
//! Uses OAuth credentials from ~/.gemini/oauth_creds.json
//! Fetches quota via Google Cloud Code Private API

use super::{ProviderFetcher, ProviderIdentity, RateWindow, UsageSnapshot};
use async_trait::async_trait;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;
use tokio::process::Command;

const QUOTA_ENDPOINT: &str = "https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuota";
const LOAD_CODE_ASSIST_ENDPOINT: &str =
    "https://cloudcode-pa.googleapis.com/v1internal:loadCodeAssist";
const TOKEN_REFRESH_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
const CREDENTIALS_PATH: &str = ".gemini/oauth_creds.json";
const SETTINGS_PATH: &str = ".gemini/settings.json";

// Gemini OAuth client credentials should be provided at runtime via env vars:
// GEMINI_OAUTH_CLIENT_ID and GEMINI_OAUTH_CLIENT_SECRET.

pub struct GeminiProvider {
    client: reqwest::Client,
}

impl GeminiProvider {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self { client }
    }

    async fn fetch_usage(&self) -> Result<UsageSnapshot, anyhow::Error> {
        // Check auth type first
        let auth_type = self.get_auth_type()?;
        match auth_type.as_deref() {
            Some("api-key") => {
                return Err(anyhow::anyhow!(
                    "Gemini API key auth not supported. Use Google account (OAuth) instead."
                ));
            }
            Some("vertex-ai") => {
                return Err(anyhow::anyhow!(
                    "Gemini Vertex AI auth not supported. Use Google account (OAuth) instead."
                ));
            }
            _ => {} // oauth-personal or unknown - try OAuth
        }

        // Load credentials
        let mut creds = self.load_credentials().await?;

        // Check if token needs refresh
        if let Some(expiry) = creds.expiry_date {
            if expiry < chrono::Utc::now().timestamp() as f64 {
                tracing::debug!("Gemini token expired, attempting refresh");
                if let Some(refresh_token) = &creds.refresh_token {
                    creds = self
                        .refresh_access_token(
                            creds.access_token.as_deref(),
                            creds.expiry_date,
                            refresh_token,
                        )
                        .await?;
                } else {
                    return Err(anyhow::anyhow!(
                        "Gemini token expired and no refresh token available"
                    ));
                }
            }
        }

        let access_token = creds
            .access_token
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No Gemini access token found"))?;

        // Extract email from ID token
        let email = creds
            .id_token
            .as_ref()
            .and_then(|t| self.extract_email_from_token(t));

        // Get tier and project from loadCodeAssist
        let code_assist = self.load_code_assist_status(access_token).await;

        // Fetch quota
        let quota_response = self
            .fetch_quota(access_token, code_assist.project_id.as_deref())
            .await?;

        // Parse quotas
        let model_quotas = self.parse_quota_response(&quota_response)?;

        // Determine plan from tier
        let plan = match code_assist.tier.as_deref() {
            Some("standard-tier") => Some("Paid".to_string()),
            Some("free-tier") => Some("Free".to_string()),
            Some("legacy-tier") => Some("Legacy".to_string()),
            _ => None,
        };

        // Convert to UsageSnapshot
        Ok(self.build_usage_snapshot(model_quotas, email, plan))
    }

    fn get_auth_type(&self) -> Result<Option<String>, anyhow::Error> {
        let settings_path = self.get_settings_path()?;
        if !settings_path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&settings_path)?;
        let settings: serde_json::Value = serde_json::from_str(&content)?;

        Ok(settings
            .get("security")
            .and_then(|s| s.get("auth"))
            .and_then(|a| a.get("selectedType"))
            .and_then(|t| t.as_str())
            .map(|s| s.to_string()))
    }

    fn get_credentials_path(&self) -> Result<PathBuf, anyhow::Error> {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        Ok(home.join(CREDENTIALS_PATH))
    }

    fn get_settings_path(&self) -> Result<PathBuf, anyhow::Error> {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        Ok(home.join(SETTINGS_PATH))
    }

    async fn load_credentials(&self) -> Result<GeminiCredentials, anyhow::Error> {
        let creds_path = self.get_credentials_path()?;

        if !creds_path.exists() {
            return Err(anyhow::anyhow!(
                "Not logged in to Gemini. Run 'gemini' in Terminal to authenticate."
            ));
        }

        let content = tokio::fs::read_to_string(&creds_path).await?;
        let creds: GeminiCredentials = serde_json::from_str(&content)?;

        if creds.access_token.is_none()
            || creds
                .access_token
                .as_ref()
                .map(|t| t.is_empty())
                .unwrap_or(true)
        {
            return Err(anyhow::anyhow!(
                "No Gemini access token found. Run 'gemini' to authenticate."
            ));
        }

        Ok(creds)
    }

    async fn refresh_access_token(
        &self,
        current_access_token: Option<&str>,
        current_expiry: Option<f64>,
        refresh_token: &str,
    ) -> Result<GeminiCredentials, anyhow::Error> {
        let client_id = std::env::var("GEMINI_OAUTH_CLIENT_ID").ok();
        let client_secret = std::env::var("GEMINI_OAUTH_CLIENT_SECRET").ok();

        if client_id.is_none() || client_secret.is_none() {
            tracing::debug!(
                "Gemini OAuth client credentials not set; trying CLI refresh"
            );
            if let Some(creds) = self
                .try_refresh_via_cli(current_access_token, current_expiry)
                .await?
            {
                return Ok(creds);
            }
            return Err(anyhow::anyhow!(
                "Missing Gemini OAuth client credentials. Set GEMINI_OAUTH_CLIENT_ID and GEMINI_OAUTH_CLIENT_SECRET or run 'gemini' to re-authenticate."
            ));
        }

        let body = format!(
            "client_id={}&client_secret={}&refresh_token={}&grant_type=refresh_token",
            client_id.as_deref().unwrap_or_default(),
            client_secret.as_deref().unwrap_or_default(),
            refresh_token
        );

        let response = self
            .client
            .post(TOKEN_REFRESH_ENDPOINT)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await?;

        if !response.status().is_success() {
            tracing::warn!(
                "Gemini token refresh failed with status {}. Trying CLI fallback.",
                response.status()
            );
            if let Some(creds) = self
                .try_refresh_via_cli(current_access_token, current_expiry)
                .await?
            {
                return Ok(creds);
            }
            return Err(anyhow::anyhow!(
                "Token refresh failed. Run 'gemini' to re-authenticate."
            ));
        }

        let refresh_response: TokenRefreshResponse = response.json().await?;

        // Build updated credentials
        let new_expiry =
            chrono::Utc::now().timestamp() as f64 + refresh_response.expires_in.unwrap_or(3600.0);

        let creds = GeminiCredentials {
            access_token: Some(refresh_response.access_token.clone()),
            id_token: refresh_response.id_token,
            refresh_token: Some(refresh_token.to_string()),
            expiry_date: Some(new_expiry * 1000.0), // Store as milliseconds
        };

        // Update stored credentials
        self.update_stored_credentials(&creds).await?;

        tracing::info!("Gemini token refreshed successfully");
        Ok(creds)
    }

    async fn try_refresh_via_cli(
        &self,
        current_access_token: Option<&str>,
        current_expiry: Option<f64>,
    ) -> Result<Option<GeminiCredentials>, anyhow::Error> {
        let gemini_path = match find_gemini_cli().await {
            Some(path) => path,
            None => return Ok(None),
        };

        let timeout = Duration::from_secs(30);
        match run_cli_with_pty(&gemini_path, &["--help"], timeout).await {
            Ok(output) => {
                if !output.success {
                    tracing::warn!(
                        "Gemini CLI fallback exited with {}: {}",
                        output.exit_code,
                        output.output.trim()
                    );
                }
            }
            Err(err) => {
                tracing::warn!("Gemini CLI fallback failed: {}", err);
            }
        }

        let refreshed = match self.load_credentials().await {
            Ok(creds) => creds,
            Err(err) => {
                tracing::debug!("Gemini CLI fallback did not refresh credentials: {}", err);
                return Ok(None);
            }
        };

        if cli_refresh_did_update(current_access_token, current_expiry, &refreshed) {
            tracing::info!("Gemini credentials refreshed via CLI fallback");
            return Ok(Some(refreshed));
        }

        Ok(None)
    }

    async fn update_stored_credentials(
        &self,
        creds: &GeminiCredentials,
    ) -> Result<(), anyhow::Error> {
        let creds_path = self.get_credentials_path()?;

        // Read existing file to preserve other fields
        let existing_content = tokio::fs::read_to_string(&creds_path)
            .await
            .unwrap_or_default();
        let mut existing: serde_json::Value =
            serde_json::from_str(&existing_content).unwrap_or(serde_json::json!({}));

        // Update fields
        if let Some(token) = &creds.access_token {
            existing["access_token"] = serde_json::json!(token);
        }
        if let Some(id_token) = &creds.id_token {
            existing["id_token"] = serde_json::json!(id_token);
        }
        if let Some(expiry) = creds.expiry_date {
            existing["expiry_date"] = serde_json::json!(expiry);
        }

        let content = serde_json::to_string_pretty(&existing)?;
        tokio::fs::write(&creds_path, content).await?;

        Ok(())
    }

    async fn load_code_assist_status(&self, access_token: &str) -> CodeAssistStatus {
        let body = r#"{"metadata":{"ideType":"GEMINI_CLI","pluginType":"GEMINI"}}"#;

        let response = match self
            .client
            .post(LOAD_CODE_ASSIST_ENDPOINT)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("loadCodeAssist request failed: {}", e);
                return CodeAssistStatus::default();
            }
        };

        if !response.status().is_success() {
            tracing::warn!("loadCodeAssist HTTP error: {}", response.status());
            return CodeAssistStatus::default();
        }

        let json: serde_json::Value = match response.json().await {
            Ok(j) => j,
            Err(e) => {
                tracing::warn!("loadCodeAssist parse error: {}", e);
                return CodeAssistStatus::default();
            }
        };

        // Extract project ID
        let project_id = json
            .get("cloudaicompanionProject")
            .and_then(|p| {
                if let Some(s) = p.as_str() {
                    Some(s.to_string())
                } else if let Some(obj) = p.as_object() {
                    obj.get("id")
                        .or(obj.get("projectId"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                } else {
                    None
                }
            })
            .filter(|s| !s.is_empty());

        // Extract tier
        let tier = json
            .get("currentTier")
            .and_then(|t| t.get("id"))
            .and_then(|id| id.as_str())
            .map(|s| s.to_string());

        if let Some(ref pid) = project_id {
            tracing::debug!("Gemini project ID: {}", pid);
        }
        if let Some(ref t) = tier {
            tracing::debug!("Gemini tier: {}", t);
        }

        CodeAssistStatus { tier, project_id }
    }

    async fn fetch_quota(
        &self,
        access_token: &str,
        project_id: Option<&str>,
    ) -> Result<serde_json::Value, anyhow::Error> {
        let body = if let Some(pid) = project_id {
            format!(r#"{{"project": "{}"}}"#, pid)
        } else {
            "{}".to_string()
        };

        let response = self
            .client
            .post(QUOTA_ENDPOINT)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await?;

        if response.status().as_u16() == 401 {
            return Err(anyhow::anyhow!(
                "Gemini token expired. Run 'gemini' to re-authenticate."
            ));
        }

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Gemini quota API error: HTTP {}",
                response.status()
            ));
        }

        let json: serde_json::Value = response.json().await?;
        Ok(json)
    }

    fn parse_quota_response(
        &self,
        json: &serde_json::Value,
    ) -> Result<Vec<ModelQuota>, anyhow::Error> {
        let buckets = json
            .get("buckets")
            .and_then(|b| b.as_array())
            .ok_or_else(|| anyhow::anyhow!("No quota buckets in response"))?;

        if buckets.is_empty() {
            return Err(anyhow::anyhow!("Empty quota buckets in response"));
        }

        // Group by model, keeping lowest fraction per model
        let mut model_map: std::collections::HashMap<String, (f64, Option<String>)> =
            std::collections::HashMap::new();

        for bucket in buckets {
            let model_id = match bucket.get("modelId").and_then(|m| m.as_str()) {
                Some(m) => m.to_string(),
                None => continue,
            };

            let fraction = match bucket.get("remainingFraction").and_then(|f| f.as_f64()) {
                Some(f) => f,
                None => continue,
            };

            let reset_time = bucket
                .get("resetTime")
                .and_then(|r| r.as_str())
                .map(|s| s.to_string());

            match model_map.get(&model_id) {
                Some((existing_fraction, _)) if fraction >= *existing_fraction => {}
                _ => {
                    model_map.insert(model_id, (fraction, reset_time));
                }
            }
        }

        let mut quotas: Vec<ModelQuota> = model_map
            .into_iter()
            .map(|(model_id, (fraction, reset_time))| {
                let reset_description = reset_time.as_ref().and_then(|t| self.format_reset_time(t));
                ModelQuota {
                    model_id,
                    percent_left: fraction * 100.0,
                    reset_time,
                    reset_description,
                }
            })
            .collect();

        quotas.sort_by(|a, b| a.model_id.cmp(&b.model_id));
        Ok(quotas)
    }

    fn build_usage_snapshot(
        &self,
        quotas: Vec<ModelQuota>,
        email: Option<String>,
        plan: Option<String>,
    ) -> UsageSnapshot {
        // Split into flash and pro models
        let flash_quotas: Vec<&ModelQuota> = quotas
            .iter()
            .filter(|q| q.model_id.to_lowercase().contains("flash"))
            .collect();
        let pro_quotas: Vec<&ModelQuota> = quotas
            .iter()
            .filter(|q| q.model_id.to_lowercase().contains("pro"))
            .collect();

        // Find minimum for each tier
        let flash_min = flash_quotas.iter().min_by(|a, b| {
            a.percent_left
                .partial_cmp(&b.percent_left)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let pro_min = pro_quotas.iter().min_by(|a, b| {
            a.percent_left
                .partial_cmp(&b.percent_left)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Primary is Pro, secondary is Flash (24h windows)
        let primary = pro_min.map(|q| RateWindow {
            used_percent: 100.0 - q.percent_left,
            window_minutes: Some(1440), // 24 hours
            resets_at: q.reset_time.clone(),
            reset_description: q.reset_description.clone(),
            label: Some("Pro".to_string()),
        });

        let secondary = flash_min.map(|q| RateWindow {
            used_percent: 100.0 - q.percent_left,
            window_minutes: Some(1440),
            resets_at: q.reset_time.clone(),
            reset_description: q.reset_description.clone(),
            label: Some("Flash".to_string()),
        });

        UsageSnapshot {
            primary,
            secondary,
            tertiary: None,
            credits: None,
            cost: None,
            identity: Some(ProviderIdentity {
                email,
                name: None,
                plan,
                organization: None,
            }),
            updated_at: chrono::Utc::now().to_rfc3339(),
            error: None,
        }
    }

    fn extract_email_from_token(&self, id_token: &str) -> Option<String> {
        let parts: Vec<&str> = id_token.split('.').collect();
        if parts.len() < 2 {
            return None;
        }

        // Decode base64url payload
        let mut payload = parts[1].replace('-', "+").replace('_', "/");
        let padding = (4 - payload.len() % 4) % 4;
        payload.push_str(&"=".repeat(padding));

        let decoded =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &payload).ok()?;

        let claims: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
        claims
            .get("email")
            .and_then(|e| e.as_str())
            .map(|s| s.to_string())
    }

    fn format_reset_time(&self, iso_string: &str) -> Option<String> {
        let reset_date = chrono::DateTime::parse_from_rfc3339(iso_string).ok()?;
        let now = chrono::Utc::now();
        let duration = reset_date.signed_duration_since(now);

        if duration.num_seconds() <= 0 {
            return Some("Resets soon".to_string());
        }

        let hours = duration.num_hours();
        let minutes = duration.num_minutes() % 60;

        if hours > 0 {
            Some(format!("Resets in {}h {}m", hours, minutes))
        } else {
            Some(format!("Resets in {}m", minutes))
        }
    }
}

#[async_trait]
impl ProviderFetcher for GeminiProvider {
    fn name(&self) -> &'static str {
        "Gemini"
    }

    fn description(&self) -> &'static str {
        "Google Gemini AI"
    }

    async fn fetch(&self) -> Result<UsageSnapshot, anyhow::Error> {
        tracing::debug!("Fetching Gemini usage");
        self.fetch_usage().await
    }
}

// ---- Internal Types ----

#[derive(Debug, Default)]
struct CodeAssistStatus {
    tier: Option<String>,
    project_id: Option<String>,
}

#[derive(Debug)]
struct ModelQuota {
    model_id: String,
    percent_left: f64,
    reset_time: Option<String>,
    reset_description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct GeminiCredentials {
    access_token: Option<String>,
    id_token: Option<String>,
    refresh_token: Option<String>,
    expiry_date: Option<f64>, // milliseconds since epoch
}

#[derive(Debug, Deserialize)]
struct TokenRefreshResponse {
    access_token: String,
    expires_in: Option<f64>,
    id_token: Option<String>,
}

struct CliRunResult {
    success: bool,
    exit_code: i32,
    output: String,
}

async fn run_cli_with_pty(
    binary: &str,
    args: &[&str],
    timeout: Duration,
) -> Result<CliRunResult, anyhow::Error> {
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    let mut command = CommandBuilder::new(binary);
    command.args(args);
    command.env("TERM", "xterm-256color");

    let mut child = pair.slave.spawn_command(command)?;
    let reader = pair.master.try_clone_reader()?;

    let output_task = tokio::task::spawn_blocking(move || read_stream_blocking(reader));
    let mut killer = child.clone_killer();
    let wait_task = tokio::task::spawn_blocking(move || child.wait());

    let status = match tokio::time::timeout(timeout, wait_task).await {
        Ok(result) => result.map_err(|err| anyhow::anyhow!(err.to_string()))??,
        Err(_) => {
            let _ = killer.kill();
            return Err(anyhow::anyhow!(
                "CLI timed out after {} seconds",
                timeout.as_secs()
            ));
        }
    };

    let output_bytes = output_task.await.unwrap_or_default();
    let exit_code = status.exit_code().try_into().unwrap_or(i32::MAX);
    Ok(CliRunResult {
        success: status.success(),
        exit_code,
        output: String::from_utf8_lossy(&output_bytes).to_string(),
    })
}

fn read_stream_blocking(mut stream: impl Read) -> Vec<u8> {
    let mut buffer = Vec::new();
    let mut temp = [0u8; 1024];
    loop {
        match stream.read(&mut temp) {
            Ok(0) => break,
            Ok(n) => buffer.extend_from_slice(&temp[..n]),
            Err(_) => break,
        }
    }
    buffer
}

async fn find_gemini_cli() -> Option<String> {
    let output = Command::new("which").arg("gemini").output().await.ok()?;
    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            return Some(path);
        }
    }

    let candidates = [
        "/usr/local/bin/gemini",
        "/opt/homebrew/bin/gemini",
        &format!(
            "{}/.local/bin/gemini",
            std::env::var("HOME").unwrap_or_default()
        ),
        &format!(
            "{}/.npm-global/bin/gemini",
            std::env::var("HOME").unwrap_or_default()
        ),
    ];

    candidates
        .iter()
        .map(|path| path.to_string())
        .find(|path| std::path::Path::new(path).exists())
}

fn cli_refresh_did_update(
    current_access_token: Option<&str>,
    current_expiry: Option<f64>,
    refreshed: &GeminiCredentials,
) -> bool {
    let refreshed_token = refreshed.access_token.as_deref();
    if let (Some(current), Some(next)) = (current_access_token, refreshed_token) {
        if current != next {
            return true;
        }
    }

    if let (Some(current_expiry), Some(next_expiry)) = (current_expiry, refreshed.expiry_date) {
        if next_expiry > current_expiry {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    #[tokio::test]
    #[cfg(unix)]
    async fn cli_fallback_refreshes_credentials() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let bin_dir = temp_dir.path().join("bin");
        fs::create_dir_all(&bin_dir).expect("bin dir");

        let gemini_path = bin_dir.join("gemini");
        let script = r#"#!/bin/sh
mkdir -p "$HOME/.gemini"
cat > "$HOME/.gemini/oauth_creds.json" <<'EOF'
{"access_token":"cli-token","refresh_token":"refresh","expiry_date":1234567890}
EOF
exit 0
"#;
        fs::write(&gemini_path, script).expect("write script");
        let mut perms = fs::metadata(&gemini_path).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&gemini_path, perms).expect("chmod");

        let previous_home = std::env::var("HOME").ok();
        let previous_path = std::env::var("PATH").ok();
        std::env::set_var("HOME", temp_dir.path());
        let new_path = match previous_path.as_ref() {
            Some(path) => format!("{}:{}", bin_dir.display(), path),
            None => bin_dir.display().to_string(),
        };
        std::env::set_var("PATH", new_path);

        let provider = GeminiProvider::new();
        let result = provider
            .try_refresh_via_cli(Some("old-token"), Some(1.0))
            .await
            .expect("cli fallback");

        if let Some(home) = previous_home {
            std::env::set_var("HOME", home);
        }
        if let Some(path) = previous_path {
            std::env::set_var("PATH", path);
        }

        let refreshed = result.expect("credentials");
        assert_eq!(refreshed.access_token.as_deref(), Some("cli-token"));
    }

    #[test]
    fn exit_code_saturates_on_overflow() {
        let exit_code: u64 = i64::from(i32::MAX) as u64 + 1;
        assert_eq!(exit_code.try_into().unwrap_or(i32::MAX), i32::MAX);
    }
}
