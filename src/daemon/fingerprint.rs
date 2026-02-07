//! Configuration fingerprinting module for change detection.
//!
//! This module provides functionality to calculate and compare configuration
//! fingerprints, enabling the daemon to detect configuration changes and
//! trigger appropriate actions.
//!
//! The fingerprint is based on SHA256 hash of the config file content,
//! ensuring any change to the configuration will result in a different hash.

use anyhow::Result;
use std::path::Path;
use std::time::SystemTime;

/// Configuration fingerprint structure
///
/// Contains the SHA256 hash of config content and modification time
/// for robust change detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigFingerprint {
    /// SHA256 hash of config content
    pub hash: String,
    /// Modification time of config file
    pub mtime: SystemTime,
}

impl ConfigFingerprint {
    /// Create a fingerprint from config content directly
    ///
    /// Useful for CLI to calculate its own fingerprint without reading the file.
    pub fn from_config_content(content: &str) -> Result<Self> {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let result = hasher.finalize();

        Ok(ConfigFingerprint {
            hash: hex::encode(result),
            mtime: SystemTime::now(),
        })
    }

    /// Create a fingerprint from a config file path
    ///
    /// Reads the config file, calculates its SHA256 hash, and extracts
    /// the modification time.
    pub fn from_config_file(config_path: &Path) -> Result<Self> {
        // Read config file content
        let content = std::fs::read_to_string(config_path)?;
        let content = content.trim();

        // Calculate SHA256 hash
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let result = hasher.finalize();

        // Get file modification time
        let metadata = std::fs::metadata(config_path)?;
        let mtime = metadata.modified()?;

        Ok(ConfigFingerprint {
            hash: hex::encode(result),
            mtime,
        })
    }

    /// Compare two fingerprints for equality
    ///
    /// Returns true if both hash and mtime match, indicating identical config.
    pub fn is_same(&self, other: &Self) -> bool {
        self.hash == other.hash && self.mtime == other.mtime
    }

    /// Check if this fingerprint differs from another
    ///
    /// Returns true if either hash or mtime differs, indicating config change.
    pub fn is_different(&self, other: &Self) -> bool {
        !self.is_same(other)
    }
}

/// Calculate config fingerprint from a Config struct
///
/// Convenience function that serializes the config to JSON and
/// calculates the SHA256 hash.
pub fn calculate_fingerprint(config: &crate::config::Config) -> String {
    use sha2::{Digest, Sha256};

    // Serialize config to JSON
    let json = serde_json::to_string(config).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(json.as_bytes());
    let result = hasher.finalize();

    // Convert to hex string
    hex::encode(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fingerprint_from_content() {
        let fp = ConfigFingerprint::from_config_content("test content").unwrap();
        assert!(!fp.hash.is_empty());
    }

    #[test]
    fn test_fingerprint_from_file() {
        // Create a temporary config file
        let temp_file = std::tempdir().unwrap().path().join("test.toml");
        std::fs::write(&temp_file, "test = \"value\"").unwrap();
        std::fs::set_last_modified(&temp_file, std::time::SystemTime::UNIX_EPOCH).unwrap();

        let fp = ConfigFingerprint::from_config_file(&temp_file).unwrap();
        assert!(!fp.hash.is_empty());
        assert_eq!(fp.mtime, std::time::UNIX_EPOCH);
    }

    #[test]
    fn test_fingerprint_difference() {
        let fp1 = ConfigFingerprint::from_config_content("same").unwrap();
        let fp2 = ConfigFingerprint::from_config_content("same").unwrap();
        assert!(fp1.is_same(&fp2));
        assert!(!fp1.is_different(&fp2));

        let fp3 = ConfigFingerprint::from_config_content("different").unwrap();
        assert!(fp1.is_different(&fp3));
        assert!(!fp1.is_same(&fp3));
    }
}
