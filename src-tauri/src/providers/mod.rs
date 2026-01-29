//! Provider system for fetching usage data from various AI services

mod amp;
mod antigravity;
mod augment;
mod claude;
mod codex;
pub mod copilot;
mod cost_usage;
mod cursor;
mod factory;
mod gemini;
mod jetbrains;
mod kimi;
mod kimi_k2;
mod kiro;
mod minimax;
pub(crate) mod opencode;
mod synthetic;
mod traits;
mod zai;

pub use traits::*;

use anyhow::anyhow;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::timeout;
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

    pub fn validated(self) -> Self {
        let description = normalize_text(self.description);
        let updated_at = normalize_datetime(self.updated_at);
        Self {
            indicator: self.indicator,
            description,
            updated_at,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RefreshSchedule {
    interval: Duration,
    next_refresh_at: SystemTime,
}

const FAILURE_BACKOFF_BASE_SECONDS: u64 = 30;

impl RefreshSchedule {
    #[allow(dead_code)]
    fn new(interval: Duration) -> Self {
        Self::new_at(SystemTime::now(), interval)
    }

    pub(crate) fn new_at(now: SystemTime, interval: Duration) -> Self {
        Self {
            interval,
            next_refresh_at: now.checked_add(interval).unwrap_or(now),
        }
    }

    /// Create a schedule that is immediately due (for initial refresh)
    pub(crate) fn new_due_now(interval: Duration) -> Self {
        Self {
            interval,
            // Set next_refresh_at to epoch so is_due() returns true immediately
            next_refresh_at: SystemTime::UNIX_EPOCH,
        }
    }

    pub(crate) fn is_due(&self, now: SystemTime) -> bool {
        now >= self.next_refresh_at
    }

    pub(crate) fn mark_refreshed(&mut self, now: SystemTime) {
        self.next_refresh_at = now.checked_add(self.interval).unwrap_or(now);
    }

    pub(crate) fn schedule_after(&mut self, now: SystemTime, delay: Duration) {
        self.next_refresh_at = now.checked_add(delay).unwrap_or(now);
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ConsecutiveFailureGate {
    streak: u32,
}

impl ConsecutiveFailureGate {
    pub(crate) fn new() -> Self {
        Self { streak: 0 }
    }

    pub(crate) fn record_success(&mut self) {
        self.streak = 0;
    }

    #[allow(dead_code)]
    pub(crate) fn reset(&mut self) {
        self.streak = 0;
    }

    pub(crate) fn should_surface_error(&mut self, had_prior_data: bool) -> bool {
        self.streak = self.streak.saturating_add(1);
        if had_prior_data && self.streak == 1 {
            return false;
        }
        true
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RefreshBackoff {
    base_interval: Duration,
    failure_streak: u32,
}

impl RefreshBackoff {
    pub(crate) fn new(base_interval: Duration) -> Self {
        Self {
            base_interval,
            failure_streak: 0,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.failure_streak = 0;
    }

    pub(crate) fn register_failure(&mut self) -> Duration {
        self.failure_streak = self.failure_streak.saturating_add(1);
        self.backoff_delay()
    }

    pub(crate) fn backoff_delay(&self) -> Duration {
        let base_seconds = FAILURE_BACKOFF_BASE_SECONDS;
        let max_seconds = self.base_interval.as_secs().max(base_seconds);
        let exponent = self.failure_streak.saturating_sub(1).min(6);
        let multiplier = 1_u64 << exponent;
        let delay_seconds = base_seconds.saturating_mul(multiplier).min(max_seconds);
        Duration::from_secs(delay_seconds)
    }
}

#[derive(Debug, Clone)]
struct ProviderRefreshState {
    schedule: RefreshSchedule,
    backoff: RefreshBackoff,
    failure_gate: ConsecutiveFailureGate,
}

impl ProviderRefreshState {
    fn new(_now: SystemTime, interval: Duration) -> Self {
        Self {
            // Use new_due_now so the first refresh happens immediately
            schedule: RefreshSchedule::new_due_now(interval),
            backoff: RefreshBackoff::new(interval),
            failure_gate: ConsecutiveFailureGate::new(),
        }
    }

    fn is_due(&self, now: SystemTime) -> bool {
        self.schedule.is_due(now)
    }

    fn record_success(&mut self, now: SystemTime) {
        self.backoff.reset();
        self.failure_gate.record_success();
        self.schedule.mark_refreshed(now);
    }

    fn record_failure(&mut self, now: SystemTime, had_data: bool) -> bool {
        let should_surface = self.failure_gate.should_surface_error(had_data);
        let delay = self.backoff.register_failure();
        self.schedule.schedule_after(now, delay);
        should_surface
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

    pub fn validated(mut self) -> Self {
        self.primary = self.primary.and_then(RateWindow::validated);
        self.secondary = self.secondary.and_then(RateWindow::validated);
        self.tertiary = self.tertiary.and_then(RateWindow::validated);
        self.credits = self.credits.and_then(Credits::validated);
        self.cost = self.cost.and_then(CostSnapshot::validated);
        self.identity = self.identity.and_then(ProviderIdentity::validated);
        self.error = normalize_text(self.error);

        if chrono::DateTime::parse_from_rfc3339(self.updated_at.trim()).is_err() {
            self.updated_at = chrono::Utc::now().to_rfc3339();
        } else {
            self.updated_at = normalize_datetime(Some(self.updated_at))
                .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
        }

        self
    }
}

impl RateWindow {
    fn validated(mut self) -> Option<Self> {
        if !self.used_percent.is_finite() {
            return None;
        }
        self.used_percent = self.used_percent.clamp(0.0, 100.0);
        if let Some(minutes) = self.window_minutes {
            if minutes <= 0 {
                self.window_minutes = None;
            }
        }
        self.resets_at = normalize_datetime(self.resets_at);
        self.reset_description = normalize_text(self.reset_description);
        self.label = normalize_text(self.label);
        Some(self)
    }
}

impl Credits {
    fn validated(mut self) -> Option<Self> {
        let unit = self.unit.trim();
        if unit.is_empty() || !self.remaining.is_finite() {
            return None;
        }
        let remaining = self.remaining.max(0.0);
        let total = match self.total {
            Some(total) if total.is_finite() => Some(total.max(0.0).max(remaining)),
            Some(_) => return None,
            None => None,
        };
        self.remaining = remaining;
        self.total = total;
        self.unit = unit.to_string();
        Some(self)
    }
}

impl CostSnapshot {
    fn validated(mut self) -> Option<Self> {
        let currency = self.currency.trim();
        if currency.is_empty() || !self.today_amount.is_finite() || !self.month_amount.is_finite() {
            return None;
        }
        self.today_amount = self.today_amount.max(0.0);
        self.month_amount = self.month_amount.max(0.0);
        self.currency = currency.to_string();
        Some(self)
    }
}

impl ProviderIdentity {
    fn validated(mut self) -> Option<Self> {
        self.email = normalize_text(self.email);
        self.name = normalize_text(self.name);
        self.plan = normalize_text(self.plan);
        self.organization = normalize_text(self.organization);
        if self.email.is_none()
            && self.name.is_none()
            && self.plan.is_none()
            && self.organization.is_none()
        {
            None
        } else {
            Some(self)
        }
    }
}

fn normalize_text(value: Option<String>) -> Option<String> {
    value.and_then(|text| {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn normalize_datetime(value: Option<String>) -> Option<String> {
    let value = value?.trim().to_string();
    if value.is_empty() {
        return None;
    }
    chrono::DateTime::parse_from_rfc3339(&value)
        .ok()
        .map(|dt| dt.with_timezone(&chrono::Utc).to_rfc3339())
}

pub async fn load_cost_snapshot(provider: ProviderId) -> Option<CostSnapshot> {
    cost_usage::load_cost_snapshot(provider).await
}

/// Provider state
#[allow(dead_code)]
struct ProviderState {
    enabled: bool,
    cached_usage: Option<UsageSnapshot>,
    fetcher: Arc<dyn ProviderFetcher>,
}

/// Registry managing all providers
pub struct ProviderRegistry {
    providers: RwLock<HashMap<ProviderId, ProviderState>>,
    /// Flag indicating whether the frontend has synced enabled providers
    /// The refresh loop waits for this before starting to avoid using stale defaults
    frontend_synced: RwLock<bool>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        let mut providers = HashMap::new();

        // Start with all providers disabled - frontend will sync the correct list
        // This prevents the race condition where refresh loop starts before frontend sync

        // Claude
        providers.insert(
            ProviderId::Claude,
            ProviderState {
                enabled: false,
                cached_usage: None,
                fetcher: Arc::new(claude::ClaudeProvider::new()),
            },
        );

        // Codex
        providers.insert(
            ProviderId::Codex,
            ProviderState {
                enabled: false,
                cached_usage: None,
                fetcher: Arc::new(codex::CodexProvider::new()),
            },
        );

        // Cursor
        providers.insert(
            ProviderId::Cursor,
            ProviderState {
                enabled: false,
                cached_usage: None,
                fetcher: Arc::new(cursor::CursorProvider::new()),
            },
        );

        // Copilot
        providers.insert(
            ProviderId::Copilot,
            ProviderState {
                enabled: false,
                cached_usage: None,
                fetcher: Arc::new(copilot::CopilotProvider::new()),
            },
        );

        // z.ai
        providers.insert(
            ProviderId::Zai,
            ProviderState {
                enabled: false, // Requires API token, not enabled by default
                cached_usage: None,
                fetcher: Arc::new(zai::ZaiProvider::new()),
            },
        );

        // Kimi K2
        providers.insert(
            ProviderId::KimiK2,
            ProviderState {
                enabled: false, // Requires API key, not enabled by default
                cached_usage: None,
                fetcher: Arc::new(kimi_k2::KimiK2Provider::new()),
            },
        );

        // Synthetic
        providers.insert(
            ProviderId::Synthetic,
            ProviderState {
                enabled: false, // Requires API key, not enabled by default
                cached_usage: None,
                fetcher: Arc::new(synthetic::SyntheticProvider::new()),
            },
        );

        // Gemini
        providers.insert(
            ProviderId::Gemini,
            ProviderState {
                enabled: false, // Requires Gemini CLI OAuth, not enabled by default
                cached_usage: None,
                fetcher: Arc::new(gemini::GeminiProvider::new()),
            },
        );

        // Antigravity
        providers.insert(
            ProviderId::Antigravity,
            ProviderState {
                enabled: false,
                cached_usage: None,
                fetcher: Arc::new(antigravity::AntigravityProvider::new()),
            },
        );

        // Factory (Droid)
        providers.insert(
            ProviderId::Factory,
            ProviderState {
                enabled: false,
                cached_usage: None,
                fetcher: Arc::new(factory::FactoryProvider::new()),
            },
        );

        // MiniMax
        providers.insert(
            ProviderId::Minimax,
            ProviderState {
                enabled: false, // Requires browser cookies
                cached_usage: None,
                fetcher: Arc::new(minimax::MinimaxProvider::new()),
            },
        );

        // Kimi
        providers.insert(
            ProviderId::Kimi,
            ProviderState {
                enabled: false, // Requires browser cookies
                cached_usage: None,
                fetcher: Arc::new(kimi::KimiProvider::new()),
            },
        );

        // Kiro
        providers.insert(
            ProviderId::Kiro,
            ProviderState {
                enabled: false,
                cached_usage: None,
                fetcher: Arc::new(kiro::KiroProvider::new()),
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
                fetcher: Arc::new(augment::AugmentProvider::new()),
            },
        );

        // Amp
        providers.insert(
            ProviderId::Amp,
            ProviderState {
                enabled: false,
                cached_usage: None,
                fetcher: Arc::new(amp::AmpProvider::new()),
            },
        );

        // JetBrains
        providers.insert(
            ProviderId::Jetbrains,
            ProviderState {
                enabled: false,
                cached_usage: None,
                fetcher: Arc::new(jetbrains::JetbrainsProvider::new()),
            },
        );

        // OpenCode
        providers.insert(
            ProviderId::Opencode,
            ProviderState {
                enabled: false,
                cached_usage: None,
                fetcher: Arc::new(opencode::OpencodeProvider::new()),
            },
        );

        Self {
            providers: RwLock::new(providers),
            frontend_synced: RwLock::new(false),
        }
    }

    fn placeholder_state(name: &'static str, description: &'static str) -> ProviderState {
        ProviderState {
            enabled: false,
            cached_usage: None,
            fetcher: Arc::new(PlaceholderProvider::new(name, description)),
        }
    }

    pub async fn fetch_usage(&self, id: &ProviderId) -> Result<UsageSnapshot, anyhow::Error> {
        const FETCH_TIMEOUT_SECS: u64 = 15;
        
        // Clone the Arc<dyn ProviderFetcher> while holding the lock briefly, then drop the lock
        // This prevents lock starvation when one provider's fetch hangs
        tracing::debug!("fetch_usage: acquiring read lock for {:?}", id);
        let fetcher = {
            let providers = self.providers.read().await;
            tracing::debug!("fetch_usage: got read lock for {:?}", id);
            match providers.get(id) {
                Some(state) => Arc::clone(&state.fetcher),
                None => return Err(anyhow::anyhow!("Provider {:?} not found", id)),
            }
        };
        // Lock is now dropped, other providers can proceed
        
        tracing::debug!("fetch_usage: starting fetch for {:?}", id);
        // Add timeout to prevent hanging on cookie/network operations
        let fetch_result = tokio::select! {
            result = fetcher.fetch() => result,
            _ = tokio::time::sleep(Duration::from_secs(FETCH_TIMEOUT_SECS)) => {
                tracing::warn!("Provider {:?} fetch timed out after {}s", id, FETCH_TIMEOUT_SECS);
                return Err(anyhow!("Fetch timed out after {}s - browser may be blocking cookie access", FETCH_TIMEOUT_SECS));
            }
        };
        
        let usage = fetch_result?.validated();
        tracing::debug!("fetch_usage: fetch completed for {:?}", id);

        // Cache the result
        let mut providers = self.providers.write().await;
        if let Some(state) = providers.get_mut(id) {
            state.cached_usage = Some(usage.clone());
        }

        Ok(usage)
    }

    pub async fn fetch_status(&self, id: &ProviderId) -> Result<ProviderStatus, anyhow::Error> {
        const FETCH_TIMEOUT_SECS: u64 = 10;
        
        // Clone the Arc<dyn ProviderFetcher> while holding the lock briefly, then drop the lock
        // This prevents lock starvation when one provider's fetch hangs
        tracing::debug!("fetch_status: acquiring read lock for {:?}", id);
        let fetcher = {
            let providers = self.providers.read().await;
            tracing::debug!("fetch_status: got read lock for {:?}", id);
            match providers.get(id) {
                Some(state) => Arc::clone(&state.fetcher),
                None => return Err(anyhow::anyhow!("Provider {:?} not found", id)),
            }
        };
        // Lock is now dropped, other providers can proceed
        
        tracing::debug!("fetch_status: starting fetch for {:?}", id);
        let status = match timeout(Duration::from_secs(FETCH_TIMEOUT_SECS), fetcher.fetch_status()).await {
            Ok(result) => result?.validated(),
            Err(_) => {
                tracing::warn!("Provider {:?} status fetch timed out after {}s", id, FETCH_TIMEOUT_SECS);
                return Err(anyhow!("Status fetch timed out after {}s", FETCH_TIMEOUT_SECS));
            }
        };
        tracing::debug!("fetch_status: completed for {:?}", id);
        Ok(status)
    }

    pub async fn get_cached_usage(&self, id: &ProviderId) -> Option<UsageSnapshot> {
        self.providers
            .read()
            .await
            .get(id)
            .and_then(|state| state.cached_usage.clone())
    }

    pub async fn get_all_cached_usage(&self) -> HashMap<ProviderId, UsageSnapshot> {
        self.providers
            .read()
            .await
            .iter()
            .filter_map(|(id, state)| state.cached_usage.clone().map(|usage| (*id, usage)))
            .collect()
    }

    pub async fn get_enabled_providers(&self) -> Vec<ProviderId> {
        self.providers
            .read()
            .await
            .iter()
            .filter_map(|(id, state)| if state.enabled { Some(*id) } else { None })
            .collect()
    }

    pub async fn set_enabled(&self, id: &ProviderId, enabled: bool) {
        if let Some(state) = self.providers.write().await.get_mut(id) {
            state.enabled = enabled;
        }
    }

    pub async fn set_enabled_providers(&self, enabled: &[ProviderId]) {
        let enabled_set: std::collections::HashSet<ProviderId> = enabled.iter().copied().collect();
        let mut providers = self.providers.write().await;
        for (id, state) in providers.iter_mut() {
            state.enabled = enabled_set.contains(id);
        }
        drop(providers);
        // Mark frontend as synced - this allows the refresh loop to start
        *self.frontend_synced.write().await = true;
        tracing::info!("set_enabled_providers: synced {} providers from frontend, refresh loop can now start", enabled.len());
    }

    /// Check if the frontend has synced enabled providers
    pub async fn is_frontend_synced(&self) -> bool {
        *self.frontend_synced.read().await
    }

    /// Mark frontend as synced (fallback in case set_enabled_providers wasn't called)
    pub async fn mark_frontend_synced(&self) {
        *self.frontend_synced.write().await = true;
    }
}

#[cfg(test)]
mod registry_tests {
    use super::{ProviderId, ProviderRegistry, UsageSnapshot};

    #[tokio::test]
    async fn registry_starts_with_all_providers_disabled() {
        let registry = ProviderRegistry::new();

        // All providers start disabled - frontend will sync the correct list
        let enabled_providers = registry.get_enabled_providers().await;
        assert!(enabled_providers.is_empty());

        let cached_usage = registry.get_all_cached_usage().await;
        assert!(cached_usage.is_empty());

        // Enabling a provider works
        registry.set_enabled(&ProviderId::Claude, true).await;
        let enabled_after = registry.get_enabled_providers().await;
        assert!(enabled_after.contains(&ProviderId::Claude));

        let usage = UsageSnapshot::error("unit test".to_string());
        {
            let mut providers = registry.providers.write().await;
            if let Some(state) = providers.get_mut(&ProviderId::Claude) {
                state.cached_usage = Some(usage.clone());
            }
        }

        let cached = registry.get_cached_usage(&ProviderId::Claude).await;
        assert_eq!(
            cached.and_then(|snapshot| snapshot.error),
            Some("unit test".to_string())
        );
    }

    #[tokio::test]
    async fn set_enabled_providers_marks_frontend_synced() {
        let registry = ProviderRegistry::new();

        assert!(!registry.is_frontend_synced().await);

        registry.set_enabled_providers(&[ProviderId::Cursor, ProviderId::Copilot]).await;

        assert!(registry.is_frontend_synced().await);
        let enabled = registry.get_enabled_providers().await;
        assert!(enabled.contains(&ProviderId::Cursor));
        assert!(enabled.contains(&ProviderId::Copilot));
        assert!(!enabled.contains(&ProviderId::Claude));
        assert!(!enabled.contains(&ProviderId::Codex));
    }
}

/// Start the background refresh loop
pub async fn start_refresh_loop(app: AppHandle) {
    let interval = std::time::Duration::from_secs(300); // 5 minutes
    let tick_interval = std::time::Duration::from_secs(5);
    let mut provider_states: HashMap<ProviderId, ProviderRefreshState> = HashMap::new();

    // Wait for frontend to sync enabled providers before starting refresh
    // This prevents the refresh loop from using hardcoded defaults
    tracing::info!("start_refresh_loop: waiting for frontend to sync enabled providers...");
    let max_wait = std::time::Duration::from_secs(30);
    let wait_start = std::time::Instant::now();
    loop {
        if let Some(registry) = app.try_state::<ProviderRegistry>() {
            if registry.is_frontend_synced().await {
                tracing::info!("start_refresh_loop: frontend synced, starting refresh loop");
                break;
            }
        }
        if wait_start.elapsed() > max_wait {
            tracing::warn!("start_refresh_loop: timed out waiting for frontend sync after {:?}, starting anyway", max_wait);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    loop {
        tokio::time::sleep(tick_interval).await;
        let now = SystemTime::now();

        if let Some(registry) = app.try_state::<ProviderRegistry>() {
            let providers = registry.get_enabled_providers().await;
            for provider_id in &providers {
                provider_states
                    .entry(*provider_id)
                    .or_insert_with(|| ProviderRefreshState::new(now, interval));
            }

            for provider_id in providers {
                let state = provider_states
                    .entry(provider_id)
                    .or_insert_with(|| ProviderRefreshState::new(now, interval));
                let had_cached_data = registry.get_cached_usage(&provider_id).await.is_some();

                if !state.is_due(now) {
                    continue;
                }

                // Skip unauthenticated providers to avoid wasting resources
                let provider_id_str = serde_json::to_string(&provider_id)
                    .unwrap_or_default()
                    .trim_matches('"')
                    .to_string();
                let auth_status = crate::login::check_auth_status(&provider_id_str).await;
                if !auth_status.authenticated {
                    tracing::debug!("start_refresh_loop: skipping {:?} - not authenticated", provider_id);
                    continue;
                }

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
                        state.record_success(now);
                    }
                    Err(e) => {
                        tracing::warn!("Refresh failed for {:?}: {}", provider_id, e);
                        if state.record_failure(now, had_cached_data) {
                            let usage = UsageSnapshot::error(e.to_string());
                            let _ = app.emit(
                                "refresh-failed",
                                serde_json::json!({
                                    "providerId": provider_id,
                                    "usage": usage.clone(),
                                }),
                            );
                            let _ = app.emit(
                                "usage-updated",
                                serde_json::json!({
                                    "providerId": provider_id,
                                    "usage": usage,
                                }),
                            );
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ConsecutiveFailureGate, Credits, ProviderIdentity, ProviderStatus, RateWindow,
        RefreshBackoff, RefreshSchedule, StatusIndicator, UsageSnapshot,
    };
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

    #[test]
    fn refresh_schedule_supports_custom_delay() {
        let start = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        let mut schedule = RefreshSchedule::new_at(start, Duration::from_secs(300));
        let delay = Duration::from_secs(45);
        schedule.schedule_after(start, delay);

        let before_due = start + Duration::from_secs(44);
        assert!(!schedule.is_due(before_due));

        let due = start + Duration::from_secs(45);
        assert!(schedule.is_due(due));
    }

    #[test]
    fn backoff_caps_at_interval() {
        let interval = Duration::from_secs(300);
        let mut backoff = RefreshBackoff::new(interval);

        let first = backoff.register_failure();
        assert_eq!(first, Duration::from_secs(30));

        for _ in 0..5 {
            backoff.register_failure();
        }

        let capped = backoff.register_failure();
        assert_eq!(capped, interval);
    }

    #[test]
    fn failure_gate_suppresses_first_error_with_prior_data() {
        let mut gate = ConsecutiveFailureGate::new();
        assert!(!gate.should_surface_error(true));
        assert!(gate.should_surface_error(true));
        gate.record_success();
        assert!(!gate.should_surface_error(true));
    }

    #[test]
    fn usage_snapshot_validation_clamps_and_cleans() {
        let snapshot = UsageSnapshot {
            primary: Some(RateWindow {
                used_percent: 180.0,
                window_minutes: Some(-5),
                resets_at: Some("bad".to_string()),
                reset_description: Some("  ".to_string()),
                label: Some(" Session ".to_string()),
            }),
            secondary: None,
            tertiary: None,
            credits: Some(Credits {
                remaining: -10.0,
                total: Some(5.0),
                unit: " credits ".to_string(),
            }),
            cost: None,
            identity: Some(ProviderIdentity {
                email: Some(" ".to_string()),
                name: Some("Alex".to_string()),
                plan: None,
                organization: None,
            }),
            updated_at: "invalid".to_string(),
            error: Some(" ".to_string()),
        };

        let validated = snapshot.validated();
        let primary = validated.primary.expect("primary");
        assert_eq!(primary.used_percent, 100.0);
        assert!(primary.window_minutes.is_none());
        assert!(primary.resets_at.is_none());
        assert!(primary.reset_description.is_none());
        assert_eq!(primary.label.as_deref(), Some("Session"));

        let credits = validated.credits.expect("credits");
        assert_eq!(credits.remaining, 0.0);
        assert_eq!(credits.total, Some(5.0));
        assert_eq!(credits.unit, "credits");

        let identity = validated.identity.expect("identity");
        assert!(identity.email.is_none());
        assert_eq!(identity.name.as_deref(), Some("Alex"));

        assert!(validated.error.is_none());
    }

    #[test]
    fn usage_snapshot_validation_drops_empty_identity_and_invalid_credits() {
        let snapshot = UsageSnapshot {
            primary: None,
            secondary: None,
            tertiary: None,
            credits: Some(Credits {
                remaining: 5.0,
                total: None,
                unit: " ".to_string(),
            }),
            cost: None,
            identity: Some(ProviderIdentity {
                email: Some(" ".to_string()),
                name: None,
                plan: None,
                organization: None,
            }),
            updated_at: chrono::Utc::now().to_rfc3339(),
            error: None,
        };

        let validated = snapshot.validated();
        assert!(validated.credits.is_none());
        assert!(validated.identity.is_none());
    }

    #[test]
    fn provider_status_validation_trims_fields() {
        let status = ProviderStatus {
            indicator: StatusIndicator::Major,
            description: Some("  degraded ".to_string()),
            updated_at: Some("bad".to_string()),
        };

        let validated = status.validated();
        assert_eq!(validated.description.as_deref(), Some("degraded"));
        assert!(validated.updated_at.is_none());
    }
}
