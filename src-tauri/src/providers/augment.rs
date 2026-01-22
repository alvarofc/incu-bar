//! Augment provider implementation
//!
//! Uses cookie-based authentication via browser cookie import.
//! Endpoints:
//! - /api/credits - usage credits
//! - /api/subscription - subscription details

use async_trait::async_trait;
use serde::Deserialize;
use super::{Credits, ProviderFetcher, ProviderIdentity, RateWindow, UsageSnapshot};

const BASE_URL: &str = "https://app.augmentcode.com";
const CREDITS_URL: &str = "https://app.augmentcode.com/api/credits";
const SUBSCRIPTION_URL: &str = "https://app.augmentcode.com/api/subscription";

const SESSION_ENDPOINTS: &[&str] = &[
    "https://app.augmentcode.com/api/auth/session",
    "https://app.augmentcode.com/api/session",
    "https://app.augmentcode.com/api/user",
];

pub struct AugmentProvider {
    client: reqwest::Client,
}

impl AugmentProvider {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self { client }
    }

    async fn fetch_with_cookies(&self, cookie_header: &str) -> Result<UsageSnapshot, AugmentError> {
        if let Err(err) = self.keepalive_session(cookie_header).await {
            match err {
                AugmentError::SessionExpired | AugmentError::NotLoggedIn => return Err(err),
                _ => tracing::debug!("Augment keepalive failed: {}", err),
            }
        }

        let credits = self.fetch_credits(cookie_header).await?;
        let subscription = match self.fetch_subscription(cookie_header).await {
            Ok(subscription) => Some(subscription),
            Err(AugmentError::SessionExpired) => return Err(AugmentError::SessionExpired),
            Err(AugmentError::NotLoggedIn) => return Err(AugmentError::NotLoggedIn),
            Err(err) => {
                tracing::debug!("Augment subscription fetch failed: {}", err);
                None
            }
        };

        Ok(self.build_snapshot(&credits, subscription.as_ref()))
    }

    async fn fetch_credits(&self, cookie_header: &str) -> Result<AugmentCreditsResponse, AugmentError> {
        let response = self.client
            .get(CREDITS_URL)
            .header("Cookie", cookie_header)
            .header("Accept", "application/json")
            .header("User-Agent", "IncuBar/1.0")
            .send()
            .await
            .map_err(|e| AugmentError::Api(e.to_string()))?;

        self.ensure_success("credits", &response).await?;

        response
            .json::<AugmentCreditsResponse>()
            .await
            .map_err(|e| AugmentError::Parse(format!("Credits response: {}", e)))
    }

    async fn fetch_subscription(
        &self,
        cookie_header: &str,
    ) -> Result<AugmentSubscriptionResponse, AugmentError> {
        let response = self.client
            .get(SUBSCRIPTION_URL)
            .header("Cookie", cookie_header)
            .header("Accept", "application/json")
            .header("User-Agent", "IncuBar/1.0")
            .send()
            .await
            .map_err(|e| AugmentError::Api(e.to_string()))?;

        self.ensure_success("subscription", &response).await?;

        response
            .json::<AugmentSubscriptionResponse>()
            .await
            .map_err(|e| AugmentError::Parse(format!("Subscription response: {}", e)))
    }

    async fn ensure_success(
        &self,
        label: &str,
        response: &reqwest::Response,
    ) -> Result<(), AugmentError> {
        match response.status().as_u16() {
            200 => Ok(()),
            401 => Err(AugmentError::SessionExpired),
            403 => Err(AugmentError::NotLoggedIn),
            status => Err(AugmentError::Api(format!("{} HTTP {}", label, status))),
        }
    }

    fn build_snapshot(
        &self,
        credits: &AugmentCreditsResponse,
        subscription: Option<&AugmentSubscriptionResponse>,
    ) -> UsageSnapshot {
        let remaining = credits.usage_units_remaining.or(credits.usage_units_available);
        let consumed = credits.usage_units_consumed_this_billing_cycle.unwrap_or(0.0).max(0.0);
        let total = remaining.map(|value| (value + consumed).max(0.0));

        let used_percent = total
            .filter(|value| *value > 0.0)
            .map(|value| (consumed / value) * 100.0)
            .unwrap_or(0.0)
            .clamp(0.0, 100.0);

        let resets_at = subscription
            .and_then(|sub| sub.billing_period_end.as_ref())
            .and_then(|date| self.normalize_reset_time(date));

        let primary = Some(RateWindow {
            used_percent,
            window_minutes: None,
            resets_at: resets_at.clone(),
            reset_description: resets_at.as_ref().and_then(|date| self.format_reset_time(date)),
            label: Some("Credits".to_string()),
        });

        let credits_snapshot = remaining.map(|value| Credits {
            remaining: value,
            total,
            unit: "credits".to_string(),
        });

        UsageSnapshot {
            primary,
            secondary: None,
            tertiary: None,
            credits: credits_snapshot,
            cost: None,
            identity: Some(ProviderIdentity {
                email: subscription.and_then(|sub| sub.email.clone()),
                name: None,
                plan: subscription.and_then(|sub| sub.plan_name.clone()),
                organization: subscription.and_then(|sub| sub.organization.clone()),
            }),
            updated_at: chrono::Utc::now().to_rfc3339(),
            error: None,
        }
    }

    fn normalize_reset_time(&self, date: &str) -> Option<String> {
        chrono::DateTime::parse_from_rfc3339(date)
            .map(|dt| dt.to_rfc3339())
            .ok()
            .or_else(|| {
                chrono::NaiveDateTime::parse_from_str(date, "%Y-%m-%dT%H:%M:%SZ")
                    .ok()
                    .map(|dt| dt.and_utc().to_rfc3339())
            })
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

    async fn keepalive_session(&self, cookie_header: &str) -> Result<(), AugmentError> {
        let mut saw_unauthorized = false;

        for endpoint in SESSION_ENDPOINTS {
            let response = self.client
                .get(*endpoint)
                .header("Cookie", cookie_header)
                .header("Accept", "application/json")
                .header("User-Agent", "IncuBar/1.0")
                .header("Origin", BASE_URL)
                .header("Referer", BASE_URL)
                .send()
                .await
                .map_err(|e| AugmentError::Api(e.to_string()))?;

            match response.status().as_u16() {
                200 => return Ok(()),
                401 => saw_unauthorized = true,
                403 => return Err(AugmentError::NotLoggedIn),
                404 => continue,
                _ => continue,
            }
        }

        if saw_unauthorized {
            Err(AugmentError::SessionExpired)
        } else {
            Err(AugmentError::Api("All session endpoints failed".to_string()))
        }
    }

    async fn load_stored_cookies(&self) -> Result<String, anyhow::Error> {
        let session_path = self.get_session_path()?;
        if session_path.exists() {
            let content = tokio::fs::read_to_string(&session_path).await?;
            let session: AugmentSession = serde_json::from_str(&content)?;
            return Ok(session.cookie_header);
        }
        Err(anyhow::anyhow!("No stored Augment session found"))
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

    async fn clear_session(&self) {
        if let Ok(path) = self.get_session_path() {
            let _ = tokio::fs::remove_file(path).await;
        }
    }

    fn get_session_path(&self) -> Result<std::path::PathBuf, anyhow::Error> {
        let data_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
        Ok(data_dir.join("IncuBar").join("augment-session.json"))
    }
}

#[async_trait]
impl ProviderFetcher for AugmentProvider {
    fn name(&self) -> &'static str {
        "Augment"
    }

    fn description(&self) -> &'static str {
        "Augment AI"
    }

    async fn fetch(&self) -> Result<UsageSnapshot, anyhow::Error> {
        tracing::debug!("Fetching Augment usage");

        if let Ok(cookies) = self.load_stored_cookies().await {
            match self.fetch_with_cookies(&cookies).await {
                Ok(usage) => return Ok(usage),
                Err(err) => {
                    tracing::debug!("Augment fetch with stored cookies failed: {}", err);
                    if matches!(err, AugmentError::SessionExpired | AugmentError::NotLoggedIn) {
                        self.clear_session().await;
                    }
                }
            }
        }

        match crate::browser_cookies::import_augment_cookies_from_browser().await {
            Ok(result) => {
                if let Err(err) = self.store_session(&result.cookie_header).await {
                    tracing::debug!("Failed to store Augment session: {}", err);
                }
                self.fetch_with_cookies(&result.cookie_header)
                    .await
                    .map_err(|err| anyhow::anyhow!(err.to_string()))
            }
            Err(err) => Err(anyhow::anyhow!("Not authenticated: {}", err)),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AugmentCreditsResponse {
    usage_units_remaining: Option<f64>,
    usage_units_consumed_this_billing_cycle: Option<f64>,
    usage_units_available: Option<f64>,
    usage_balance_status: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AugmentSubscriptionResponse {
    plan_name: Option<String>,
    billing_period_end: Option<String>,
    email: Option<String>,
    organization: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AugmentSession {
    cookie_header: String,
}

#[derive(thiserror::Error, Debug)]
enum AugmentError {
    #[error("Augment session expired")]
    SessionExpired,
    #[error("Not logged in to Augment")]
    NotLoggedIn,
    #[error("Augment API error: {0}")]
    Api(String),
    #[error("Failed to parse Augment response: {0}")]
    Parse(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_snapshot_with_credits_and_subscription() {
        let provider = AugmentProvider::new();
        let credits = AugmentCreditsResponse {
            usage_units_remaining: Some(120.0),
            usage_units_consumed_this_billing_cycle: Some(30.0),
            usage_units_available: None,
            usage_balance_status: Some("active".to_string()),
        };
        let subscription = AugmentSubscriptionResponse {
            plan_name: Some("Pro".to_string()),
            billing_period_end: Some("2030-01-02T00:00:00Z".to_string()),
            email: Some("user@example.com".to_string()),
            organization: Some("Acme".to_string()),
        };

        let snapshot = provider.build_snapshot(&credits, Some(&subscription));
        let primary = snapshot.primary.expect("primary");
        let credits_snapshot = snapshot.credits.expect("credits");

        assert!((primary.used_percent - 20.0).abs() < 0.01);
        assert_eq!(primary.resets_at, Some("2030-01-02T00:00:00+00:00".to_string()));
        assert_eq!(credits_snapshot.remaining, 120.0);
        assert_eq!(credits_snapshot.total, Some(150.0));
        assert_eq!(credits_snapshot.unit, "credits");
        let identity = snapshot.identity.as_ref().expect("identity");
        assert_eq!(identity.plan.as_deref(), Some("Pro"));
        assert_eq!(identity.email.as_deref(), Some("user@example.com"));
        assert_eq!(identity.organization.as_deref(), Some("Acme"));
    }

    #[test]
    fn builds_snapshot_with_available_credits_only() {
        let provider = AugmentProvider::new();
        let credits = AugmentCreditsResponse {
            usage_units_remaining: None,
            usage_units_consumed_this_billing_cycle: Some(10.0),
            usage_units_available: Some(40.0),
            usage_balance_status: None,
        };

        let snapshot = provider.build_snapshot(&credits, None);
        let primary = snapshot.primary.expect("primary");
        let credits_snapshot = snapshot.credits.expect("credits");

        assert!((primary.used_percent - 20.0).abs() < 0.01);
        assert_eq!(credits_snapshot.remaining, 40.0);
        assert_eq!(credits_snapshot.total, Some(50.0));
    }
}
