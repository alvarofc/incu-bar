//! Synthetic provider implementation
//!
//! Uses API key authentication via SYNTHETIC_API_KEY environment variable.

use super::{ProviderFetcher, ProviderIdentity, RateWindow, UsageSnapshot};
use async_trait::async_trait;

const QUOTA_API_URL: &str = "https://api.synthetic.new/v2/quotas";

pub struct SyntheticProvider {
    client: reqwest::Client,
}

impl SyntheticProvider {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self { client }
    }

    /// Resolve API key from environment
    fn resolve_api_key(&self) -> Option<String> {
        if let Ok(key) = std::env::var("SYNTHETIC_API_KEY") {
            let cleaned = Self::clean_token(&key);
            if !cleaned.is_empty() {
                return Some(cleaned);
            }
        }
        None
    }

    /// Clean token value (remove quotes, whitespace)
    fn clean_token(token: &str) -> String {
        let mut value = token.trim().to_string();

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
                "Synthetic API key not found. Set SYNTHETIC_API_KEY environment variable."
            )
        })?;

        tracing::debug!("Fetching Synthetic usage from: {}", QUOTA_API_URL);

        let response = self
            .client
            .get(QUOTA_API_URL)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Accept", "application/json")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            if status.as_u16() == 401 || status.as_u16() == 403 {
                return Err(anyhow::anyhow!("Invalid Synthetic API credentials"));
            }
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Synthetic API error: HTTP {} - {}",
                status,
                body
            ));
        }

        let body = response.text().await?;
        tracing::debug!("Synthetic API response: {}", body);

        let json: serde_json::Value = serde_json::from_str(&body)?;

        self.parse_response(&json)
    }

    fn parse_response(&self, json: &serde_json::Value) -> Result<UsageSnapshot, anyhow::Error> {
        let plan_name = self.find_plan_name(json);
        let quota_objects = self.find_quota_objects(json);

        if quota_objects.is_empty() {
            return Err(anyhow::anyhow!("Missing quota data in Synthetic response"));
        }

        let quotas: Vec<QuotaEntry> = quota_objects
            .iter()
            .filter_map(|obj| self.parse_quota(obj))
            .collect();

        if quotas.is_empty() {
            return Err(anyhow::anyhow!("Could not parse any quota entries"));
        }

        let primary = quotas.first().map(|q| self.quota_to_rate_window(q));
        let secondary = quotas.get(1).map(|q| self.quota_to_rate_window(q));

        Ok(UsageSnapshot {
            primary,
            secondary,
            tertiary: None,
            credits: None,
            cost: None,
            identity: plan_name.map(|p| ProviderIdentity {
                email: None,
                name: None,
                plan: Some(p),
                organization: None,
            }),
            updated_at: chrono::Utc::now().to_rfc3339(),
            error: None,
        })
    }

    fn find_plan_name(&self, json: &serde_json::Value) -> Option<String> {
        const PLAN_KEYS: &[&str] = &[
            "plan",
            "planName",
            "plan_name",
            "subscription",
            "subscriptionPlan",
            "tier",
            "package",
            "packageName",
        ];

        // Check root level
        for key in PLAN_KEYS {
            if let Some(s) = json.get(*key).and_then(|v| v.as_str()) {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }

        // Check in data object
        if let Some(data) = json.get("data") {
            for key in PLAN_KEYS {
                if let Some(s) = data.get(*key).and_then(|v| v.as_str()) {
                    let trimmed = s.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
        }

        None
    }

    fn find_quota_objects<'a>(&self, json: &'a serde_json::Value) -> Vec<&'a serde_json::Value> {
        const QUOTA_KEYS: &[&str] = &[
            "quotas",
            "quota",
            "limits",
            "usage",
            "entries",
            "subscription",
            "data",
        ];

        // Try root level first
        for key in QUOTA_KEYS {
            if let Some(arr) = json.get(*key).and_then(|v| v.as_array()) {
                return arr.iter().collect();
            }
            if let Some(obj) = json.get(*key) {
                if obj.is_object() && self.is_quota_payload(obj) {
                    return vec![obj];
                }
            }
        }

        // Try in data object
        if let Some(data) = json.get("data") {
            for key in QUOTA_KEYS {
                if let Some(arr) = data.get(*key).and_then(|v| v.as_array()) {
                    return arr.iter().collect();
                }
                if let Some(obj) = data.get(*key) {
                    if obj.is_object() && self.is_quota_payload(obj) {
                        return vec![obj];
                    }
                }
            }
        }

        // If root is an array, use it directly
        if let Some(arr) = json.as_array() {
            return arr.iter().collect();
        }

        // If root itself is a quota payload
        if self.is_quota_payload(json) {
            return vec![json];
        }

        vec![]
    }

    fn is_quota_payload(&self, json: &serde_json::Value) -> bool {
        const CHECK_KEYS: &[&[&str]] = &[
            &["limit", "quota", "max", "total", "capacity", "allowance"],
            &[
                "used",
                "usage",
                "requests",
                "requestCount",
                "request_count",
                "consumed",
                "spent",
            ],
            &["remaining", "left", "available", "balance"],
            &[
                "percentUsed",
                "usedPercent",
                "usagePercent",
                "usage_percent",
                "used_percent",
                "percent_used",
                "percent",
            ],
            &[
                "percentRemaining",
                "remainingPercent",
                "remaining_percent",
                "percent_remaining",
            ],
        ];

        for keys in CHECK_KEYS {
            for key in *keys {
                if json.get(*key).and_then(|v| self.to_double(v)).is_some() {
                    return true;
                }
            }
        }
        false
    }

    fn parse_quota(&self, json: &serde_json::Value) -> Option<QuotaEntry> {
        let label = self.find_string(
            json,
            &["name", "label", "type", "period", "scope", "title", "id"],
        );

        // Try to find percent used directly
        let percent_used = self
            .find_double(
                json,
                &[
                    "percentUsed",
                    "usedPercent",
                    "usagePercent",
                    "usage_percent",
                    "used_percent",
                    "percent_used",
                    "percent",
                ],
            )
            .map(|v| self.normalize_percent(v));

        let percent_remaining = self
            .find_double(
                json,
                &[
                    "percentRemaining",
                    "remainingPercent",
                    "remaining_percent",
                    "percent_remaining",
                ],
            )
            .map(|v| self.normalize_percent(v));

        let mut used_percent = percent_used;
        if used_percent.is_none() {
            if let Some(remaining) = percent_remaining {
                used_percent = Some(100.0 - remaining);
            }
        }

        // If no percent found, calculate from limit/used/remaining
        if used_percent.is_none() {
            let limit = self.find_double(
                json,
                &["limit", "quota", "max", "total", "capacity", "allowance"],
            );
            let used = self.find_double(
                json,
                &[
                    "used",
                    "usage",
                    "requests",
                    "requestCount",
                    "request_count",
                    "consumed",
                    "spent",
                ],
            );
            let remaining = self.find_double(json, &["remaining", "left", "available", "balance"]);

            let final_limit = limit.or_else(|| match (used, remaining) {
                (Some(u), Some(r)) => Some(u + r),
                _ => None,
            });

            let final_used = used.or_else(|| match (final_limit, remaining) {
                (Some(l), Some(r)) => Some(l - r),
                _ => None,
            });

            if let (Some(l), Some(u)) = (final_limit, final_used) {
                if l > 0.0 {
                    used_percent = Some((u / l) * 100.0);
                }
            }
        }

        let used_percent = used_percent?.clamp(0.0, 100.0);

        let window_minutes = self.find_window_minutes(json);
        let resets_at = self.find_date(
            json,
            &[
                "resetAt",
                "reset_at",
                "resetsAt",
                "resets_at",
                "renewAt",
                "renew_at",
                "renewsAt",
                "renews_at",
                "periodEnd",
                "period_end",
                "expiresAt",
                "expires_at",
                "endAt",
                "end_at",
            ],
        );

        let reset_description = if resets_at.is_none() {
            self.window_description(window_minutes)
        } else {
            None
        };

        Some(QuotaEntry {
            label,
            used_percent,
            window_minutes,
            resets_at,
            reset_description,
        })
    }

    fn find_string(&self, json: &serde_json::Value, keys: &[&str]) -> Option<String> {
        for key in keys {
            if let Some(s) = json.get(*key).and_then(|v| v.as_str()) {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
        None
    }

    fn find_double(&self, json: &serde_json::Value, keys: &[&str]) -> Option<f64> {
        for key in keys {
            if let Some(v) = json.get(*key) {
                if let Some(n) = self.to_double(v) {
                    return Some(n);
                }
            }
        }
        None
    }

    fn find_window_minutes(&self, json: &serde_json::Value) -> Option<i32> {
        if let Some(m) = self.find_double(
            json,
            &[
                "windowMinutes",
                "window_minutes",
                "periodMinutes",
                "period_minutes",
            ],
        ) {
            return Some(m as i32);
        }
        if let Some(h) = self.find_double(
            json,
            &["windowHours", "window_hours", "periodHours", "period_hours"],
        ) {
            return Some((h * 60.0) as i32);
        }
        if let Some(d) = self.find_double(
            json,
            &["windowDays", "window_days", "periodDays", "period_days"],
        ) {
            return Some((d * 24.0 * 60.0) as i32);
        }
        if let Some(s) = self.find_double(
            json,
            &[
                "windowSeconds",
                "window_seconds",
                "periodSeconds",
                "period_seconds",
            ],
        ) {
            return Some((s / 60.0) as i32);
        }
        None
    }

    fn find_date(&self, json: &serde_json::Value, keys: &[&str]) -> Option<String> {
        for key in keys {
            if let Some(v) = json.get(*key) {
                if let Some(ts) = self.to_double(v) {
                    // Convert timestamp to ISO string
                    let secs = if ts > 1_000_000_000_000.0 {
                        ts / 1000.0
                    } else {
                        ts
                    };
                    if let Some(dt) = chrono::DateTime::from_timestamp(secs as i64, 0) {
                        return Some(dt.to_rfc3339());
                    }
                }
                if let Some(s) = v.as_str() {
                    // Try to parse as timestamp
                    if let Ok(ts) = s.trim().parse::<f64>() {
                        let secs = if ts > 1_000_000_000_000.0 {
                            ts / 1000.0
                        } else {
                            ts
                        };
                        if let Some(dt) = chrono::DateTime::from_timestamp(secs as i64, 0) {
                            return Some(dt.to_rfc3339());
                        }
                    }
                    // Assume it's already an ISO string
                    if chrono::DateTime::parse_from_rfc3339(s).is_ok() {
                        return Some(s.to_string());
                    }
                }
            }
        }
        None
    }

    fn to_double(&self, value: &serde_json::Value) -> Option<f64> {
        match value {
            serde_json::Value::Number(n) => n.as_f64(),
            serde_json::Value::String(s) => s.trim().parse().ok(),
            _ => None,
        }
    }

    fn normalize_percent(&self, value: f64) -> f64 {
        if value <= 1.0 {
            value * 100.0
        } else {
            value
        }
    }

    fn window_description(&self, minutes: Option<i32>) -> Option<String> {
        let minutes = minutes?;
        if minutes <= 0 {
            return None;
        }

        let day_minutes = 24 * 60;
        if minutes % day_minutes == 0 {
            let days = minutes / day_minutes;
            let suffix = if days == 1 { "" } else { "s" };
            return Some(format!("{} day{} window", days, suffix));
        }
        if minutes % 60 == 0 {
            let hours = minutes / 60;
            let suffix = if hours == 1 { "" } else { "s" };
            return Some(format!("{} hour{} window", hours, suffix));
        }
        let suffix = if minutes == 1 { "" } else { "s" };
        Some(format!("{} minute{} window", minutes, suffix))
    }

    fn quota_to_rate_window(&self, quota: &QuotaEntry) -> RateWindow {
        RateWindow {
            used_percent: quota.used_percent,
            window_minutes: quota.window_minutes,
            resets_at: quota.resets_at.clone(),
            reset_description: quota.reset_description.clone(),
            label: quota.label.clone(),
        }
    }
}

struct QuotaEntry {
    label: Option<String>,
    used_percent: f64,
    window_minutes: Option<i32>,
    resets_at: Option<String>,
    reset_description: Option<String>,
}

#[async_trait]
impl ProviderFetcher for SyntheticProvider {
    fn name(&self) -> &'static str {
        "Synthetic"
    }

    fn description(&self) -> &'static str {
        "Synthetic AI Platform"
    }

    async fn fetch(&self) -> Result<UsageSnapshot, anyhow::Error> {
        tracing::debug!("Fetching Synthetic usage");
        self.fetch_usage().await
    }
}
