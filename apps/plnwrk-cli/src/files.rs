//! Opening and saving workspace files from the command line, including the passphrase.

use std::io::IsTerminal as _;
use std::path::Path;

use plnwrk_engine::Session;
use plnwrk_platform::{RandomIds, SystemClock};
use plnwrk_store::{Document, Manifest, Protection, SecretString, StoreError, file};
use serde_json::{Value, json};
use unicode_normalization::UnicodeNormalization as _;
use zeroize::Zeroize as _;

/// Environment variable with the passphrase (for scripts and agents).
pub const PASSPHRASE_VAR: &str = "PLNWRK_PASSPHRASE";

/// Shortest passphrase accepted when a file is protected.
pub const MIN_PASSPHRASE_CHARS: usize = 12;

/// A failure with a machine-readable form.
pub struct Failure(pub Value);

impl From<StoreError> for Failure {
    fn from(error: StoreError) -> Self {
        Self(serde_json::to_value(&error).unwrap_or_else(|_| json!({"error": error.to_string()})))
    }
}

impl From<plnwrk_engine::CommandError> for Failure {
    fn from(error: plnwrk_engine::CommandError) -> Self {
        Self(serde_json::to_value(&error).unwrap_or_else(|_| json!({"error": error.to_string()})))
    }
}

impl Failure {
    /// A failure with a plain message.
    pub fn message(text: impl Into<String>) -> Self {
        Self(json!({"error": "cli", "message": text.into()}))
    }
}

fn check_new(passphrase: &str) -> Result<(), Failure> {
    // Counted as the passphrase is used: after Unicode normalisation.
    if passphrase.nfc().count() < MIN_PASSPHRASE_CHARS {
        return Err(Failure::message(format!(
            "a new passphrase needs at least {MIN_PASSPHRASE_CHARS} characters"
        )));
    }
    Ok(())
}

/// The passphrase from the environment, or asked interactively. `new` = it protects a file
/// from now on: then it is asked twice and must be long enough.
pub fn passphrase(new: bool) -> Result<SecretString, Failure> {
    if let Ok(value) = std::env::var(PASSPHRASE_VAR) {
        if value.is_empty() {
            return Err(Failure::message(format!("{PASSPHRASE_VAR} is empty")));
        }
        if new {
            check_new(&value)?;
        }
        return Ok(SecretString::from(value));
    }
    if !std::io::stdin().is_terminal() {
        return Err(StoreError::PassphraseRequired.into());
    }
    let first = rpassword::prompt_password("Passphrase: ")
        .map_err(|e| Failure::message(format!("cannot read the passphrase: {e}")))?;
    if first.is_empty() {
        return Err(Failure::message("the passphrase must not be empty"));
    }
    if new {
        if let Err(failure) = check_new(&first) {
            let mut first = first;
            first.zeroize();
            return Err(failure);
        }
        let mut second = rpassword::prompt_password("Passphrase (again): ")
            .map_err(|e| Failure::message(format!("cannot read the passphrase: {e}")))?;
        let same = first == second;
        second.zeroize();
        if !same {
            let mut first = first;
            first.zeroize();
            return Err(Failure::message("the two passphrases differ"));
        }
    }
    Ok(SecretString::from(first))
}

/// An open file: its content and how it is protected.
pub struct Open {
    /// Data and history.
    pub doc: Document,
    /// Protection to use when saving it back.
    pub protection: Protection,
    /// Its head.
    pub manifest: Manifest,
}

/// Reads and verifies a file, asking for the passphrase if it is encrypted. `saving_plain`:
/// the caller will save it back as it is – then a passphrase given for an unencrypted file
/// earns a warning.
pub fn open(path: &Path, saving_plain: bool) -> Result<Open, Failure> {
    let bytes = file::read(path)?;
    let head = plnwrk_store::peek(&bytes)?;
    let (opened, protection) = if head.encrypted {
        let secret = passphrase(false)?;
        let opened = plnwrk_store::open(&bytes, Some(&secret))?;
        (opened, Protection::Passphrase(secret))
    } else {
        if saving_plain && std::env::var_os(PASSPHRASE_VAR).is_some() {
            eprintln!(
                "plnwrk-cli: warning: {} is not encrypted; {PASSPHRASE_VAR} is not used and changes are saved unencrypted (use `encrypt`)",
                path.display()
            );
        }
        (plnwrk_store::open(&bytes, None)?, Protection::None)
    };
    Ok(Open {
        doc: opened.document,
        protection,
        manifest: opened.manifest,
    })
}

/// Locks a file against other plnwrk processes, then opens it – for anything that changes it.
pub fn open_for_change(path: &Path, saving_plain: bool) -> Result<(Open, file::Lock), Failure> {
    let lock = file::lock(path)?;
    Ok((open(path, saving_plain)?, lock))
}

/// Writes a document atomically over an existing file.
pub fn save(path: &Path, doc: &Document, protection: &Protection) -> Result<(), Failure> {
    let bytes = plnwrk_store::save(doc, protection)?;
    file::write_atomic(path, &bytes)?;
    Ok(())
}

/// Writes a document to a new file; never replaces an existing one.
pub fn save_new(path: &Path, doc: &Document, protection: &Protection) -> Result<(), Failure> {
    let bytes = plnwrk_store::save(doc, protection)?;
    file::write_new(path, &bytes)?;
    Ok(())
}

/// A session on a loaded document, with the real clock and real ids.
pub fn session(doc: Document) -> Session {
    doc.into_session(Box::new(SystemClock), Box::new(RandomIds::default()))
}

/// The document of a session.
pub fn document(session: &Session) -> Document {
    Document::from_session(session)
}
