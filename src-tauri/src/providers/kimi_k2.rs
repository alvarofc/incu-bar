//! Kimi K2 provider implementation
//!
//! Uses API key authentication via KIMI_K2_API_KEY, KIMI_API_KEY, or KIMI_KEY
//! environment variables.

use async_trait::async_trait;
use super::{ProviderFetcher, UsageSnapshot, RateWindow, ProviderIdentity};

const CREDITS_URL: &str = "https://kimi-k2.ai/api/user/credits";

pub struct KimiK2Provider {
    client: reqwest::Client,
}

impl KimiK2Provider {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self { client }
    }

    /// Resolve API key from environment variables
    fn resolve_api_key(&self) -> Option<String> {
        const API_KEY_VARS: &[&str] = &["KIMI_K2_API_KEY", "KIMI_API_KEY", "KIMI_KEY"];

        for var in API_KEY_VARS {
            if let Ok(key) = std::env::var(var) {
                let cleaned = Self::clean_token(&key);
                if !cleaned.is_empty() {
                    return Some(cleaned);
                }
            }
        }

        // TODO: Check keychain/settings storage
        None
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

    async fn fetch_usage(&self) -> Result<UsageSnapshot, anyhow::Error> {
        let api_key = self.resolve_api_key()
            .ok_or_else(|| anyhow::anyhow!("Kimi K2 API key not found. Set KIMI_K2_API_KEY environment variable."))?;

        tracing::debug!("Fetching Kimi K2 usage from: {}", CREDITS_URL);

        let response = self.client
            .get(CREDITS_URL)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Accept", "application/json")
            .send()
            .await?;

        // Check for remaining credits in headers
        let remaining_from_header = response
            .headers()
            .get("x-credits-remaining")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<f64>().ok());

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Kimi K2 API error: HTTP {} - {}", status, body));
        }

        let body = response.text().await?;
        tracing::debug!("Kimi K2 API response: {}", body);

        let json: serde_json::Value = serde_json::from_str(&body)?;
        
        Ok(self.parse_response(&json, remaining_from_header))
    }

    fn parse_response(&self, json: &serde_json::Value, remaining_from_header: Option<f64>) -> UsageSnapshot {
        // Build list of contexts to search in
        let contexts = self.build_contexts(json);
        
        // Search for consumed credits
        let consumed = self.find_double(&contexts, &[
            &["total_credits_consumed"],
            &["totalCreditsConsumed"],
            &["total_credits_used"],
            &["totalCreditsUsed"],
            &["credits_consumed"],
            &["creditsConsumed"],
            &["consumedCredits"],
            &["usedCredits"],
            &["total"],
            &["usage", "total"],
            &["usage", "consumed"],
        ]).unwrap_or(0.0);

        // Search for remaining credits
        let remaining = self.find_double(&contexts, &[
            &["credits_remaining"],
            &["creditsRemaining"],
            &["remaining_credits"],
            &["remainingCredits"],
            &["available_credits"],
            &["availableCredits"],
            &["credits_left"],
            &["creditsLeft"],
            &["usage", "credits_remaining"],
            &["usage", "remaining"],
        ]).or(remaining_from_header).unwrap_or(0.0);

        let total = (consumed + remaining).max(0.0);
        let used_percent = if total > 0.0 {
            ((consumed / total) * 100.0).min(100.0).max(0.0)
        } else {
            0.0
        };

        let reset_description = if total > 0.0 {
            Some(format!("Credits: {:.0}/{:.0}", consumed, total))
        } else {
            None
        };

        UsageSnapshot {
            primary: Some(RateWindow {
                used_percent,
                window_minutes: None,
                resets_at: None,
                reset_description,
                label: Some("Credits".to_string()),
            }),
            secondary: None,
            tertiary: None,
            credits: None,
            cost: None,
            identity: Some(ProviderIdentity {
                email: None,
                name: None,
                plan: None,
                organization: None,
            }),
            updated_at: chrono::Utc::now().to_rfc3339(),
            error: None,
        }
    }

    fn build_contexts<'a>(&self, json: &'a serde_json::Value) -> Vec<&'a serde_json::Value> {
        let mut contexts = vec![json];

        // Try nested structures
        if let Some(data) = json.get("data") {
            contexts.push(data);
            if let Some(usage) = data.get("usage") {
                contexts.push(usage);
            }
            if let Some(credits) = data.get("credits") {
                contexts.push(credits);
            }
        }
        if let Some(result) = json.get("result") {
            contexts.push(result);
            if let Some(usage) = result.get("usage") {
                contexts.push(usage);
            }
            if let Some(credits) = result.get("credits") {
                contexts.push(credits);
            }
        }
        if let Some(usage) = json.get("usage") {
            contexts.push(usage);
        }
        if let Some(credits) = json.get("credits") {
            contexts.push(credits);
        }

        contexts
    }

    fn find_double(&self, contexts: &[&serde_json::Value], paths: &[&[&str]]) -> Option<f64> {
        for path in paths {
            for context in contexts {
                if let Some(value) = self.get_at_path(context, path) {
                    if let Some(num) = self.to_double(value) {
                        return Some(num);
                    }
                }
            }
        }
        None
    }

    fn get_at_path<'a>(&self, value: &'a serde_json::Value, path: &[&str]) -> Option<&'a serde_json::Value> {
        let mut current = value;
        for key in path {
            current = current.get(*key)?;
        }
        Some(current)
    }

    fn to_double(&self, value: &serde_json::Value) -> Option<f64> {
        match value {
            serde_json::Value::Number(n) => n.as_f64(),
            serde_json::Value::String(s) => s.parse().ok(),
            _ => None,
        }
    }
}

#[async_trait]
impl ProviderFetcher for KimiK2Provider {
    fn name(&self) -> &'static str {
        "Kimi K2"
    }

    fn description(&self) -> &'static str {
        "Kimi K2 AI Assistant"
    }

    async fn fetch(&self) -> Result<UsageSnapshot, anyhow::Error> {
        tracing::debug!("Fetching Kimi K2 usage");
        self.fetch_usage().await
    }
}
