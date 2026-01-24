//! Cursor provider implementation
//!
//! Uses cookie-based authentication via browser cookie import.
//! Endpoints:
//! - /api/usage-summary - Token-based usage
//! - /api/auth/me - User info

use super::{Credits, ProviderFetcher, ProviderIdentity, RateWindow, UsageSnapshot};
use crate::debug_settings;
use async_trait::async_trait;
use serde::Deserialize;

const USAGE_SUMMARY_URL: &str = "https://cursor.com/api/usage-summary";
const AUTH_ME_URL: &str = "https://cursor.com/api/auth/me";

pub struct CursorProvider {
    client: reqwest::Client,
}

impl CursorProvider {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self { client }
    }

    /// Fetch usage with a cookie header
    async fn fetch_with_cookies(
        &self,
        cookie_header: &str,
    ) -> Result<UsageSnapshot, anyhow::Error> {
        // Fetch usage summary
        let usage_response = self
            .client
            .get(USAGE_SUMMARY_URL)
            .header("Cookie", cookie_header)
            .header("Accept", "application/json")
            .header("User-Agent", "IncuBar/1.0")
            .send()
            .await?;

        if !usage_response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Cursor API returned status: {}",
                usage_response.status()
            ));
        }

        // Get the raw JSON for debugging
        let raw_json = usage_response.text().await?;
        tracing::debug!(
            "Cursor usage-summary response: {}",
            debug_settings::redact_value(&raw_json)
        );

        let usage: CursorUsageSummary = serde_json::from_str(&raw_json)?;
        tracing::debug!("Parsed membership_type: {:?}", usage.membership_type);

        // Fetch user info
        let user_info = self.fetch_user_info(cookie_header).await.ok();
        if let Some(ref info) = user_info {
            tracing::debug!(
                "User info: email={}, name={}",
                debug_settings::redact_option(info.email.as_deref()),
                debug_settings::redact_option(info.name.as_deref())
            );
        }

        Ok(self.convert_response(usage, user_info))
    }

    /// Fetch user info
    async fn fetch_user_info(&self, cookie_header: &str) -> Result<CursorUserInfo, anyhow::Error> {
        let response = self
            .client
            .get(AUTH_ME_URL)
            .header("Cookie", cookie_header)
            .header("Accept", "application/json")
            .header("User-Agent", "IncuBar/1.0")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Cursor auth API returned status: {}",
                response.status()
            ));
        }

        Ok(response.json().await?)
    }

    /// Convert API response to UsageSnapshot
    /// This matches the parsing logic from the original CodexBar Swift implementation
    fn convert_response(
        &self,
        usage: CursorUsageSummary,
        user_info: Option<CursorUserInfo>,
    ) -> UsageSnapshot {
        // Parse billing cycle end date
        let billing_cycle_end = usage
            .billing_cycle_end
            .as_ref()
            .and_then(|d| self.parse_iso_date(d));

        // Calculate plan usage from individual usage
        let (primary, credits) = if let Some(individual) = &usage.individual_usage {
            if let Some(plan) = &individual.plan {
                // Get raw values in cents
                let used_cents = plan.used.unwrap_or(0) as f64;

                // Use plan.limit as the total allowance
                // Note: breakdown.total is NOT the limit - it's a breakdown of usage categories
                let limit_cents = plan.limit.unwrap_or(0) as f64;

                // Calculate percentage from raw values (more accurate than total_percent_used)
                let used_percent = if limit_cents > 0.0 {
                    (used_cents / limit_cents) * 100.0
                } else if let Some(total_pct) = plan.total_percent_used {
                    // Fallback to API-provided value, normalize if needed
                    if total_pct <= 1.0 {
                        total_pct * 100.0
                    } else {
                        total_pct
                    }
                } else {
                    0.0
                };

                // Convert cents to USD
                let used_usd = used_cents / 100.0;
                let limit_usd = limit_cents / 100.0;

                let primary = Some(RateWindow {
                    used_percent,
                    window_minutes: None,
                    resets_at: usage.billing_cycle_end.clone(),
                    reset_description: billing_cycle_end.map(|d| self.format_reset_time(&d)),
                    label: Some("Plan Usage".to_string()),
                });

                let credits = if limit_usd > 0.0 {
                    Some(Credits {
                        remaining: limit_usd - used_usd,
                        total: Some(limit_usd),
                        unit: "USD".to_string(),
                    })
                } else {
                    None
                };

                (primary, credits)
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        // Calculate on-demand usage (secondary rate window)
        let secondary = if let Some(individual) = &usage.individual_usage {
            if let Some(on_demand) = &individual.on_demand {
                if on_demand.enabled.unwrap_or(false) {
                    let used_cents = on_demand.used.unwrap_or(0) as f64;
                    let limit_cents = on_demand.limit.map(|l| l as f64);

                    let used_percent = match limit_cents {
                        Some(limit) if limit > 0.0 => (used_cents / limit) * 100.0,
                        _ => 0.0, // Unlimited or no limit
                    };

                    let used_usd = used_cents / 100.0;

                    // Only show if there's actual usage or it's enabled
                    if used_usd > 0.0 || on_demand.enabled.unwrap_or(false) {
                        Some(RateWindow {
                            used_percent,
                            window_minutes: None,
                            resets_at: usage.billing_cycle_end.clone(),
                            reset_description: Some(format!("${:.2} on-demand", used_usd)),
                            label: Some("On-Demand".to_string()),
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        // Calculate team on-demand usage (tertiary rate window)
        let tertiary = if let Some(team) = &usage.team_usage {
            if let Some(on_demand) = &team.on_demand {
                if on_demand.enabled.unwrap_or(false) {
                    let used_cents = on_demand.used.unwrap_or(0) as f64;
                    let limit_cents = on_demand.limit.map(|l| l as f64);

                    let used_percent = match limit_cents {
                        Some(limit) if limit > 0.0 => (used_cents / limit) * 100.0,
                        _ => 0.0,
                    };

                    let used_usd = used_cents / 100.0;

                    if used_usd > 0.0 {
                        Some(RateWindow {
                            used_percent,
                            window_minutes: None,
                            resets_at: usage.billing_cycle_end.clone(),
                            reset_description: Some(format!("${:.2} team on-demand", used_usd)),
                            label: Some("Team On-Demand".to_string()),
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        // Format membership type
        let plan = usage
            .membership_type
            .as_ref()
            .map(|t| self.format_membership_type(t));

        let email = user_info.as_ref().and_then(|u| u.email.clone());
        let name = user_info.as_ref().and_then(|u| u.name.clone());

        UsageSnapshot {
            primary,
            secondary,
            tertiary,
            credits,
            cost: None,
            identity: Some(ProviderIdentity {
                email,
                name,
                plan,
                organization: None,
            }),
            updated_at: chrono::Utc::now().to_rfc3339(),
            error: None,
        }
    }

    /// Format membership type to display name
    fn format_membership_type(&self, membership_type: &str) -> String {
        match membership_type.to_lowercase().as_str() {
            "pro" => "Pro+".to_string(), // Pro is marketed as Pro+
            "pro_plus" => "Pro+".to_string(),
            "enterprise" => "Enterprise".to_string(),
            "team" => "Team".to_string(),
            "hobby" => "Hobby".to_string(),
            "free" => "Free".to_string(),
            "business" => "Business".to_string(),
            other => {
                // Capitalize first letter for unknown types
                let mut chars = other.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    None => other.to_string(),
                }
            }
        }
    }

    fn parse_iso_date(&self, iso_time: &str) -> Option<chrono::DateTime<chrono::Utc>> {
        // Try parsing with fractional seconds first, then without
        chrono::DateTime::parse_from_rfc3339(iso_time)
            .ok()
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .or_else(|| {
                // Try ISO8601 without fractional seconds
                chrono::NaiveDateTime::parse_from_str(iso_time, "%Y-%m-%dT%H:%M:%SZ")
                    .ok()
                    .map(|ndt| ndt.and_utc())
            })
    }

    fn format_reset_time(&self, reset_date: &chrono::DateTime<chrono::Utc>) -> String {
        let now = chrono::Utc::now();
        let duration = reset_date.signed_duration_since(now);

        if duration.num_days() < 1 {
            let hours = duration.num_hours().max(1);
            format!("Resets in {}h", hours)
        } else {
            format!("Resets in {} days", duration.num_days())
        }
    }

    /// Try to load cookies from a stored session file
    async fn load_stored_cookies(&self) -> Result<String, anyhow::Error> {
        // Check for stored session in app data
        let session_path = self.get_session_path()?;

        if session_path.exists() {
            let content = tokio::fs::read_to_string(&session_path).await?;
            let session: CursorSession = serde_json::from_str(&content)?;
            return Ok(session.cookie_header);
        }

        Err(anyhow::anyhow!("No stored Cursor session found"))
    }

    fn get_session_path(&self) -> Result<std::path::PathBuf, anyhow::Error> {
        let data_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
        Ok(data_dir.join("IncuBar").join("cursor-session.json"))
    }
}

#[async_trait]
impl ProviderFetcher for CursorProvider {
    fn name(&self) -> &'static str {
        "Cursor"
    }

    fn description(&self) -> &'static str {
        "Cursor AI Code Editor"
    }

    async fn fetch(&self) -> Result<UsageSnapshot, anyhow::Error> {
        tracing::debug!("Fetching Cursor usage");

        // Try to load stored cookies
        match self.load_stored_cookies().await {
            Ok(cookies) => match self.fetch_with_cookies(&cookies).await {
                Ok(usage) => {
                    tracing::debug!("Cursor fetch successful");
                    return Ok(usage);
                }
                Err(e) => {
                    tracing::debug!("Cursor fetch with stored cookies failed: {}", e);
                }
            },
            Err(e) => {
                tracing::debug!("No stored Cursor cookies: {}", e);
            }
        }

        // TODO: Implement browser cookie import

        // Fallback to mock data
        Ok(UsageSnapshot {
            primary: Some(RateWindow {
                used_percent: 62.0,
                window_minutes: None,
                resets_at: Some((chrono::Utc::now() + chrono::Duration::days(12)).to_rfc3339()),
                reset_description: Some("Resets in 12 days".to_string()),
                label: Some("Monthly".to_string()),
            }),
            secondary: None,
            tertiary: None,
            credits: None,
            cost: None,
            identity: Some(ProviderIdentity {
                email: None,
                name: None,
                plan: Some("(Mock)".to_string()),
                organization: None,
            }),
            updated_at: chrono::Utc::now().to_rfc3339(),
            error: None,
        })
    }
}

// ---- Response Types ----

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CursorSession {
    cookie_header: String,
}

// Note: Some fields below are unused but required for serde deserialization
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CursorUsageSummary {
    billing_cycle_start: Option<String>,
    billing_cycle_end: Option<String>,
    membership_type: Option<String>,
    limit_type: Option<String>,
    is_unlimited: Option<bool>,
    individual_usage: Option<CursorIndividualUsage>,
    team_usage: Option<CursorTeamUsage>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CursorIndividualUsage {
    plan: Option<CursorPlanUsage>,
    on_demand: Option<CursorOnDemandUsage>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CursorTeamUsage {
    plan: Option<CursorPlanUsage>,
    on_demand: Option<CursorOnDemandUsage>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CursorPlanUsage {
    enabled: Option<bool>,
    used: Option<i32>,  // In cents
    limit: Option<i32>, // In cents
    remaining: Option<i32>,
    breakdown: Option<CursorPlanBreakdown>,
    total_percent_used: Option<f64>,
    auto_percent_used: Option<f64>,
    api_percent_used: Option<f64>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CursorPlanBreakdown {
    included: Option<i32>,
    bonus: Option<i32>,
    total: Option<i32>, // included + bonus
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CursorOnDemandUsage {
    enabled: Option<bool>,
    used: Option<i32>,
    limit: Option<i32>,
    remaining: Option<i32>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CursorUserInfo {
    email: Option<String>,
    email_verified: Option<bool>,
    name: Option<String>,
    sub: Option<String>,
    picture: Option<String>,
}
