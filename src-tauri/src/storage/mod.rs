//! Storage utilities for credentials and settings

pub mod install_origin;
pub mod keyring;
pub mod secure_delete;
pub mod widget_snapshot;

// Re-export commonly used items
pub use keyring::SecureStorage;
