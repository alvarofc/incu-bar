//! Provider trait definition

use async_trait::async_trait;
use super::UsageSnapshot;

/// Trait for implementing a provider fetcher
#[async_trait]
pub trait ProviderFetcher: Send + Sync {
    /// Fetch the current usage data for this provider
    async fn fetch(&self) -> Result<UsageSnapshot, anyhow::Error>;
    
    /// Get the provider name for display
    fn name(&self) -> &'static str;
    
    /// Get the provider description
    fn description(&self) -> &'static str {
        ""
    }
}
