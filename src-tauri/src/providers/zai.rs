//! z.ai provider implementation
//!
//! Uses API token authentication via Z_AI_API_KEY environment variable
//! or stored in keychain/settings.

use super::{ProviderFetcher, ProviderIdentity, RateWindow, UsageSnapshot};
use async_trait::async_trait;
use serde::Deserialize;
use crate::storage::keyring::KeyringError;
use crate::storage::SecureStorage;

/// API regions for z.ai
#[derive(Debug, Clone, Copy, Default)]
pub enum ZaiRegion {
    #[default]
    Global,
    #[allow(dead_code)]
    BigModelCN,
}

impl ZaiRegion {
    pub fn base_url(&self) -> &'static str {
        match self {
            ZaiRegion::Global => "https://api.z.ai",
            ZaiRegion::BigModelCN => "https://open.bigmodel.cn",
        }
    }

    pub fn quota_url(&self) -> String {
        format!("{}/api/monitor/usage/quota/limit", self.base_url())
    }
}

pub struct ZaiProvider {
    client: reqwest::Client,
    region: ZaiRegion,
}

const KEYCHAIN_TOKEN_KEY: &str = "zai";

impl ZaiProvider {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self {
            client,
            region: ZaiRegion::default(),
        }
    }

    #[allow(dead_code)]
    pub fn with_region(mut self, region: ZaiRegion) -> Self {
        self.region = region;
        self
    }

    /// Resolve API token from environment or settings
    fn resolve_api_token(&self) -> Option<String> {
        self.resolve_api_token_with(|| Self::load_keychain_token())
    }

    fn resolve_api_token_with<F>(&self, load_keychain: F) -> Option<String>
    where
        F: FnOnce() -> Option<String>,
    {
        // Check environment variable first
        if let Ok(token) = std::env::var("Z_AI_API_KEY") {
            let cleaned = Self::clean_token(&token);
            if !cleaned.is_empty() {
                return Some(cleaned);
            }
        }

        let keychain_token = load_keychain()?;
        let cleaned = Self::clean_token(&keychain_token);
        if cleaned.is_empty() {
            None
        } else {
            Some(cleaned)
        }
    }

    fn load_keychain_token() -> Option<String> {
        let storage = SecureStorage::new();
        match storage.get(KEYCHAIN_TOKEN_KEY) {
            Ok(value) => Some(value),
            Err(KeyringError::NotFound) => None,
            Err(err) => {
                tracing::debug!("Failed to read z.ai token from keychain: {}", err);
                None
            }
        }
    }

    /// Clean token value (remove quotes, whitespace)
    fn clean_token(token: &str) -> String {
        let mut value = token.trim().to_string();

        // Remove surrounding quotes
        if (value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\''))
        {
            value = value[1..value.len() - 1].to_string();
        }

        value.trim().to_string()
    }

    /// Resolve the quota URL (with environment overrides)
    fn resolve_quota_url(&self) -> String {
        // Check for full URL override
        if let Ok(url) = std::env::var("Z_AI_QUOTA_URL") {
            let cleaned = url.trim();
            if !cleaned.is_empty() {
                return cleaned.to_string();
            }
        }

        // Check for host override
        if let Ok(host) = std::env::var("Z_AI_API_HOST") {
            let cleaned = host.trim().trim_end_matches('/');
            if !cleaned.is_empty() {
                let base = if cleaned.starts_with("http://") || cleaned.starts_with("https://") {
                    cleaned.to_string()
                } else {
                    format!("https://{}", cleaned)
                };
                return format!("{}/api/monitor/usage/quota/limit", base);
            }
        }

        // Use region default
        self.region.quota_url()
    }

    async fn fetch_usage(&self) -> Result<UsageSnapshot, anyhow::Error> {
        let api_key = self.resolve_api_token().ok_or_else(|| {
            anyhow::anyhow!(
                "z.ai API token not found. Set Z_AI_API_KEY or store a keychain token."
            )
        })?;

        let url = self.resolve_quota_url();
        tracing::debug!("Fetching z.ai usage from: {}", url);

        let response = self
            .client
            .get(&url)
            .header("authorization", format!("Bearer {}", api_key))
            .header("accept", "application/json")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "z.ai API error: HTTP {} - {}",
                status,
                body
            ));
        }

        let api_response: ZaiQuotaResponse = response.json().await?;

        if !api_response.success || api_response.code != 200 {
            return Err(anyhow::anyhow!("z.ai API error: {}", api_response.msg));
        }

        let data = api_response
            .data
            .ok_or_else(|| anyhow::anyhow!("z.ai API returned no data"))?;

        Ok(self.convert_response(data))
    }

    fn convert_response(&self, data: ZaiQuotaData) -> UsageSnapshot {
        // Find token and time limits
        let token_limit = data.limits.iter().find(|l| l.limit_type == "TOKENS_LIMIT");
        let time_limit = data.limits.iter().find(|l| l.limit_type == "TIME_LIMIT");

        // Primary is token limit, secondary is time limit
        let primary_limit = token_limit.or(time_limit);
        let secondary_limit = if token_limit.is_some() && time_limit.is_some() {
            time_limit
        } else {
            None
        };

        let primary = primary_limit.map(|l| self.rate_window_from_limit(l));
        let secondary = secondary_limit.map(|l| self.rate_window_from_limit(l));

        let plan = data
            .plan_name
            .or(data.plan)
            .or(data.plan_type)
            .or(data.package_name);

        UsageSnapshot {
            primary,
            secondary,
            tertiary: None,
            credits: None,
            cost: None,
            identity: plan.map(|p| ProviderIdentity {
                email: None,
                name: None,
                plan: Some(p),
                organization: None,
            }),
            updated_at: chrono::Utc::now().to_rfc3339(),
            error: None,
        }
    }

    fn rate_window_from_limit(&self, limit: &ZaiLimitRaw) -> RateWindow {
        let used_percent = self.compute_used_percent(limit);
        let window_minutes = self.compute_window_minutes(limit);
        let reset_time = limit.next_reset_time.map(|ts| {
            chrono::DateTime::from_timestamp_millis(ts)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_default()
        });
        let reset_description = self.window_label(limit);

        RateWindow {
            used_percent,
            window_minutes,
            resets_at: reset_time,
            reset_description,
            label: if limit.limit_type == "TOKENS_LIMIT" {
                Some("Tokens".to_string())
            } else {
                Some("Time".to_string())
            },
        }
    }

    fn compute_used_percent(&self, limit: &ZaiLimitRaw) -> f64 {
        // If we have usage data, compute from remaining/usage
        if limit.usage > 0 {
            let total = limit.usage.max(0);
            if total > 0 {
                let used_from_remaining = total - limit.remaining;
                let used = limit
                    .current_value
                    .max(used_from_remaining)
                    .min(total)
                    .max(0);
                let percent = (used as f64 / total as f64) * 100.0;
                return percent.min(100.0).max(0.0);
            }
        }

        // Fall back to API-provided percentage
        limit.percentage as f64
    }

    fn compute_window_minutes(&self, limit: &ZaiLimitRaw) -> Option<i32> {
        if limit.number <= 0 {
            return None;
        }

        match limit.unit {
            5 => Some(limit.number),           // minutes
            3 => Some(limit.number * 60),      // hours
            1 => Some(limit.number * 24 * 60), // days
            _ => None,
        }
    }

    fn window_label(&self, limit: &ZaiLimitRaw) -> Option<String> {
        if limit.number <= 0 {
            if limit.limit_type == "TIME_LIMIT" {
                return Some("Monthly".to_string());
            }
            return None;
        }

        let unit_label = match limit.unit {
            5 => "minute",
            3 => "hour",
            1 => "day",
            _ => return None,
        };

        let suffix = if limit.number == 1 {
            unit_label
        } else {
            &format!("{}s", unit_label)
        };
        Some(format!("{} {} window", limit.number, suffix))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_env_var<F>(key: &str, value: Option<&str>, f: F)
    where
        F: FnOnce(),
    {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let previous = env::var(key).ok();

        match value {
            Some(value) => env::set_var(key, value),
            None => env::remove_var(key),
        }

        f();

        match previous {
            Some(value) => env::set_var(key, value),
            None => env::remove_var(key),
        }
    }

    #[test]
    fn resolve_api_token_prefers_env_var() {
        with_env_var("Z_AI_API_KEY", Some("  'env-token'  "), || {
            let provider = ZaiProvider::new();
            let token = provider.resolve_api_token_with(|| Some("stored-token".to_string()));
            assert_eq!(token.as_deref(), Some("env-token"));
        });
    }

    #[test]
    fn resolve_api_token_falls_back_to_keychain() {
        with_env_var("Z_AI_API_KEY", Some("   "), || {
            let provider = ZaiProvider::new();
            let token = provider.resolve_api_token_with(|| Some("\"stored\"".to_string()));
            assert_eq!(token.as_deref(), Some("stored"));
        });
    }

    #[test]
    fn resolve_api_token_returns_none_when_missing() {
        with_env_var("Z_AI_API_KEY", None, || {
            let provider = ZaiProvider::new();
            let token = provider.resolve_api_token_with(|| None);
            assert!(token.is_none());
        });
    }
}

#[async_trait]
impl ProviderFetcher for ZaiProvider {
    fn name(&self) -> &'static str {
        "z.ai"
    }

    fn description(&self) -> &'static str {
        "z.ai AI Assistant"
    }

    async fn fetch(&self) -> Result<UsageSnapshot, anyhow::Error> {
        tracing::debug!("Fetching z.ai usage");
        self.fetch_usage().await
    }
}

// ---- API Response Types ----

#[derive(Debug, Deserialize)]
struct ZaiQuotaResponse {
    code: i32,
    msg: String,
    data: Option<ZaiQuotaData>,
    success: bool,
}

#[derive(Debug, Deserialize)]
struct ZaiQuotaData {
    #[serde(default)]
    limits: Vec<ZaiLimitRaw>,
    #[serde(rename = "planName")]
    plan_name: Option<String>,
    plan: Option<String>,
    #[serde(rename = "plan_type")]
    plan_type: Option<String>,
    #[serde(rename = "packageName")]
    package_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ZaiLimitRaw {
    #[serde(rename = "type")]
    limit_type: String,
    #[serde(default)]
    unit: i32,
    #[serde(default)]
    number: i32,
    #[serde(default)]
    usage: i32,
    #[serde(rename = "currentValue", default)]
    current_value: i32,
    #[serde(default)]
    remaining: i32,
    #[serde(default)]
    percentage: i32,
    #[serde(rename = "usageDetails", default)]
    _usage_details: Vec<ZaiUsageDetail>,
    #[serde(rename = "nextResetTime")]
    next_reset_time: Option<i64>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct ZaiUsageDetail {
    #[serde(rename = "modelCode")]
    model_code: String,
    usage: i32,
}
