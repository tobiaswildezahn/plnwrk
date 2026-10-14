//! Opening and saving workspace files from the command line, including the passphrase.

use std::io::IsTerminal as _;
use std::path::Path;

use plnwrk_engine::Session;
use plnwrk_store::{Document, Manifest, Protection, SecretString, StoreError, file};
use serde_json::{Value, json};

use crate::system::{RandomIds, SystemClock};

/// Environment variable with the passphrase (for scripts and agents).
pub const PASSPHRASE_VAR: &str = "PLNWRK_PASSPHRASE";

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

/// The passphrase from the environment, or asked interactively (twice when `confirm`).
pub fn passphrase(confirm: bool) -> Result<SecretString, Failure> {
    if let Ok(value) = std::env::var(PASSPHRASE_VAR) {
        if value.is_empty() {
            return Err(Failure::message(format!("{PASSPHRASE_VAR} is empty")));
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
    if confirm {
        let second = rpassword::prompt_password("Passphrase (again): ")
            .map_err(|e| Failure::message(format!("cannot read the passphrase: {e}")))?;
        if first != second {
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

/// Reads and verifies a file, asking for the passphrase if it is encrypted.
pub fn open(path: &Path) -> Result<Open, Failure> {
    let bytes = file::read(path)?;
    let manifest = plnwrk_store::peek(&bytes)?;
    let (doc, protection) = if manifest.encrypted {
        let secret = passphrase(false)?;
        let doc = plnwrk_store::load(&bytes, Some(&secret))?;
        (doc, Protection::Passphrase(secret))
    } else {
        (plnwrk_store::load(&bytes, None)?, Protection::None)
    };
    Ok(Open {
        doc,
        protection,
        manifest,
    })
}

/// Writes a document atomically.
pub fn save(path: &Path, doc: &Document, protection: &Protection) -> Result<(), Failure> {
    let bytes = plnwrk_store::save(doc, protection)?;
    file::write_atomic(path, &bytes)?;
    Ok(())
}

/// A session on a loaded document, with the real clock and real ids.
pub fn session(doc: Document) -> Session {
    Session::with_workspace(
        doc.workspace,
        doc.log,
        Box::new(SystemClock),
        Box::new(RandomIds::default()),
    )
}

/// The document of a session.
pub fn document(session: &Session) -> Document {
    Document {
        workspace: session.workspace().clone(),
        log: session.log().to_vec(),
    }
}
