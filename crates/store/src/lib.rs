//! The `.plnwrk` file format of plnwrk (layer 2). See `docs/PLAN.md` §3.4.
//!
//! A file is always a ZIP with a readable `manifest.json`. Unencrypted, it also holds
//! `workspace.json` (the current state) and `log.jsonl` (every change, hash-chained).
//! Encrypted, those lie in an inner ZIP stored as `payload.age`. Loading replays the log from
//! nothing and requires the result to equal the stored state, so data and history cannot drift
//! apart unnoticed. This crate works on bytes only and runs in the browser; [`file`] adds atomic
//! writing on desktop systems.

pub mod chain;
mod container;
mod crypto;
mod error;
pub mod export;
pub mod manifest;
mod replay;

#[cfg(not(target_arch = "wasm32"))]
pub mod file;

use std::collections::BTreeMap;

pub use age::secrecy::SecretString;
use plnwrk_core::{Classification, Workspace};
use plnwrk_engine::Change;

pub use container::Limits;
use container::Packing;
pub use crypto::{MAX_READ_WORK_FACTOR, WORK_FACTOR, WORK_FACTOR_RANGE};
pub use error::StoreError;
pub use manifest::Manifest;
pub use replay::replay;

const MANIFEST: &str = "manifest.json";
const WORKSPACE: &str = "workspace.json";
const LOG: &str = "log.jsonl";
const PAYLOAD: &str = "payload.age";

/// What a file holds: the data and its full history.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Document {
    /// The current state.
    pub workspace: Workspace,
    /// Every change that led to it, oldest first.
    pub log: Vec<Change>,
}

/// How a file is protected.
#[derive(Clone)]
pub enum Protection {
    /// Not encrypted. Refused for classified content.
    None,
    /// Encrypted with this passphrase.
    Passphrase(SecretString),
}

/// Tuning for [`save_with`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SaveOptions {
    /// scrypt work factor; `None` means [`WORK_FACTOR`]. Lower values only for tests.
    pub work_factor: Option<u8>,
}

/// A loaded file: its head and its content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opened {
    /// The verified head. For encrypted files the inner one (with change count and hash),
    /// marked `encrypted`.
    pub manifest: Manifest,
    /// Data and history.
    pub document: Document,
}

/// Largest manifest accepted.
const MANIFEST_LIMIT: u64 = 64 * 1024;

const PLAIN_ENTRIES: [&str; 3] = [LOG, MANIFEST, WORKSPACE];
const ENCRYPTED_ENTRIES: [&str; 2] = [MANIFEST, PAYLOAD];

/// The classification of a file: the highest level anywhere in the current data **or anywhere
/// in its history**. The change log keeps every earlier value, so content that was once VS-NfD
/// stays in the file even after the task is deleted or the project is declassified – like a
/// paper file, a plnwrk file is never classified lower again. Lowering it needs a deliberate,
/// history-dropping export (docs/PLAN.md Phase 7).
pub fn classification_of(doc: &Document) -> Classification {
    fn level(entity: &plnwrk_core::Entity) -> Classification {
        match entity {
            plnwrk_core::Entity::Project(p) => p.classification,
            plnwrk_core::Entity::Task(t) => t.classification.unwrap_or_default(),
            _ => Classification::Open,
        }
    }
    let history = doc
        .log
        .iter()
        .flat_map(|change| &change.events)
        .flat_map(|event| {
            let (before, after) = match event {
                plnwrk_engine::Event::Put { before, after } => (before.as_deref(), Some(&**after)),
                plnwrk_engine::Event::Remove { before } => (Some(&**before), None),
            };
            before.into_iter().chain(after).map(level)
        });
    history
        .chain(std::iter::once(doc.workspace.highest_classification()))
        .max()
        .unwrap_or_default()
}

/// Writes a document with default options.
pub fn save(doc: &Document, protection: &Protection) -> Result<Vec<u8>, StoreError> {
    save_with(doc, protection, SaveOptions::default())
}

/// Writes a document. Refuses broken data and classified content without encryption.
pub fn save_with(
    doc: &Document,
    protection: &Protection,
    options: SaveOptions,
) -> Result<Vec<u8>, StoreError> {
    let violations = doc.workspace.check();
    if !violations.is_empty() {
        return Err(StoreError::Invalid { violations });
    }
    let classification = classification_of(doc);
    if classification >= Classification::VsNfd && matches!(protection, Protection::None) {
        return Err(StoreError::ClassifiedUnencrypted);
    }
    let (log, head) = chain::seal(&doc.log)?;
    let workspace = serde_json::to_vec(&doc.workspace)
        .map_err(|e| StoreError::corrupt(format!("workspace: {e}")))?;
    let inner = Manifest {
        format: manifest::FORMAT.to_owned(),
        version: manifest::VERSION,
        classification,
        encrypted: false,
        created_with: manifest::created_with(),
        changes: Some(u64::try_from(doc.log.len()).unwrap_or(u64::MAX)),
        head,
    };
    let plain = container::write_zip(&[
        (MANIFEST, &inner.to_bytes()?, Packing::Compressed),
        (WORKSPACE, &workspace, Packing::Compressed),
        (LOG, log.as_bytes(), Packing::Compressed),
    ])?;
    match protection {
        Protection::None => Ok(plain),
        Protection::Passphrase(passphrase) => {
            let payload = crypto::encrypt(&plain, passphrase, options.work_factor)?;
            let outer = Manifest {
                encrypted: true,
                changes: None,
                head: None,
                ..inner
            };
            container::write_zip(&[
                (MANIFEST, &outer.to_bytes()?, Packing::Compressed),
                (PAYLOAD, &payload, Packing::Stored),
            ])
        }
    }
}

/// Reads only the readable head – works without the passphrase.
pub fn peek(bytes: &[u8]) -> Result<Manifest, StoreError> {
    Manifest::parse(&container::read_entry(bytes, MANIFEST, MANIFEST_LIMIT)?)
}

/// Reads and fully verifies a document with default limits.
pub fn load(bytes: &[u8], passphrase: Option<&SecretString>) -> Result<Document, StoreError> {
    open(bytes, passphrase).map(|opened| opened.document)
}

/// Reads and fully verifies a document.
pub fn load_with(
    bytes: &[u8],
    passphrase: Option<&SecretString>,
    limits: Limits,
) -> Result<Document, StoreError> {
    open_with(bytes, passphrase, limits).map(|opened| opened.document)
}

/// Reads and fully verifies a file with default limits, returning its verified head too.
pub fn open(bytes: &[u8], passphrase: Option<&SecretString>) -> Result<Opened, StoreError> {
    open_with(bytes, passphrase, Limits::default())
}

fn expect_entries(entries: &BTreeMap<String, Vec<u8>>, names: &[&str]) -> Result<(), StoreError> {
    let found: Vec<&str> = entries.keys().map(String::as_str).collect();
    if found == names {
        Ok(())
    } else {
        Err(StoreError::corrupt(format!(
            "unexpected entries {found:?}; expected {names:?}"
        )))
    }
}

/// Reads and fully verifies a file, returning its verified head too.
pub fn open_with(
    bytes: &[u8],
    passphrase: Option<&SecretString>,
    limits: Limits,
) -> Result<Opened, StoreError> {
    let entries = container::read_zip(bytes, limits)?;
    let manifest = Manifest::parse(entries.get(MANIFEST).ok_or(StoreError::NotPlnwrk)?)?;
    let (verified, document) = if manifest.encrypted {
        expect_entries(&entries, &ENCRYPTED_ENTRIES)?;
        if manifest.changes.is_some() || manifest.head.is_some() {
            return Err(StoreError::corrupt(
                "the readable head of an encrypted file must not describe its content",
            ));
        }
        let payload = entries
            .get(PAYLOAD)
            .ok_or_else(|| StoreError::corrupt("encrypted content is missing"))?;
        let passphrase = passphrase.ok_or(StoreError::PassphraseRequired)?;
        let plain = crypto::decrypt(payload, passphrase)?;
        let inner_entries = container::read_zip(&plain, limits)?;
        let inner = Manifest::parse(
            inner_entries
                .get(MANIFEST)
                .ok_or_else(|| StoreError::corrupt("encrypted content has no manifest"))?,
        )?;
        let matches = !inner.encrypted
            && inner.format == manifest.format
            && inner.version == manifest.version
            && inner.classification == manifest.classification
            && inner.created_with == manifest.created_with;
        if !matches {
            return Err(StoreError::corrupt(
                "the readable head does not match the encrypted content",
            ));
        }
        expect_entries(&inner_entries, &PLAIN_ENTRIES)?;
        let document = read_plain(&inner, &inner_entries)?;
        (
            Manifest {
                encrypted: true,
                ..inner
            },
            document,
        )
    } else {
        expect_entries(&entries, &PLAIN_ENTRIES)?;
        let document = read_plain(&manifest, &entries)?;
        (manifest, document)
    };
    let actual = classification_of(&document);
    if actual != verified.classification {
        return Err(StoreError::corrupt(
            "the classification in the head does not match the content and its history",
        ));
    }
    if actual >= Classification::VsNfd && !verified.encrypted {
        return Err(StoreError::ClassifiedUnencrypted);
    }
    Ok(Opened {
        manifest: verified,
        document,
    })
}

fn text(entries: &BTreeMap<String, Vec<u8>>, name: &str) -> Result<String, StoreError> {
    let bytes = entries
        .get(name)
        .ok_or_else(|| StoreError::corrupt(format!("{name} is missing")))?;
    String::from_utf8(bytes.clone()).map_err(|_| StoreError::corrupt(format!("{name} is not text")))
}

/// Every logged change must carry parameters its command accepts (no hidden extra keys).
fn check_params(log: &[Change]) -> Result<(), StoreError> {
    for change in log {
        let ok = match change.command.as_str() {
            "undo" | "redo" => change.params == serde_json::json!({}),
            command => plnwrk_engine::find(command).is_some_and(|spec| {
                plnwrk_engine::params::Params::check(&change.params, spec.params).is_ok()
            }),
        };
        if !ok {
            return Err(StoreError::corrupt(format!(
                "change {} has parameters its command does not take",
                change.seq
            )));
        }
    }
    Ok(())
}

fn read_plain(
    manifest: &Manifest,
    entries: &BTreeMap<String, Vec<u8>>,
) -> Result<Document, StoreError> {
    let stored = entries
        .get(WORKSPACE)
        .ok_or_else(|| StoreError::corrupt(format!("{WORKSPACE} is missing")))?;
    let log = chain::unseal(&text(entries, LOG)?, manifest.head.as_deref())?;
    if manifest.changes != Some(u64::try_from(log.len()).unwrap_or(u64::MAX)) {
        return Err(StoreError::corrupt(
            "number of changes does not match the head",
        ));
    }
    check_params(&log)?;
    // The stored state must be byte for byte what the log produces – so nothing can hide in it
    // (e.g. a duplicate key whose first value a JSON reader would silently drop).
    let workspace = replay(&log)?;
    let expected = serde_json::to_vec(&workspace)
        .map_err(|e| StoreError::corrupt(format!("workspace: {e}")))?;
    if expected != *stored {
        return Err(StoreError::SnapshotMismatch);
    }
    let violations = workspace.check();
    if !violations.is_empty() {
        return Err(StoreError::Invalid { violations });
    }
    Ok(Document { workspace, log })
}
