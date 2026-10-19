//! Why a file could not be written or read. Every failure of a hostile or damaged file ends here,
//! never in a panic.

use plnwrk_core::Violation;
use serde::Serialize;

/// Why saving or loading failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum StoreError {
    /// Not a plnwrk file (no ZIP, or no plnwrk manifest).
    #[error("this is not a plnwrk file")]
    NotPlnwrk,
    /// Written by a newer plnwrk.
    #[error(
        "the file has format version {found}; this plnwrk reads up to {supported} – please update"
    )]
    NewerFormat {
        /// Version in the file.
        found: u32,
        /// Newest version this build reads.
        supported: u32,
    },
    /// A version this build cannot read.
    #[error("format version {found} is not supported")]
    UnsupportedFormat {
        /// Version in the file.
        found: u32,
    },
    /// The file is damaged or inconsistent.
    #[error("the file is damaged: {reason}")]
    Corrupt {
        /// What is wrong.
        reason: String,
    },
    /// The file exceeds the size limits.
    #[error("the file is larger than allowed ({limit} bytes or entries)")]
    TooLarge {
        /// The limit that was hit.
        limit: u64,
    },
    /// The file is encrypted and no passphrase was given.
    #[error("the file is encrypted; a passphrase is required")]
    PassphraseRequired,
    /// The passphrase does not open the file.
    #[error("the passphrase is wrong")]
    WrongPassphrase,
    /// Classified content (VS-NfD) would be or is stored without encryption.
    #[error("classified content (VS-NfD) must be stored encrypted")]
    ClassifiedUnencrypted,
    /// The change log was altered: a change does not match its hash or its place.
    #[error("the change log was altered at change {seq}")]
    ChainBroken {
        /// The first change that does not fit.
        seq: u64,
    },
    /// The stored state differs from what the change log produces.
    #[error("the stored state does not match the change log")]
    SnapshotMismatch,
    /// The stored data breaks rules of the model.
    #[error("the stored data breaks {} rule(s)", violations.len())]
    Invalid {
        /// Every broken rule.
        violations: Vec<Violation>,
    },
    /// Encryption or decryption could not run.
    #[error("encryption failed: {reason}")]
    Encryption {
        /// Details.
        reason: String,
    },
    /// Another plnwrk program (the app, the command line, the MCP server) has the file open.
    #[error("{path} is in use by another plnwrk")]
    InUse {
        /// The file.
        path: String,
    },
    /// The file changed on disk while it was being saved (another program wrote it although it
    /// should have waited for the lock – e.g. on a network drive that ignores locks). Nothing
    /// was written; saving again merges what is there now.
    #[error("{path} changed while saving")]
    ChangedMeanwhile {
        /// The file.
        path: String,
    },
    /// Reading or writing the file on disk failed.
    #[error("file access failed: {reason}")]
    Io {
        /// Details.
        reason: String,
    },
}

impl StoreError {
    pub(crate) fn corrupt(reason: impl Into<String>) -> Self {
        Self::Corrupt {
            reason: reason.into(),
        }
    }
}
