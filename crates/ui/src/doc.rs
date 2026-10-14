//! The open workspace: its session, where it lives on disk and whether it is saved.

use std::path::{Path, PathBuf};

use plnwrk_engine::{CommandError, Session, fixture};
use plnwrk_platform::{RandomIds, SystemClock};
use plnwrk_store::{Document, Protection, SecretString, StoreError};

use crate::text;

/// One open workspace.
pub struct OpenDoc {
    /// Data, history, undo.
    pub session: Session,
    /// The file, once saved or opened.
    path: Option<PathBuf>,
    /// How the file is protected; kept when saving.
    protection: Protection,
    /// Length of the log when it was last saved or opened.
    saved: usize,
    /// Keeps other plnwrk processes off the file while it is open.
    #[cfg(not(target_arch = "wasm32"))]
    _lock: Option<plnwrk_store::file::Lock>,
}

fn session(doc: Document) -> Session {
    doc.into_session(Box::new(SystemClock), Box::new(RandomIds::default()))
}

impl OpenDoc {
    fn with(session: Session, path: Option<PathBuf>, protection: Protection, saved: usize) -> Self {
        Self {
            session,
            path,
            protection,
            saved,
            #[cfg(not(target_arch = "wasm32"))]
            _lock: None,
        }
    }

    /// An empty workspace that has never been saved.
    pub fn new() -> Self {
        Self::with(session(Document::default()), None, Protection::None, 0)
    }

    /// The sample workspace, not saved yet.
    pub fn sample() -> Result<Self, CommandError> {
        let mut session = session(Document::default());
        fixture::sample(&mut session)?;
        Ok(Self::with(session, None, Protection::None, 0))
    }

    /// A workspace read from the bytes of a file, e.g. an upload in the browser.
    pub fn from_bytes(bytes: &[u8], passphrase: Option<SecretString>) -> Result<Self, StoreError> {
        let opened = plnwrk_store::open(bytes, passphrase.as_ref())?;
        let protection = match passphrase {
            Some(secret) if opened.manifest.encrypted => Protection::Passphrase(secret),
            _ => Protection::None,
        };
        let saved = opened.document.log.len();
        Ok(Self::with(
            session(opened.document),
            None,
            protection,
            saved,
        ))
    }

    /// The file it lives in.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Whether the file is encrypted.
    pub fn encrypted(&self) -> bool {
        matches!(self.protection, Protection::Passphrase(_))
    }

    /// Whether there are changes since it was last saved or opened.
    pub fn is_dirty(&self) -> bool {
        self.session.log().len() != self.saved
    }

    /// The name to show: the file name without extension, or "Unbenannt".
    pub fn name(&self) -> String {
        self.path.as_deref().and_then(Path::file_stem).map_or_else(
            || text::UNTITLED.to_owned(),
            |s| s.to_string_lossy().into_owned(),
        )
    }

    /// The bytes of the file as it would be saved now.
    pub fn to_bytes(&self) -> Result<Vec<u8>, StoreError> {
        plnwrk_store::save(&Document::from_session(&self.session), &self.protection)
    }
}

impl Default for OpenDoc {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl OpenDoc {
    /// Opens a file and locks it against other plnwrk processes. An encrypted file without
    /// passphrase gives [`StoreError::PassphraseRequired`].
    pub fn open(path: &Path, passphrase: Option<SecretString>) -> Result<Self, StoreError> {
        use plnwrk_store::file;
        let path = file::resolve(path)?;
        let lock = file::lock(&path)?;
        let bytes = file::read(&path)?;
        if plnwrk_store::peek(&bytes)?.encrypted && passphrase.is_none() {
            return Err(StoreError::PassphraseRequired);
        }
        let mut doc = Self::from_bytes(&bytes, passphrase)?;
        doc.path = Some(path);
        doc._lock = Some(lock);
        Ok(doc)
    }

    /// Saves to its file. Fails without a file (use [`Self::save_as`]).
    pub fn save(&mut self) -> Result<(), StoreError> {
        let Some(path) = self.path.clone() else {
            return Err(StoreError::Io {
                reason: "no file chosen yet".to_owned(),
            });
        };
        let bytes = self.to_bytes()?;
        plnwrk_store::file::write_atomic(&path, &bytes)?;
        self.saved = self.session.log().len();
        Ok(())
    }

    /// Saves to a new place (replacing a file there) and keeps working on it.
    pub fn save_as(&mut self, path: &Path) -> Result<(), StoreError> {
        use plnwrk_store::file;
        let path = resolve_target(&with_extension(path))?;
        if self.path.as_deref() == Some(path.as_path()) {
            return self.save();
        }
        let lock = file::lock(&path)?;
        let bytes = self.to_bytes()?;
        file::write_atomic(&path, &bytes)?;
        self.path = Some(path);
        self._lock = Some(lock);
        self.saved = self.session.log().len();
        Ok(())
    }
}

/// The real place of a file that may not exist yet: links and `..` of an existing file or of
/// its folder are resolved, so locks and comparisons see one path per file.
#[cfg(not(target_arch = "wasm32"))]
fn resolve_target(path: &Path) -> Result<PathBuf, StoreError> {
    use plnwrk_store::file;
    if path.exists() {
        return file::resolve(path);
    }
    let name = path.file_name().ok_or_else(|| StoreError::Io {
        reason: format!("{} is not a file name", path.display()),
    })?;
    let folder = match path.parent() {
        Some(folder) if !folder.as_os_str().is_empty() => folder,
        _ => Path::new("."),
    };
    Ok(file::resolve(folder)?.join(name))
}

/// Adds `.plnwrk` if the name has no extension.
pub fn with_extension(path: &Path) -> PathBuf {
    if path.extension().is_some() {
        path.to_path_buf()
    } else {
        path.with_extension("plnwrk")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plnwrk_engine::Origin;
    use serde_json::json;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("plnwrk-ui-doc-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn add_person(doc: &mut OpenDoc, name: &str) {
        doc.session
            .run("person.create", &json!({"name": name}), Origin::User)
            .unwrap();
    }

    #[test]
    fn a_new_workspace_is_clean_until_changed() {
        let mut doc = OpenDoc::new();
        assert!(!doc.is_dirty());
        assert_eq!(doc.name(), text::UNTITLED);
        add_person(&mut doc, "Anna");
        assert!(doc.is_dirty());
    }

    #[test]
    fn the_sample_counts_as_unsaved() {
        let doc = OpenDoc::sample().unwrap();
        assert!(doc.is_dirty());
        assert!(doc.session.workspace().projects().count() >= 2);
    }

    #[test]
    fn save_as_then_open_gives_the_same_data() {
        let dir = scratch("roundtrip");
        let mut doc = OpenDoc::sample().unwrap();
        assert!(doc.save().is_err(), "no file yet");
        doc.save_as(&dir.join("abteilung")).unwrap();
        assert!(!doc.is_dirty());
        assert_eq!(doc.name(), "abteilung");
        let path = doc.path().unwrap().to_path_buf();
        assert_eq!(path.extension().unwrap(), "plnwrk");

        // Open while still open here: the lock refuses it.
        let busy = OpenDoc::open(&path, None).err().unwrap();
        assert!(matches!(busy, StoreError::Io { .. }), "{busy:?}");

        let workspace = doc.session.workspace().clone();
        drop(doc);
        let mut again = OpenDoc::open(&path, None).unwrap();
        assert_eq!(again.session.workspace(), &workspace);
        assert!(!again.is_dirty());
        add_person(&mut again, "Neu");
        again.save().unwrap();
        assert!(!again.is_dirty());
        drop(again);
        let third = OpenDoc::open(&path, None).unwrap();
        assert!(third.session.workspace().people().any(|p| p.name == "Neu"));
    }

    #[test]
    fn encrypted_files_need_the_passphrase_and_stay_encrypted() {
        let dir = scratch("encrypted");
        let path = dir.join("geheim.plnwrk");
        let secret = SecretString::from("richtig-lange-passphrase".to_owned());
        let doc = Document::from_session(&OpenDoc::sample().unwrap().session);
        let bytes = plnwrk_store::save_with(
            &doc,
            &Protection::Passphrase(secret.clone()),
            plnwrk_store::SaveOptions {
                work_factor: Some(10),
            },
        )
        .unwrap();
        std::fs::write(&path, bytes).unwrap();

        assert_eq!(
            OpenDoc::open(&path, None).err(),
            Some(StoreError::PassphraseRequired)
        );
        let wrong = SecretString::from("falsche-passphrase".to_owned());
        assert_eq!(
            OpenDoc::open(&path, Some(wrong)).err(),
            Some(StoreError::WrongPassphrase)
        );
        let opened = OpenDoc::open(&path, Some(secret)).unwrap();
        assert!(opened.encrypted());
        assert!(
            plnwrk_store::peek(&opened.to_bytes().unwrap())
                .unwrap()
                .encrypted
        );
    }

    #[test]
    fn a_broken_file_is_refused() {
        let dir = scratch("broken");
        let path = dir.join("kaputt.plnwrk");
        std::fs::write(&path, b"kein zip").unwrap();
        assert_eq!(
            OpenDoc::open(&path, None).err(),
            Some(StoreError::NotPlnwrk)
        );
        assert!(OpenDoc::open(&dir.join("fehlt.plnwrk"), None).is_err());
    }

    #[test]
    fn names_get_the_extension() {
        assert_eq!(
            with_extension(Path::new("a/b")),
            PathBuf::from("a/b.plnwrk")
        );
        assert_eq!(
            with_extension(Path::new("a/b.plnwrk")),
            PathBuf::from("a/b.plnwrk")
        );
    }
}
