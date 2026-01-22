//!
//! Factory (Droid) provider implementation
//!
//! Uses cookie-based authentication via browser cookie import or stored session.
//! Endpoint: https://app.factory.ai/api/usage

use async_trait::async_trait;
use serde_json::Value;
use super::{ProviderFetcher, UsageSnapshot, RateWindow, ProviderIdentity};

const USAGE_URL: &str = "https://app.factory.ai/api/usage";

pub struct FactoryProvider {
    client: reqwest::Client,
}

impl FactoryProvider {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self { client }
    }

    async fn fetch_with_cookies(&self, cookie_header: &str) -> Result<UsageSnapshot, anyhow::Error> {
        let response = self.client
            .get(USAGE_URL)
            .header("Cookie", cookie_header)
            .header("Accept", "application/json")
            .header("User-Agent", "IncuBar/1.0")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Factory API returned status: {}",
                response.status()
            ));
        }

        let raw_json = response.text().await?;
        tracing::debug!("Factory usage response: {}", &raw_json);

        let json: Value = serde_json::from_str(&raw_json)?;
        self.parse_usage_response(&json)
    }

    fn parse_usage_response(&self, json: &Value) -> Result<UsageSnapshot, anyhow::Error> {
        let base = json.get("data").unwrap_or(json);
        let reset_fallback = self.find_reset_time(base);
        let window_minutes = self.find_window_minutes(base);

        let quotas = self.extract_quota_entries(base, reset_fallback.clone(), window_minutes);
        if quotas.is_empty() {
            return Err(anyhow::anyhow!("Factory usage response missing quota data"));
        }

        let primary = quotas.get(0).map(|q| q.to_rate_window());
        let secondary = quotas.get(1).map(|q| q.to_rate_window());

        let plan = self.find_string(base, &[
            "plan", "planName", "plan_name", "tier", "subscription", "package",
        ]);

        let email = self.find_string(base, &["email", "userEmail", "user_email"]);

        Ok(UsageSnapshot {
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
        })
    }

    fn extract_quota_entries(
        &self,
        base: &Value,
        reset_fallback: Option<String>,
        window_minutes: Option<i32>,
    ) -> Vec<QuotaEntry> {
        let mut entries = Vec::new();

        if let Some(array) = base.get("usage").and_then(|v| v.as_array()) {
            for value in array {
                if let Some(entry) = self.parse_quota_entry(None, value, reset_fallback.clone(), window_minutes) {
                    entries.push(entry);
                }
            }
        }

        if let Some(obj) = base.get("usage") {
            if obj.is_object() {
                if let Some(entry) = self.parse_quota_entry(Some("Usage".to_string()), obj, reset_fallback.clone(), window_minutes) {
                    entries.push(entry);
                }
            }
        }

        if let Some(array) = base.get("quotas").and_then(|v| v.as_array()) {
            for value in array {
                if let Some(entry) = self.parse_quota_entry(None, value, reset_fallback.clone(), window_minutes) {
                    entries.push(entry);
                }
            }
        }

        if let Some(obj) = base.as_object() {
            for (key, value) in obj.iter() {
                if value.is_object() {
                    if let Some(entry) = self.parse_quota_entry(Some(key.clone()), value, reset_fallback.clone(), window_minutes) {
                        entries.push(entry);
                    }
                }
            }
        }

        entries
    }

    fn parse_quota_entry(
        &self,
        label: Option<String>,
        value: &Value,
        reset_fallback: Option<String>,
        window_minutes: Option<i32>,
    ) -> Option<QuotaEntry> {
        let used = self.find_double(value, &[
            "used", "usage", "usedCount", "consumed", "spent", "requestsUsed",
            "tokensUsed", "usageUsed", "used_total", "usedTotal",
        ])?;
        let limit = self.find_double(value, &[
            "limit", "quota", "max", "total", "capacity", "allowance",
            "requestsLimit", "tokensLimit", "usageLimit", "limit_total", "limitTotal",
        ])?;

        let used_percent = if limit > 0.0 {
            (used / limit) * 100.0
        } else {
            0.0
        };

        let resets_at = self.find_reset_time(value).or(reset_fallback.clone());
        let reset_description = resets_at
            .as_ref()
            .and_then(|t| self.format_reset_time(t));

        Some(QuotaEntry {
            label,
            used_percent: used_percent.clamp(0.0, 100.0),
            window_minutes,
            resets_at,
            reset_description,
        })
    }

    fn find_string(&self, json: &Value, keys: &[&str]) -> Option<String> {
        for key in keys {
            if let Some(value) = json.get(*key) {
                if let Some(s) = value.as_str() {
                    let trimmed = s.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
        }
        None
    }

    fn find_double(&self, json: &Value, keys: &[&str]) -> Option<f64> {
        for key in keys {
            if let Some(value) = json.get(*key) {
                if let Some(number) = self.to_double(value) {
                    return Some(number);
                }
            }
        }
        None
    }

    fn find_window_minutes(&self, json: &Value) -> Option<i32> {
        if let Some(minutes) = self.find_double(json, &["windowMinutes", "window_minutes", "periodMinutes", "period_minutes"]) {
            return Some(minutes as i32);
        }
        if let Some(hours) = self.find_double(json, &["windowHours", "window_hours", "periodHours", "period_hours"]) {
            return Some((hours * 60.0) as i32);
        }
        if let Some(days) = self.find_double(json, &["windowDays", "window_days", "periodDays", "period_days"]) {
            return Some((days * 24.0 * 60.0) as i32);
        }
        if let Some(seconds) = self.find_double(json, &["windowSeconds", "window_seconds", "periodSeconds", "period_seconds"]) {
            return Some((seconds / 60.0) as i32);
        }
        None
    }

    fn find_reset_time(&self, json: &Value) -> Option<String> {
        let timestamp_keys = [
            "resetAt", "reset_at", "resetsAt", "resets_at", "resetTime", "reset_time",
            "nextReset", "next_reset", "renewAt", "renew_at", "periodEnd", "period_end",
        ];

        for key in &timestamp_keys {
            if let Some(value) = json.get(*key) {
                if let Some(ts) = self.to_double(value) {
                    return self.timestamp_to_iso(ts);
                }
                if let Some(s) = value.as_str() {
                    if let Ok(ts) = s.trim().parse::<f64>() {
                        return self.timestamp_to_iso(ts);
                    }
                    if chrono::DateTime::parse_from_rfc3339(s).is_ok() {
                        return Some(s.to_string());
                    }
                }
            }
        }

        let duration_keys = [
            "resetInSeconds", "reset_in_seconds", "resetSeconds", "reset_seconds",
            "resetIn", "reset_in",
        ];

        for key in &duration_keys {
            if let Some(seconds) = json.get(*key).and_then(|v| self.to_double(v)) {
                if seconds > 0.0 {
                    let reset_at = chrono::Utc::now() + chrono::Duration::seconds(seconds as i64);
                    return Some(reset_at.to_rfc3339());
                }
            }
        }

        None
    }

    fn timestamp_to_iso(&self, timestamp: f64) -> Option<String> {
        let seconds = if timestamp > 1_000_000_000_000.0 {
            (timestamp / 1000.0) as i64
        } else {
            timestamp as i64
        };
        chrono::DateTime::from_timestamp(seconds, 0).map(|dt| dt.to_rfc3339())
    }

    fn to_double(&self, value: &Value) -> Option<f64> {
        match value {
            Value::Number(n) => n.as_f64(),
            Value::String(s) => s.trim().parse().ok(),
            _ => None,
        }
    }

    fn format_reset_time(&self, iso_time: &str) -> Option<String> {
        let reset_date = chrono::DateTime::parse_from_rfc3339(iso_time).ok()?;
        let now = chrono::Utc::now();
        let duration = reset_date.signed_duration_since(now);

        if duration.num_seconds() <= 0 {
            return Some("Resets soon".to_string());
        }

        if duration.num_hours() < 1 {
            Some(format!("Resets in {} min", duration.num_minutes().max(1)))
        } else if duration.num_hours() < 24 {
            Some(format!("Resets in {}h", duration.num_hours()))
        } else {
            Some(format!("Resets in {} days", duration.num_days()))
        }
    }

    async fn load_stored_cookies(&self) -> Result<String, anyhow::Error> {
        let session_path = self.get_session_path()?;
        if session_path.exists() {
            let content = tokio::fs::read_to_string(&session_path).await?;
            let session: FactorySession = serde_json::from_str(&content)?;
            return Ok(session.cookie_header);
        }
        Err(anyhow::anyhow!("No stored Factory session found"))
    }

    async fn store_session(&self, cookie_header: &str) -> Result<(), anyhow::Error> {
        let session_path = self.get_session_path()?;
        if let Some(parent) = session_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let content = serde_json::json!({
            "cookieHeader": cookie_header,
            "savedAt": chrono::Utc::now().to_rfc3339(),
        });
        tokio::fs::write(&session_path, serde_json::to_string_pretty(&content)?).await?;
        Ok(())
    }

    fn get_session_path(&self) -> Result<std::path::PathBuf, anyhow::Error> {
        let data_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
        Ok(data_dir.join("IncuBar").join("factory-session.json"))
    }
}

#[async_trait]
impl ProviderFetcher for FactoryProvider {
    fn name(&self) -> &'static str {
        "Droid"
    }

    fn description(&self) -> &'static str {
        "Factory AI Droid"
    }

    async fn fetch(&self) -> Result<UsageSnapshot, anyhow::Error> {
        tracing::debug!("Fetching Factory usage");

        if let Ok(cookies) = self.load_stored_cookies().await {
            if let Ok(usage) = self.fetch_with_cookies(&cookies).await {
                return Ok(usage);
            }
        }

        match crate::browser_cookies::import_factory_cookies_from_browser().await {
            Ok(result) => {
                if let Err(e) = self.store_session(&result.cookie_header).await {
                    tracing::debug!("Failed to store Factory session: {}", e);
                }
                self.fetch_with_cookies(&result.cookie_header).await
            }
            Err(e) => Err(anyhow::anyhow!("Not authenticated: {}", e)),
        }
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct FactorySession {
    cookie_header: String,
}

#[derive(Debug)]
struct QuotaEntry {
    label: Option<String>,
    used_percent: f64,
    window_minutes: Option<i32>,
    resets_at: Option<String>,
    reset_description: Option<String>,
}

impl QuotaEntry {
    fn to_rate_window(&self) -> RateWindow {
        RateWindow {
            used_percent: self.used_percent,
            window_minutes: self.window_minutes,
            resets_at: self.resets_at.clone(),
            reset_description: self.reset_description.clone(),
            label: self.label.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_usage_with_reset() {
        let provider = FactoryProvider::new();
        let json = json!({
            "data": {
                "plan": "Standard",
                "resetAt": "2030-01-02T00:00:00Z",
                "standard": { "used": 50, "limit": 200 },
                "premium": { "used": 10, "limit": 40 }
            }
        });

        let snapshot = provider.parse_usage_response(&json).expect("snapshot");
        let primary = snapshot.primary.expect("primary");
        let secondary = snapshot.secondary.expect("secondary");

        assert_eq!(primary.used_percent, 25.0);
        assert_eq!(secondary.used_percent, 25.0);
        assert!(primary.resets_at.is_some());
        assert!(primary.reset_description.is_some());
        assert_eq!(snapshot.identity.and_then(|i| i.plan), Some("Standard".to_string()));
    }

    #[test]
    fn parses_usage_with_relative_reset() {
        let provider = FactoryProvider::new();
        let json = json!({
            "usage": { "used": 12, "limit": 24, "resetInSeconds": 3600 },
            "planName": "Premium"
        });

        let snapshot = provider.parse_usage_response(&json).expect("snapshot");
        let primary = snapshot.primary.expect("primary");

        assert_eq!(primary.used_percent, 50.0);
        assert!(primary.resets_at.is_some());
        assert!(primary.reset_description.is_some());
        assert_eq!(snapshot.identity.and_then(|i| i.plan), Some("Premium".to_string()));
    }
}
