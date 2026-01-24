//! Storage utilities for credentials and settings

pub mod keyring;
pub mod secure_delete;
pub mod widget_snapshot;
pub mod install_origin;

// Re-export commonly used items
pub use keyring::SecureStorage;
