//! Secure secret storage using platform keyrings.
//!
//! Provides encrypted storage for API keys and sensitive configuration
//! using platform-specific credential managers:
//! - Linux: Secret Service (GNOME Keyring, KWallet)
//! - macOS: Keychain
//! - Windows: Credential Manager

use keyring::Entry;
use thiserror::Error;
use tracing::{info, warn};

/// Service name for keyring entries
const SERVICE_NAME: &str = "openhush";

/// Errors that can occur during secret operations.
#[derive(Error, Debug)]
pub enum SecretError {
    #[error("Secret '{0}' not found")]
    NotFound(String),

    #[error("Keyring error: {0}")]
    Keyring(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<keyring::Error> for SecretError {
    fn from(e: keyring::Error) -> Self {
        match e {
            keyring::Error::NoEntry => SecretError::NotFound("(unknown)".to_string()),
            _ => SecretError::Keyring(e.to_string()),
        }
    }
}

/// Secure secret storage using platform keyrings.
///
/// # Example
///
/// ```ignore
/// let store = SecretStore::new();
///
/// // Store a secret
/// store.set("api-key", "sk-abc123")?;
///
/// // Retrieve it
/// let key = store.get("api-key")?;
///
/// // Delete it
/// store.delete("api-key")?;
/// ```
pub struct SecretStore {
    service: String,
}

impl SecretStore {
    /// Create a new secret store.
    pub fn new() -> Self {
        Self {
            service: SERVICE_NAME.to_string(),
        }
    }

    /// Store a secret in the keyring.
    pub fn set(&self, name: &str, value: &str) -> Result<(), SecretError> {
        let entry =
            Entry::new(&self.service, name).map_err(|e| SecretError::Keyring(e.to_string()))?;
        entry.set_password(value)?;
        info!("Secret '{}' stored in keyring", name);
        Ok(())
    }

    /// Retrieve a secret from the keyring.
    pub fn get(&self, name: &str) -> Result<String, SecretError> {
        let entry =
            Entry::new(&self.service, name).map_err(|e| SecretError::Keyring(e.to_string()))?;
        match entry.get_password() {
            Ok(password) => Ok(password),
            Err(keyring::Error::NoEntry) => Err(SecretError::NotFound(name.to_string())),
            Err(e) => Err(SecretError::Keyring(e.to_string())),
        }
    }

    /// Delete a secret from the keyring.
    pub fn delete(&self, name: &str) -> Result<(), SecretError> {
        let entry =
            Entry::new(&self.service, name).map_err(|e| SecretError::Keyring(e.to_string()))?;
        match entry.delete_credential() {
            Ok(()) => {
                info!("Secret '{}' deleted from keyring", name);
                Ok(())
            }
            Err(keyring::Error::NoEntry) => Err(SecretError::NotFound(name.to_string())),
            Err(e) => Err(SecretError::Keyring(e.to_string())),
        }
    }

    /// List all stored secret names.
    ///
    /// Note: The keyring crate doesn't support listing entries directly.
    /// This returns an empty list - users should track their secret names.
    #[allow(dead_code)]
    pub fn list(&self) -> Vec<String> {
        // The keyring crate doesn't support listing entries
        // Users need to track their secret names manually
        warn!("Keyring doesn't support listing entries - returning empty list");
        Vec::new()
    }

    /// Check if the keyring is available on this system.
    pub fn is_available() -> bool {
        // Try to create an entry to check if keyring is accessible
        Entry::new(SERVICE_NAME, "__availability_check__").is_ok()
    }
}

impl Default for SecretStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolve a secret value, checking for keyring: prefix.
///
/// If the value starts with "keyring:", the rest is treated as a secret name
/// and looked up in the system keyring. Otherwise, the value is returned as-is
/// (with a warning about plaintext storage).
///
/// # Example
///
/// ```ignore
/// // Looks up "api-key" in keyring
/// let secret = resolve_secret("keyring:api-key", &store)?;
///
/// // Returns "plaintext-value" directly (with warning)
/// let value = resolve_secret("plaintext-value", &store)?;
/// ```
#[allow(dead_code)]
pub fn resolve_secret(value: &str, store: &SecretStore) -> Result<String, SecretError> {
    if let Some(key_name) = value.strip_prefix("keyring:") {
        store.get(key_name)
    } else {
        // Plaintext value - warn user
        warn!(
            "Secret stored in plaintext config. Consider using 'keyring:name' for better security."
        );
        Ok(value.to_string())
    }
}

/// Prompt user for a secret value (hidden input).
pub fn prompt_secret(prompt: &str) -> Result<String, SecretError> {
    rpassword::prompt_password(prompt).map_err(SecretError::from)
}

/// CLI handlers for secret management commands.
pub mod cli {
    use super::*;

    /// Handle `openhush secret set <name>` command.
    pub fn handle_set(name: &str) -> Result<(), SecretError> {
        let secret = prompt_secret("Enter secret: ")?;
        if secret.is_empty() {
            eprintln!("Error: Secret cannot be empty");
            std::process::exit(1);
        }
        let store = SecretStore::new();
        store.set(name, &secret)?;
        println!("Secret '{}' stored securely.", name);
        Ok(())
    }

    /// Handle `openhush secret list` command.
    pub fn handle_list() {
        println!("Note: The system keyring doesn't support listing entries.");
        println!("Track your secret names manually. Common names:");
        println!("  - ollama-api");
        println!("  - openai-api");
        println!("  - webhook-url");
        println!();
        println!("To check if a specific secret exists:");
        println!("  openhush secret show <name>");
    }

    /// Handle `openhush secret delete <name>` command.
    pub fn handle_delete(name: &str) -> Result<(), SecretError> {
        let store = SecretStore::new();
        store.delete(name)?;
        println!("Secret '{}' deleted.", name);
        Ok(())
    }

    /// Handle `openhush secret show <name>` command.
    pub fn handle_show(name: &str, force: bool) -> Result<(), SecretError> {
        if !force {
            eprintln!("WARNING: This will display the secret in your terminal.");
            eprint!("Continue? [y/N] ");

            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Aborted.");
                return Ok(());
            }
        }

        let store = SecretStore::new();
        let secret = store.get(name)?;
        println!("{}", secret);
        Ok(())
    }

    /// Handle `openhush secret check` command - verify keyring availability.
    pub fn handle_check() {
        if SecretStore::is_available() {
            println!("Keyring is available and working.");

            #[cfg(target_os = "linux")]
            println!("Backend: Secret Service (GNOME Keyring / KWallet)");

            #[cfg(target_os = "macos")]
            println!("Backend: macOS Keychain");

            #[cfg(target_os = "windows")]
            println!("Backend: Windows Credential Manager");
        } else {
            eprintln!("Keyring is NOT available on this system.");
            eprintln!();
            eprintln!("Possible causes:");

            #[cfg(target_os = "linux")]
            {
                eprintln!(
                    "  - No Secret Service daemon running (install gnome-keyring or kwallet)"
                );
                eprintln!("  - D-Bus session not available");
                eprintln!("  - Running in a container without keyring access");
            }

            #[cfg(target_os = "macos")]
            eprintln!("  - Keychain access denied");

            #[cfg(target_os = "windows")]
            eprintln!("  - Credential Manager service not running");

            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===================
    // SecretStore Tests
    // ===================

    #[test]
    fn test_secret_store_new() {
        let store = SecretStore::new();
        assert_eq!(store.service, SERVICE_NAME);
    }

    #[test]
    fn test_secret_store_default() {
        let store = SecretStore::default();
        assert_eq!(store.service, SERVICE_NAME);
    }

    #[test]
    fn test_is_available() {
        // Just verify it doesn't panic - result depends on system
        let _ = SecretStore::is_available();
    }

    #[test]
    fn test_list_returns_empty() {
        let store = SecretStore::new();
        let list = store.list();
        assert!(list.is_empty());
    }

    // ===================
    // resolve_secret Tests
    // ===================

    #[test]
    fn test_resolve_secret_plaintext() {
        let store = SecretStore::new();
        let result = resolve_secret("plain-value", &store);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "plain-value");
    }

    #[test]
    fn test_resolve_secret_keyring_prefix() {
        let store = SecretStore::new();
        // This will fail because the secret doesn't exist, but it tests the prefix detection
        let result = resolve_secret("keyring:nonexistent-test-key", &store);
        assert!(result.is_err());
        match result {
            Err(SecretError::NotFound(name)) => assert_eq!(name, "nonexistent-test-key"),
            Err(SecretError::Keyring(_)) => {} // Keyring might not be available
            _ => panic!("Expected NotFound or Keyring error"),
        }
    }

    // ===================
    // SecretError Tests
    // ===================

    #[test]
    fn test_secret_error_display() {
        let err = SecretError::NotFound("test-key".to_string());
        assert_eq!(format!("{}", err), "Secret 'test-key' not found");

        let err = SecretError::Keyring("connection failed".to_string());
        assert_eq!(format!("{}", err), "Keyring error: connection failed");
    }

    // ===================
    // Integration Tests (require keyring access)
    // ===================

    #[test]
    #[ignore] // Run manually: cargo test test_keyring_roundtrip -- --ignored
    fn test_keyring_roundtrip() {
        let store = SecretStore::new();
        let test_name = "openhush-test-secret";
        let test_value = "test-secret-value-12345";

        // Clean up any existing test secret
        let _ = store.delete(test_name);

        // Set
        store
            .set(test_name, test_value)
            .expect("Failed to set secret");

        // Get
        let retrieved = store.get(test_name).expect("Failed to get secret");
        assert_eq!(retrieved, test_value);

        // Delete
        store.delete(test_name).expect("Failed to delete secret");

        // Verify deleted
        let result = store.get(test_name);
        assert!(matches!(result, Err(SecretError::NotFound(_))));
    }

    #[test]
    #[ignore] // Run with: cargo test test_keyring_overwrite -- --ignored
    fn test_keyring_overwrite() {
        let store = SecretStore::new();
        let test_name = "openhush-test-overwrite";

        // Clean up
        let _ = store.delete(test_name);

        // Set initial value
        store.set(test_name, "value1").expect("Failed to set");

        // Overwrite with new value
        store.set(test_name, "value2").expect("Failed to overwrite");

        // Should get the new value
        let retrieved = store.get(test_name).expect("Failed to get");
        assert_eq!(retrieved, "value2");

        // Cleanup
        let _ = store.delete(test_name);
    }

    #[test]
    #[ignore] // Run with: cargo test test_keyring_special_chars -- --ignored
    fn test_keyring_special_chars() {
        let store = SecretStore::new();
        let test_name = "openhush-test-special";
        let test_value = "p@ssw0rd!#$%^&*()_+-=[]{}|;':\",./<>?";

        // Clean up
        let _ = store.delete(test_name);

        // Set value with special characters
        store.set(test_name, test_value).expect("Failed to set");

        // Retrieve and verify
        let retrieved = store.get(test_name).expect("Failed to get");
        assert_eq!(retrieved, test_value);

        // Cleanup
        let _ = store.delete(test_name);
    }

    #[test]
    #[ignore] // Run with: cargo test test_keyring_unicode -- --ignored
    fn test_keyring_unicode() {
        let store = SecretStore::new();
        let test_name = "openhush-test-unicode";
        let test_value = "密码🔐пароль🔑";

        // Clean up
        let _ = store.delete(test_name);

        // Set unicode value
        store.set(test_name, test_value).expect("Failed to set");

        // Retrieve and verify
        let retrieved = store.get(test_name).expect("Failed to get");
        assert_eq!(retrieved, test_value);

        // Cleanup
        let _ = store.delete(test_name);
    }

    #[test]
    fn test_delete_nonexistent() {
        let store = SecretStore::new();
        // Deleting a non-existent secret should fail
        let result = store.delete("openhush-definitely-does-not-exist-12345");
        // May fail with NotFound or succeed silently depending on backend
        // Just verify it doesn't panic
        let _ = result;
    }

    #[test]
    fn test_get_nonexistent() {
        let store = SecretStore::new();
        let result = store.get("openhush-definitely-does-not-exist-12345");
        assert!(result.is_err());
    }

    // ===================
    // resolve_secret Edge Cases
    // ===================

    #[test]
    fn test_resolve_secret_empty_string() {
        let store = SecretStore::new();
        let result = resolve_secret("", &store);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    fn test_resolve_secret_keyring_empty_name() {
        let store = SecretStore::new();
        let result = resolve_secret("keyring:", &store);
        // Should try to look up empty name, which will fail
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_secret_not_keyring_prefix() {
        let store = SecretStore::new();
        // "keyring" without colon should be treated as plaintext
        let result = resolve_secret("keyring", &store);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "keyring");
    }

    #[test]
    fn test_resolve_secret_with_colon_in_value() {
        let store = SecretStore::new();
        // Value with colon but not keyring: prefix
        let result = resolve_secret("api:key:value", &store);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "api:key:value");
    }
}
