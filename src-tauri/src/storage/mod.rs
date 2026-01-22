//! Storage utilities for credentials and settings

pub mod keyring;

// Re-export commonly used items
pub use keyring::SecureStorage;
