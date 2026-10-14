//! The manifest: the readable head of every file. Format, version, classification and whether
//! the content is encrypted – never titles or other content.

use plnwrk_core::Classification;
use serde::{Deserialize, Serialize};

use crate::StoreError;

/// Marks a plnwrk file.
pub const FORMAT: &str = "plnwrk";
/// The format version this build writes and reads.
pub const VERSION: u32 = 1;
/// The oldest version this build can read (older ones would be migrated).
pub const OLDEST: u32 = 1;

/// The head of a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Always [`FORMAT`].
    pub format: String,
    /// Format version.
    pub version: u32,
    /// Highest classification of the content.
    pub classification: Classification,
    /// Whether the content is encrypted (then it lies in `payload.age`).
    pub encrypted: bool,
    /// The program that wrote the file.
    pub created_with: String,
    /// Number of logged changes (only in the unencrypted part, never readable from outside an
    /// encrypted file).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changes: Option<u64>,
    /// Hash of the last logged change (same visibility as `changes`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
}

impl Manifest {
    /// Reads a manifest, checking format and version before anything else.
    pub fn parse(bytes: &[u8]) -> Result<Self, StoreError> {
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|_| StoreError::NotPlnwrk)?;
        if value.get("format").and_then(|f| f.as_str()) != Some(FORMAT) {
            return Err(StoreError::NotPlnwrk);
        }
        let version = value
            .get("version")
            .and_then(serde_json::Value::as_u64)
            .and_then(|v| u32::try_from(v).ok())
            .ok_or_else(|| StoreError::corrupt("manifest has no valid version"))?;
        if version > VERSION {
            return Err(StoreError::NewerFormat {
                found: version,
                supported: VERSION,
            });
        }
        if version < OLDEST {
            return Err(StoreError::UnsupportedFormat { found: version });
        }
        serde_json::from_value(value).map_err(|e| StoreError::corrupt(format!("manifest: {e}")))
    }

    /// The manifest as written.
    pub fn to_bytes(&self) -> Result<Vec<u8>, StoreError> {
        serde_json::to_vec_pretty(self).map_err(|e| StoreError::corrupt(format!("manifest: {e}")))
    }
}

/// The program name and version written into manifests.
pub fn created_with() -> String {
    format!("plnwrk {}", env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> Manifest {
        Manifest {
            format: FORMAT.into(),
            version: VERSION,
            classification: Classification::Open,
            encrypted: false,
            created_with: created_with(),
            changes: Some(3),
            head: Some("ab".into()),
        }
    }

    #[test]
    fn round_trip() {
        let m = manifest();
        assert_eq!(Manifest::parse(&m.to_bytes().unwrap()).unwrap(), m);
    }

    #[test]
    fn versions_are_checked_first() {
        let newer = br#"{"format":"plnwrk","version":2,"future":"field"}"#;
        assert_eq!(
            Manifest::parse(newer),
            Err(StoreError::NewerFormat {
                found: 2,
                supported: 1
            })
        );
        let older = br#"{"format":"plnwrk","version":0}"#;
        assert_eq!(
            Manifest::parse(older),
            Err(StoreError::UnsupportedFormat { found: 0 })
        );
    }

    #[test]
    fn foreign_or_broken_manifests_are_refused() {
        assert_eq!(Manifest::parse(b"{}"), Err(StoreError::NotPlnwrk));
        assert_eq!(Manifest::parse(b"nope"), Err(StoreError::NotPlnwrk));
        assert_eq!(
            Manifest::parse(br#"{"format":"other","version":1}"#),
            Err(StoreError::NotPlnwrk)
        );
        assert!(matches!(
            Manifest::parse(br#"{"format":"plnwrk","version":"1"}"#),
            Err(StoreError::Corrupt { .. })
        ));
        assert!(matches!(
            Manifest::parse(br#"{"format":"plnwrk","version":1,"classification":"open","encrypted":false,"created_with":"x","extra":1}"#),
            Err(StoreError::Corrupt { .. })
        ));
    }
}
