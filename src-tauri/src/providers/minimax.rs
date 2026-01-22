//!
//! MiniMax provider implementation
//!
//! Uses cookie-based authentication via browser cookie import.
//! Endpoint: https://platform.minimax.io/platform/api/subscription/coding_plan/remains

use async_trait::async_trait;
use serde::Deserialize;
use super::{Credits, ProviderFetcher, ProviderIdentity, RateWindow, UsageSnapshot};

const USAGE_URL: &str = "https://platform.minimax.io/platform/api/subscription/coding_plan/remains";

pub struct MinimaxProvider {
    client: reqwest::Client,
}

impl MinimaxProvider {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self { client }
    }

    async fn fetch_with_cookies(&self, cookie_header: &str) -> Result<MinimaxUsageResponse, MinimaxError> {
        let response = self.client
            .get(USAGE_URL)
            .header("Cookie", cookie_header)
            .header("Accept", "application/json")
            .header("User-Agent", "IncuBar/1.0")
            .send()
            .await
            .map_err(|e| MinimaxError::Api(e.to_string()))?;

        self.ensure_success(&response).await?;

        response
            .json::<MinimaxUsageResponse>()
            .await
            .map_err(|e| MinimaxError::Parse(format!("Usage response: {}", e)))
    }

    async fn ensure_success(&self, response: &reqwest::Response) -> Result<(), MinimaxError> {
        match response.status().as_u16() {
            200 => Ok(()),
            401 => Err(MinimaxError::SessionExpired),
            403 => Err(MinimaxError::NotLoggedIn),
            status => Err(MinimaxError::Api(format!("HTTP {}", status))),
        }
    }

    fn build_snapshot(&self, response: &MinimaxUsageResponse) -> Result<UsageSnapshot, MinimaxError> {
        let data = response.data.as_ref().unwrap_or(&response.flat);
        if data.remaining_credits.is_none() && data.total_credits.is_none() {
            return Err(MinimaxError::MissingData);
        }

        let remaining = data.remaining_credits.unwrap_or(0.0).max(0.0);
        let total = data.total_credits.unwrap_or(0.0).max(0.0);
        let consumed = (total - remaining).max(0.0);

        let used_percent = if total > 0.0 {
            ((consumed / total) * 100.0).min(100.0).max(0.0)
        } else {
            0.0
        };

        let primary = Some(RateWindow {
            used_percent,
            window_minutes: None,
            resets_at: None,
            reset_description: if total > 0.0 {
                Some(format!("Credits: {:.0}/{:.0}", consumed, total))
            } else {
                None
            },
            label: Some("Credits".to_string()),
        });

        let credits = if total > 0.0 || remaining > 0.0 {
            Some(Credits {
                remaining,
                total: if total > 0.0 { Some(total) } else { None },
                unit: "credits".to_string(),
            })
        } else {
            None
        };

        Ok(UsageSnapshot {
            primary,
            secondary: None,
            tertiary: None,
            credits,
            cost: None,
            identity: Some(ProviderIdentity {
                email: None,
                name: None,
                plan: None,
                organization: None,
            }),
            updated_at: chrono::Utc::now().to_rfc3339(),
            error: None,
        })
    }

    async fn load_stored_cookies(&self) -> Result<String, anyhow::Error> {
        let session_path = self.get_session_path()?;
        if session_path.exists() {
            let content = tokio::fs::read_to_string(&session_path).await?;
            let session: MinimaxSession = serde_json::from_str(&content)?;
            return Ok(session.cookie_header);
        }
        Err(anyhow::anyhow!("No stored MiniMax session found"))
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
        Ok(data_dir.join("IncuBar").join("minimax-session.json"))
    }
}

#[async_trait]
impl ProviderFetcher for MinimaxProvider {
    fn name(&self) -> &'static str {
        "MiniMax"
    }

    fn description(&self) -> &'static str {
        "MiniMax"
    }

    async fn fetch(&self) -> Result<UsageSnapshot, anyhow::Error> {
        tracing::debug!("Fetching MiniMax usage");

        if let Ok(cookies) = self.load_stored_cookies().await {
            match self.fetch_with_cookies(&cookies).await {
                Ok(usage) => return Ok(self.build_snapshot(&usage)?),
                Err(err) => {
                    tracing::debug!("MiniMax fetch with stored cookies failed: {}", err);
                    if matches!(err, MinimaxError::SessionExpired | MinimaxError::NotLoggedIn) {
                        self.clear_session().await;
                    }
                }
            }
        }

        match crate::browser_cookies::import_minimax_cookies_from_browser().await {
            Ok(result) => {
                if let Err(err) = self.store_session(&result.cookie_header).await {
                    tracing::debug!("Failed to store MiniMax session: {}", err);
                }
                let response = self.fetch_with_cookies(&result.cookie_header).await?;
                self.build_snapshot(&response).map_err(|err| anyhow::anyhow!(err.to_string()))
            }
            Err(err) => Err(anyhow::anyhow!("Not authenticated: {}", err)),
        }
    }
}

impl MinimaxProvider {
    async fn clear_session(&self) {
        if let Ok(path) = self.get_session_path() {
            let _ = tokio::fs::remove_file(path).await;
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MinimaxUsageResponse {
    #[serde(default)]
    data: Option<MinimaxUsageData>,
    #[serde(flatten)]
    flat: MinimaxUsageData,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
struct MinimaxUsageData {
    remaining_credits: Option<f64>,
    total_credits: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MinimaxSession {
    cookie_header: String,
}

#[derive(thiserror::Error, Debug)]
enum MinimaxError {
    #[error("MiniMax session expired")]
    SessionExpired,
    #[error("Not logged in to MiniMax")]
    NotLoggedIn,
    #[error("MiniMax API error: {0}")]
    Api(String),
    #[error("Failed to parse MiniMax response: {0}")]
    Parse(String),
    #[error("MiniMax response missing data")]
    MissingData,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_snapshot_with_credits() {
        let provider = MinimaxProvider::new();
        let response = MinimaxUsageResponse {
            data: Some(MinimaxUsageData {
                remaining_credits: Some(120.0),
                total_credits: Some(200.0),
            }),
            flat: MinimaxUsageData::default(),
        };

        let snapshot = provider.build_snapshot(&response).expect("snapshot");
        let primary = snapshot.primary.expect("primary");
        let credits = snapshot.credits.expect("credits");

        assert!((primary.used_percent - 40.0).abs() < 0.01);
        assert_eq!(primary.label.as_deref(), Some("Credits"));
        assert_eq!(credits.remaining, 120.0);
        assert_eq!(credits.total, Some(200.0));
        assert_eq!(credits.unit, "credits");
    }

    #[test]
    fn builds_snapshot_with_zero_total() {
        let provider = MinimaxProvider::new();
        let response = MinimaxUsageResponse {
            data: Some(MinimaxUsageData {
                remaining_credits: Some(0.0),
                total_credits: Some(0.0),
            }),
            flat: MinimaxUsageData::default(),
        };

        let snapshot = provider.build_snapshot(&response).expect("snapshot");
        let primary = snapshot.primary.expect("primary");

        assert_eq!(primary.used_percent, 0.0);
        assert!(snapshot.credits.is_none());
    }
}
