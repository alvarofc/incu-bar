//! Provider system for fetching usage data from various AI services

mod traits;
mod claude;
mod codex;
mod cursor;
mod factory;
pub mod copilot;
mod zai;
mod kimi_k2;
mod synthetic;
mod gemini;

pub use traits::*;

use std::collections::HashMap;
use anyhow::anyhow;
use async_trait::async_trait;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Emitter};

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
        let default_enabled = vec![ProviderId::Claude, ProviderId::Codex, ProviderId::Cursor, ProviderId::Copilot];

        // Claude
        providers.insert(ProviderId::Claude, ProviderState {
            enabled: default_enabled.contains(&ProviderId::Claude),
            cached_usage: None,
            fetcher: Box::new(claude::ClaudeProvider::new()),
        });

        // Codex
        providers.insert(ProviderId::Codex, ProviderState {
            enabled: default_enabled.contains(&ProviderId::Codex),
            cached_usage: None,
            fetcher: Box::new(codex::CodexProvider::new()),
        });

        // Cursor
        providers.insert(ProviderId::Cursor, ProviderState {
            enabled: default_enabled.contains(&ProviderId::Cursor),
            cached_usage: None,
            fetcher: Box::new(cursor::CursorProvider::new()),
        });

        // Copilot
        providers.insert(ProviderId::Copilot, ProviderState {
            enabled: default_enabled.contains(&ProviderId::Copilot),
            cached_usage: None,
            fetcher: Box::new(copilot::CopilotProvider::new()),
        });

        // z.ai
        providers.insert(ProviderId::Zai, ProviderState {
            enabled: false, // Requires API token, not enabled by default
            cached_usage: None,
            fetcher: Box::new(zai::ZaiProvider::new()),
        });

        // Kimi K2
        providers.insert(ProviderId::KimiK2, ProviderState {
            enabled: false, // Requires API key, not enabled by default
            cached_usage: None,
            fetcher: Box::new(kimi_k2::KimiK2Provider::new()),
        });

        // Synthetic
        providers.insert(ProviderId::Synthetic, ProviderState {
            enabled: false, // Requires API key, not enabled by default
            cached_usage: None,
            fetcher: Box::new(synthetic::SyntheticProvider::new()),
        });

        // Gemini
        providers.insert(ProviderId::Gemini, ProviderState {
            enabled: false, // Requires Gemini CLI OAuth, not enabled by default
            cached_usage: None,
            fetcher: Box::new(gemini::GeminiProvider::new()),
        });

        // Antigravity
        providers.insert(ProviderId::Antigravity, Self::placeholder_state(
            "Antigravity",
            "Antigravity provider not implemented",
        ));

        // Factory (Droid)
        providers.insert(ProviderId::Factory, ProviderState {
            enabled: false,
            cached_usage: None,
            fetcher: Box::new(factory::FactoryProvider::new()),
        });

        // MiniMax
        providers.insert(ProviderId::Minimax, Self::placeholder_state(
            "MiniMax",
            "MiniMax provider not implemented",
        ));

        // Kimi
        providers.insert(ProviderId::Kimi, Self::placeholder_state(
            "Kimi",
            "Kimi provider not implemented",
        ));

        // Kiro
        providers.insert(ProviderId::Kiro, Self::placeholder_state(
            "Kiro",
            "Kiro provider not implemented",
        ));

        // Vertex AI
        providers.insert(ProviderId::Vertex, Self::placeholder_state(
            "Vertex AI",
            "Vertex AI provider not implemented",
        ));

        // Augment
        providers.insert(ProviderId::Augment, Self::placeholder_state(
            "Augment",
            "Augment provider not implemented",
        ));

        // Amp
        providers.insert(ProviderId::Amp, Self::placeholder_state(
            "Amp",
            "Amp provider not implemented",
        ));

        // JetBrains
        providers.insert(ProviderId::Jetbrains, Self::placeholder_state(
            "JetBrains AI",
            "JetBrains provider not implemented",
        ));

        // OpenCode
        providers.insert(ProviderId::Opencode, Self::placeholder_state(
            "OpenCode",
            "OpenCode provider not implemented",
        ));

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

    pub fn get_cached_usage(&self, _id: &ProviderId) -> Option<UsageSnapshot> {
        // For sync access, we'd need a different approach
        // For now, return None and let the frontend trigger a refresh
        None
    }

    pub fn get_all_cached_usage(&self) -> HashMap<ProviderId, UsageSnapshot> {
        HashMap::new()
    }

    pub fn get_enabled_providers(&self) -> Vec<ProviderId> {
        // Return default enabled for now
        vec![ProviderId::Claude, ProviderId::Codex, ProviderId::Cursor, ProviderId::Copilot]
    }

    pub fn set_enabled(&self, _id: &ProviderId, _enabled: bool) {
        // TODO: Implement with async lock
    }
}

/// Start the background refresh loop
pub async fn start_refresh_loop(app: AppHandle) {
    let interval = std::time::Duration::from_secs(300); // 5 minutes

    loop {
        tokio::time::sleep(interval).await;
        
        if let Some(registry) = app.try_state::<ProviderRegistry>() {
            let providers = registry.get_enabled_providers();
            
            for provider_id in providers {
                match registry.fetch_usage(&provider_id).await {
                    Ok(usage) => {
                        let _ = app.emit("usage-updated", serde_json::json!({
                            "providerId": provider_id,
                            "usage": usage,
                        }));
                    }
                    Err(e) => {
                        tracing::warn!("Refresh failed for {:?}: {}", provider_id, e);
                    }
                }
            }
        }
    }
}
