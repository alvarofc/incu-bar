//! Secure credential storage using the system keyring

use keyring::Entry;
use thiserror::Error;

const SERVICE_NAME: &str = "com.incubar.app";

#[derive(Error, Debug)]
pub enum KeyringError {
    #[error("Keyring error: {0}")]
    Keyring(#[from] keyring::Error),
    #[error("Credential not found")]
    NotFound,
}

/// Secure storage wrapper for the system keyring
pub struct SecureStorage {
    service: &'static str,
}

impl SecureStorage {
    pub fn new() -> Self {
        Self {
            service: SERVICE_NAME,
        }
    }

    /// Store a credential in the keyring
    pub fn store(&self, key: &str, value: &str) -> Result<(), KeyringError> {
        let entry = Entry::new(self.service, key)?;
        entry.set_password(value)?;
        tracing::debug!("Stored credential for key: {}", key);
        Ok(())
    }

    /// Retrieve a credential from the keyring
    pub fn get(&self, key: &str) -> Result<String, KeyringError> {
        let entry = Entry::new(self.service, key)?;
        match entry.get_password() {
            Ok(password) => Ok(password),
            Err(keyring::Error::NoEntry) => Err(KeyringError::NotFound),
            Err(e) => Err(KeyringError::Keyring(e)),
        }
    }

    /// Delete a credential from the keyring
    pub fn delete(&self, key: &str) -> Result<(), KeyringError> {
        let entry = Entry::new(self.service, key)?;
        entry.delete_credential()?;
        tracing::debug!("Deleted credential for key: {}", key);
        Ok(())
    }

    /// Check if a credential exists
    pub fn exists(&self, key: &str) -> bool {
        self.get(key).is_ok()
    }
}

impl Default for SecureStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_roundtrip() {
        let storage = SecureStorage::new();
        let test_key = "test_incubar_key";
        let test_value = "test_secret_value";

        // Clean up any existing entry
        let _ = storage.delete(test_key);

        // Store
        if let Err(err) = storage.store(test_key, test_value) {
            if matches!(err, KeyringError::Keyring(_)) {
                return;
            }
            panic!("Failed to store: {:?}", err);
        }

        // Retrieve
        let retrieved = storage.get(test_key).expect("Failed to get");
        assert_eq!(retrieved, test_value);

        // Delete
        storage.delete(test_key).expect("Failed to delete");

        // Verify deleted
        assert!(matches!(storage.get(test_key), Err(KeyringError::NotFound)));
    }
}
