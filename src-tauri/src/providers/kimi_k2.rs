//! Kimi K2 provider implementation
//!
//! Uses API key authentication via KIMI_K2_API_KEY, KIMI_API_KEY, or KIMI_KEY
//! environment variables.

use super::{ProviderFetcher, ProviderIdentity, RateWindow, UsageSnapshot};
use async_trait::async_trait;
use crate::storage::keyring::KeyringError;
use crate::storage::SecureStorage;

const CREDITS_URL: &str = "https://kimi-k2.ai/api/user/credits";

pub struct KimiK2Provider {
    client: reqwest::Client,
}

const KEYCHAIN_TOKEN_KEY: &str = "kimi_k2";

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
        self.resolve_api_key_with(|| Self::load_keychain_token())
    }

    fn resolve_api_key_with<F>(&self, load_keychain: F) -> Option<String>
    where
        F: FnOnce() -> Option<String>,
    {
        const API_KEY_VARS: &[&str] = &["KIMI_K2_API_KEY", "KIMI_API_KEY", "KIMI_KEY"];

        for var in API_KEY_VARS {
            if let Ok(key) = std::env::var(var) {
                let cleaned = Self::clean_token(&key);
                if !cleaned.is_empty() {
                    return Some(cleaned);
                }
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
                tracing::debug!("Failed to read Kimi K2 token from keychain: {}", err);
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

    async fn fetch_usage(&self) -> Result<UsageSnapshot, anyhow::Error> {
        let api_key = self.resolve_api_key().ok_or_else(|| {
            anyhow::anyhow!(
                "Kimi K2 API key not found. Set KIMI_K2_API_KEY or store a keychain token."
            )
        })?;

        tracing::debug!("Fetching Kimi K2 usage from: {}", CREDITS_URL);

        let response = self
            .client
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
            return Err(anyhow::anyhow!(
                "Kimi K2 API error: HTTP {} - {}",
                status,
                body
            ));
        }

        let body = response.text().await?;
        tracing::debug!("Kimi K2 API response: {}", body);

        let json: serde_json::Value = serde_json::from_str(&body)?;

        Ok(self.parse_response(&json, remaining_from_header))
    }

    fn parse_response(
        &self,
        json: &serde_json::Value,
        remaining_from_header: Option<f64>,
    ) -> UsageSnapshot {
        // Build list of contexts to search in
        let contexts = self.build_contexts(json);

        // Search for consumed credits
        let consumed = self
            .find_double(
                &contexts,
                &[
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
                ],
            )
            .unwrap_or(0.0);

        // Search for remaining credits
        let remaining = self
            .find_double(
                &contexts,
                &[
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
                ],
            )
            .or(remaining_from_header)
            .unwrap_or(0.0);

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

    fn get_at_path<'a>(
        &self,
        value: &'a serde_json::Value,
        path: &[&str],
    ) -> Option<&'a serde_json::Value> {
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
    fn resolve_api_key_prefers_env_var() {
        with_env_var("KIMI_K2_API_KEY", Some("  'env-token'  "), || {
            let provider = KimiK2Provider::new();
            let token = provider.resolve_api_key_with(|| Some("stored-token".to_string()));
            assert_eq!(token.as_deref(), Some("env-token"));
        });
    }

    #[test]
    fn resolve_api_key_falls_back_to_keychain() {
        with_env_var("KIMI_K2_API_KEY", Some("   "), || {
            let provider = KimiK2Provider::new();
            let token = provider.resolve_api_key_with(|| Some("\"stored\"".to_string()));
            assert_eq!(token.as_deref(), Some("stored"));
        });
    }

    #[test]
    fn resolve_api_key_returns_none_when_missing() {
        with_env_var("KIMI_K2_API_KEY", None, || {
            let provider = KimiK2Provider::new();
            let token = provider.resolve_api_key_with(|| None);
            assert!(token.is_none());
        });
    }
}
