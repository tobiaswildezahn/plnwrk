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
    /// Data of an older format was brought up to date when it was opened (not saved yet).
    pub upgraded: bool,
    /// The name to show without a file (e.g. a recovered copy).
    title: Option<String>,
    /// The protection was changed and not saved yet.
    protection_changed: bool,
    /// Tells the recovery copy of this workspace apart from others (when it was opened).
    recovery_key: String,
    /// Keeps other plnwrk processes off the file while it is open.
    #[cfg(not(target_arch = "wasm32"))]
    _lock: Option<plnwrk_store::file::Lock>,
}

/// A key unique to this opening: date and time, and a counter for several in one second.
fn recovery_key() -> String {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let stamp = jiff::Zoned::now().strftime("%Y-%m-%d %H-%M-%S");
    // Keeps two plnwrk windows (or browser tabs) opened in the same second apart.
    format!("{stamp} {}-{n}", plnwrk_platform::instance())
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
            upgraded: false,
            title: None,
            protection_changed: false,
            recovery_key: recovery_key(),
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
        let (session, upgraded) = opened
            .document
            .open_session(Box::new(SystemClock), Box::new(RandomIds::default()))
            .map_err(|e| StoreError::Corrupt {
                reason: format!("{} {}", text::t().upgrade_failed, text::command_error(&e)),
            })?;
        // A conversion is a change in the log: the file counts as changed until saved.
        let mut doc = Self::with(session, None, protection, saved);
        doc.upgraded = upgraded;
        Ok(doc)
    }

    /// A file uploaded in the browser: named like the file, without a path. An encrypted file
    /// without passphrase gives [`StoreError::PassphraseRequired`].
    pub fn from_upload(
        name: &str,
        bytes: &[u8],
        passphrase: Option<SecretString>,
    ) -> Result<Self, StoreError> {
        if plnwrk_store::peek(bytes)?.encrypted && passphrase.is_none() {
            return Err(StoreError::PassphraseRequired);
        }
        let mut doc = Self::from_bytes(bytes, passphrase)?;
        doc.title = Some(stem(name));
        Ok(doc)
    }

    /// Treats it as work not saved anywhere (a recovered copy).
    pub fn mark_unsaved(&mut self) {
        self.saved = usize::MAX;
    }

    /// Tells the recovery copy of this opening apart from others.
    pub fn recovery_key(&self) -> &str {
        &self.recovery_key
    }

    /// The file was saved somewhere else (downloaded in the browser): nothing is unsaved now.
    pub fn mark_saved(&mut self) {
        self.saved = self.session.log().len();
        self.protection_changed = false;
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
        self.session.log().len() != self.saved || self.protection_changed
    }

    /// Protects the file with a passphrase from the next save on (`None`: no protection).
    /// Returns the protection it had, to put back if saving fails.
    pub fn set_protection(&mut self, protection: Protection) -> Protection {
        self.protection_changed = true;
        std::mem::replace(&mut self.protection, protection)
    }

    /// Puts back a protection after saving with a new one failed.
    pub fn restore_protection(&mut self, protection: Protection) {
        self.protection = protection;
        self.protection_changed = false;
    }

    /// The name to show: the file name without extension, or "Unbenannt".
    pub fn name(&self) -> String {
        match self.path.as_deref().and_then(Path::file_stem) {
            Some(stem) => stem.to_string_lossy().into_owned(),
            None => self
                .title
                .clone()
                .unwrap_or_else(|| text::t().untitled.to_owned()),
        }
    }

    /// How many changes the log holds (a cheap fingerprint of the state).
    pub fn log_len(&self) -> usize {
        self.session.log().len()
    }

    /// The data and protection as they would be saved now, for writing elsewhere (e.g. in the
    /// background).
    pub fn snapshot(&self) -> (Document, Protection) {
        (
            Document::from_session(&self.session),
            self.protection.clone(),
        )
    }

    /// The bytes of the file as it would be saved now.
    pub fn to_bytes(&self) -> Result<Vec<u8>, StoreError> {
        plnwrk_store::save(&Document::from_session(&self.session), &self.protection)
    }
}

/// A file name without folder and `.plnwrk` (or any other extension).
pub fn stem(name: &str) -> String {
    let file = Path::new(name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    if file.trim().is_empty() {
        text::t().untitled.to_owned()
    } else {
        file
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
                reason: text::t().no_file_chosen.to_owned(),
            });
        };
        let bytes = self.to_bytes()?;
        plnwrk_store::file::write_atomic(&path, &bytes)?;
        self.saved = self.session.log().len();
        self.protection_changed = false;
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
        self.protection_changed = false;
        Ok(())
    }
}

/// Where recovery copies go: `~/Library/Application Support/plnwrk/Notkopien` on macOS,
/// `%APPDATA%/plnwrk/Notkopien` on Windows, `~/.local/share/plnwrk/Notkopien` elsewhere.
#[cfg(not(target_arch = "wasm32"))]
pub fn recovery_folder() -> Option<PathBuf> {
    let base = if cfg!(target_os = "macos") {
        PathBuf::from(std::env::var_os("HOME")?)
            .join("Library")
            .join("Application Support")
    } else if cfg!(windows) {
        PathBuf::from(std::env::var_os("APPDATA")?)
    } else {
        std::env::var_os("XDG_DATA_HOME").map_or_else(
            || std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share")),
            |d| Some(PathBuf::from(d)),
        )?
    };
    Some(base.join("plnwrk").join("Notkopien"))
}

/// The recovery copies in a folder, newest first.
#[cfg(not(target_arch = "wasm32"))]
pub fn recovery_copies(folder: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut copies: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "plnwrk"))
        .collect();
    // Newest first.
    copies.sort_by_cached_key(|p| {
        std::cmp::Reverse(std::fs::metadata(p).and_then(|m| m.modified()).ok())
    });
    copies
}

#[cfg(not(target_arch = "wasm32"))]
impl OpenDoc {
    /// Where the recovery copy of this workspace goes in `folder`: one fixed name per opening,
    /// so each autosave replaces the last.
    pub fn recovery_path(&self, folder: &Path) -> PathBuf {
        folder.join(format!(
            "{} – Wiederherstellung {}.plnwrk",
            self.name(),
            self.recovery_key
        ))
    }

    /// Treats an opened recovery copy as a new, unsaved workspace: it keeps its name, but saving
    /// asks for a place (never back into the recovery folder). Returns the copy's path.
    pub fn detach_as_untitled(&mut self) -> Option<PathBuf> {
        let path = self.path.take()?;
        let name = self.name_of(&path);
        self.title = Some(name);
        self._lock = None;
        self.saved = usize::MAX;
        Some(path)
    }

    fn name_of(&self, path: &Path) -> String {
        path.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .and_then(|stem| stem.split(" – Wiederherstellung").next().map(str::to_owned))
            .unwrap_or_else(|| text::t().untitled.to_owned())
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
        reason: format!("{} {}", text::t().not_a_file_name, path.display()),
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
        let dir = std::env::temp_dir().join(format!(
            "plnwrk-ui-doc-{}-{name}",
            plnwrk_platform::instance()
        ));
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
        assert_eq!(doc.name(), text::t().untitled);
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
