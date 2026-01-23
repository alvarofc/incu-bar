//! Provider trait definition

use super::{ProviderStatus, UsageSnapshot};
use async_trait::async_trait;

/// Trait for implementing a provider fetcher
#[async_trait]
pub trait ProviderFetcher: Send + Sync {
    /// Fetch the current usage data for this provider
    async fn fetch(&self) -> Result<UsageSnapshot, anyhow::Error>;

    /// Fetch provider incident/status information
    async fn fetch_status(&self) -> Result<ProviderStatus, anyhow::Error> {
        Ok(ProviderStatus::none())
    }

    /// Get the provider name for display
    fn name(&self) -> &'static str;

    /// Get the provider description
    fn description(&self) -> &'static str {
        ""
    }
}
