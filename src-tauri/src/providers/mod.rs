//! Provider system for fetching usage data from various AI services

mod amp;
mod antigravity;
mod augment;
mod claude;
mod codex;
pub mod copilot;
mod cursor;
mod factory;
mod gemini;
mod jetbrains;
mod kimi;
mod kimi_k2;
mod kiro;
mod minimax;
pub(crate) mod opencode;
mod cost_usage;
mod synthetic;
mod traits;
mod zai;

pub use traits::*;

use anyhow::anyhow;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::RwLock;

/// Provider identifier enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderId {
    Claude,
    Codex,
    Cursor,
    Copilot,
    Gemini,
    Antigravity,
    Factory,
    Zai,
    Minimax,
    Kimi,
    #[serde(rename = "kimi_k2")]
    KimiK2,
    Kiro,
    #[serde(rename = "vertexai")]
    Vertex,
    Augment,
    Amp,
    Jetbrains,
    Opencode,
    Synthetic,
}

impl ProviderId {
    pub fn all() -> Vec<ProviderId> {
        vec![
            ProviderId::Claude,
            ProviderId::Codex,
            ProviderId::Cursor,
            ProviderId::Copilot,
            ProviderId::Gemini,
            ProviderId::Antigravity,
            ProviderId::Factory,
            ProviderId::Zai,
            ProviderId::Minimax,
            ProviderId::Kimi,
            ProviderId::KimiK2,
            ProviderId::Kiro,
            ProviderId::Vertex,
            ProviderId::Augment,
            ProviderId::Amp,
            ProviderId::Jetbrains,
            ProviderId::Opencode,
            ProviderId::Synthetic,
        ]
    }
}

struct PlaceholderProvider {
    name: &'static str,
    description: &'static str,
}

impl PlaceholderProvider {
    fn new(name: &'static str, description: &'static str) -> Self {
        Self { name, description }
    }
}

#[async_trait]
impl ProviderFetcher for PlaceholderProvider {
    fn name(&self) -> &'static str {
        self.name
    }

    fn description(&self) -> &'static str {
        self.description
    }

    async fn fetch(&self) -> Result<UsageSnapshot, anyhow::Error> {
        Err(anyhow!("Provider not implemented"))
    }
}

/// Rate window (usage period)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateWindow {
    pub used_percent: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_minutes: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reset_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// Credits information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Credits {
    pub remaining: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<f64>,
    pub unit: String,
}

/// Cost snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CostSnapshot {
    pub today_amount: f64,
    pub today_tokens: u64,
    pub month_amount: f64,
    pub month_tokens: u64,
    pub currency: String,
}

/// Provider identity
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderIdentity {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization: Option<String>,
}

/// Full usage snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSnapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary: Option<RateWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary: Option<RateWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tertiary: Option<RateWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credits: Option<Credits>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<CostSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity: Option<ProviderIdentity>,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusIndicator {
    None,
    Minor,
    Major,
    Critical,
    Maintenance,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub indicator: StatusIndicator,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

impl ProviderStatus {
    pub fn none() -> Self {
        Self {
            indicator: StatusIndicator::None,
            description: None,
            updated_at: None,
        }
    }

    pub fn is_incident(&self) -> bool {
        self.indicator != StatusIndicator::None
    }
}

#[derive(Debug, Clone)]
struct RefreshSchedule {
    interval: Duration,
    next_refresh_at: SystemTime,
}

impl RefreshSchedule {
    fn new(interval: Duration) -> Self {
        Self::new_at(SystemTime::now(), interval)
    }

    fn new_at(now: SystemTime, interval: Duration) -> Self {
        Self {
            interval,
            next_refresh_at: now.checked_add(interval).unwrap_or(now),
        }
    }

    fn is_due(&self, now: SystemTime) -> bool {
        now >= self.next_refresh_at
    }

    fn mark_refreshed(&mut self, now: SystemTime) {
        self.next_refresh_at = now.checked_add(self.interval).unwrap_or(now);
    }
}

impl UsageSnapshot {
    pub fn error(message: String) -> Self {
        Self {
            primary: None,
            secondary: None,
            tertiary: None,
            credits: None,
            cost: None,
            identity: None,
            updated_at: chrono::Utc::now().to_rfc3339(),
            error: Some(message),
        }
    }
}

pub async fn load_cost_snapshot(provider: ProviderId) -> Option<CostSnapshot> {
    cost_usage::load_cost_snapshot(provider).await
}

/// Provider state
#[allow(dead_code)]
struct ProviderState {
    enabled: bool,
    cached_usage: Option<UsageSnapshot>,
    fetcher: Box<dyn ProviderFetcher>,
}

/// Registry managing all providers
pub struct ProviderRegistry {
    providers: RwLock<HashMap<ProviderId, ProviderState>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        let mut providers = HashMap::new();

        // Initialize with default enabled providers
        let default_enabled = vec![
            ProviderId::Claude,
            ProviderId::Codex,
            ProviderId::Cursor,
            ProviderId::Copilot,
        ];

        // Claude
        providers.insert(
            ProviderId::Claude,
            ProviderState {
                enabled: default_enabled.contains(&ProviderId::Claude),
                cached_usage: None,
                fetcher: Box::new(claude::ClaudeProvider::new()),
            },
        );

        // Codex
        providers.insert(
            ProviderId::Codex,
            ProviderState {
                enabled: default_enabled.contains(&ProviderId::Codex),
                cached_usage: None,
                fetcher: Box::new(codex::CodexProvider::new()),
            },
        );

        // Cursor
        providers.insert(
            ProviderId::Cursor,
            ProviderState {
                enabled: default_enabled.contains(&ProviderId::Cursor),
                cached_usage: None,
                fetcher: Box::new(cursor::CursorProvider::new()),
            },
        );

        // Copilot
        providers.insert(
            ProviderId::Copilot,
            ProviderState {
                enabled: default_enabled.contains(&ProviderId::Copilot),
                cached_usage: None,
                fetcher: Box::new(copilot::CopilotProvider::new()),
            },
        );

        // z.ai
        providers.insert(
            ProviderId::Zai,
            ProviderState {
                enabled: false, // Requires API token, not enabled by default
                cached_usage: None,
                fetcher: Box::new(zai::ZaiProvider::new()),
            },
        );

        // Kimi K2
        providers.insert(
            ProviderId::KimiK2,
            ProviderState {
                enabled: false, // Requires API key, not enabled by default
                cached_usage: None,
                fetcher: Box::new(kimi_k2::KimiK2Provider::new()),
            },
        );

        // Synthetic
        providers.insert(
            ProviderId::Synthetic,
            ProviderState {
                enabled: false, // Requires API key, not enabled by default
                cached_usage: None,
                fetcher: Box::new(synthetic::SyntheticProvider::new()),
            },
        );

        // Gemini
        providers.insert(
            ProviderId::Gemini,
            ProviderState {
                enabled: false, // Requires Gemini CLI OAuth, not enabled by default
                cached_usage: None,
                fetcher: Box::new(gemini::GeminiProvider::new()),
            },
        );

        // Antigravity
        providers.insert(
            ProviderId::Antigravity,
            ProviderState {
                enabled: false,
                cached_usage: None,
                fetcher: Box::new(antigravity::AntigravityProvider::new()),
            },
        );

        // Factory (Droid)
        providers.insert(
            ProviderId::Factory,
            ProviderState {
                enabled: false,
                cached_usage: None,
                fetcher: Box::new(factory::FactoryProvider::new()),
            },
        );

        // MiniMax
        providers.insert(
            ProviderId::Minimax,
            ProviderState {
                enabled: false, // Requires browser cookies
                cached_usage: None,
                fetcher: Box::new(minimax::MinimaxProvider::new()),
            },
        );

        // Kimi
        providers.insert(
            ProviderId::Kimi,
            ProviderState {
                enabled: false, // Requires browser cookies
                cached_usage: None,
                fetcher: Box::new(kimi::KimiProvider::new()),
            },
        );

        // Kiro
        providers.insert(
            ProviderId::Kiro,
            ProviderState {
                enabled: false,
                cached_usage: None,
                fetcher: Box::new(kiro::KiroProvider::new()),
            },
        );

        // Vertex AI
        providers.insert(
            ProviderId::Vertex,
            Self::placeholder_state("Vertex AI", "Vertex AI provider not implemented"),
        );

        // Augment
        providers.insert(
            ProviderId::Augment,
            ProviderState {
                enabled: false,
                cached_usage: None,
                fetcher: Box::new(augment::AugmentProvider::new()),
            },
        );

        // Amp
        providers.insert(
            ProviderId::Amp,
            ProviderState {
                enabled: false,
                cached_usage: None,
                fetcher: Box::new(amp::AmpProvider::new()),
            },
        );

        // JetBrains
        providers.insert(
            ProviderId::Jetbrains,
            ProviderState {
                enabled: false,
                cached_usage: None,
                fetcher: Box::new(jetbrains::JetbrainsProvider::new()),
            },
        );

        // OpenCode
        providers.insert(
            ProviderId::Opencode,
            ProviderState {
                enabled: false,
                cached_usage: None,
                fetcher: Box::new(opencode::OpencodeProvider::new()),
            },
        );

        Self {
            providers: RwLock::new(providers),
        }
    }

    fn placeholder_state(name: &'static str, description: &'static str) -> ProviderState {
        ProviderState {
            enabled: false,
            cached_usage: None,
            fetcher: Box::new(PlaceholderProvider::new(name, description)),
        }
    }

    pub async fn fetch_usage(&self, id: &ProviderId) -> Result<UsageSnapshot, anyhow::Error> {
        let providers = self.providers.read().await;

        if let Some(state) = providers.get(id) {
            let usage = state.fetcher.fetch().await?;
            drop(providers);

            // Cache the result
            let mut providers = self.providers.write().await;
            if let Some(state) = providers.get_mut(id) {
                state.cached_usage = Some(usage.clone());
            }

            Ok(usage)
        } else {
            Err(anyhow::anyhow!("Provider {:?} not found", id))
        }
    }

    pub async fn fetch_status(&self, id: &ProviderId) -> Result<ProviderStatus, anyhow::Error> {
        let providers = self.providers.read().await;
        if let Some(state) = providers.get(id) {
            let status = state.fetcher.fetch_status().await?;
            Ok(status)
        } else {
            Err(anyhow::anyhow!("Provider {:?} not found", id))
        }
    }

    pub fn get_cached_usage(&self, _id: &ProviderId) -> Option<UsageSnapshot> {
        // For sync access, we'd need a different approach
        // For now, return None and let the frontend trigger a refresh
        None
    }

    pub fn get_all_cached_usage(&self) -> HashMap<ProviderId, UsageSnapshot> {
        HashMap::new()
    }

    pub fn get_enabled_providers(&self) -> Vec<ProviderId> {
        self.providers
            .blocking_read()
            .iter()
            .filter_map(|(id, state)| if state.enabled { Some(*id) } else { None })
            .collect()
    }

    pub fn set_enabled(&self, id: &ProviderId, enabled: bool) {
        if let Some(state) = self.providers.blocking_write().get_mut(id) {
            state.enabled = enabled;
        }
    }
}

/// Start the background refresh loop
    pub async fn start_refresh_loop(app: AppHandle) {
    let interval = std::time::Duration::from_secs(300); // 5 minutes
    let tick_interval = std::time::Duration::from_secs(5);
    let mut schedule = RefreshSchedule::new(interval);

    loop {
        tokio::time::sleep(tick_interval).await;
        let now = SystemTime::now();

        if !schedule.is_due(now) {
            continue;
        }

        if let Some(registry) = app.try_state::<ProviderRegistry>() {
            let providers = registry.get_enabled_providers();

            for provider_id in providers {
                if let Ok(status) = registry.fetch_status(&provider_id).await {
                    let _ = app.emit(
                        "status-updated",
                        serde_json::json!({
                            "providerId": provider_id,
                            "status": status,
                        }),
                    );
                }
                match registry.fetch_usage(&provider_id).await {
                    Ok(usage) => {
                        let _ = app.emit(
                            "usage-updated",
                            serde_json::json!({
                                "providerId": provider_id,
                                "usage": usage,
                            }),
                        );
                    }
                    Err(e) => {
                        tracing::warn!("Refresh failed for {:?}: {}", provider_id, e);
                    }
                }
            }
        }

        schedule.mark_refreshed(SystemTime::now());
    }
}

#[cfg(test)]
mod tests {
    use super::RefreshSchedule;
    use std::time::{Duration, SystemTime};

    #[test]
    fn refresh_schedule_waits_until_due() {
        let start = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        let mut schedule = RefreshSchedule::new_at(start, Duration::from_secs(300));

        let before_due = start + Duration::from_secs(299);
        assert!(!schedule.is_due(before_due));

        let due = start + Duration::from_secs(300);
        assert!(schedule.is_due(due));

        schedule.mark_refreshed(due);
        let after_refresh = due + Duration::from_secs(1);
        assert!(!schedule.is_due(after_refresh));
    }

    #[test]
    fn refresh_schedule_handles_sleep_gaps() {
        let start = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        let mut schedule = RefreshSchedule::new_at(start, Duration::from_secs(300));

        let wake_time = start + Duration::from_secs(3_600);
        assert!(schedule.is_due(wake_time));

        schedule.mark_refreshed(wake_time);
        let next_due = wake_time + Duration::from_secs(299);
        assert!(!schedule.is_due(next_due));
    }
}
