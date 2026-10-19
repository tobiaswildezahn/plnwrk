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
    /// How much of the log the file on disk holds too: everything after it is own work not yet
    /// in the file (Phase 10a – what saving merges onto the file).
    base: usize,
    /// How the file looked when it was last read or written here.
    #[cfg(not(target_arch = "wasm32"))]
    stamp: Option<plnwrk_store::file::Stamp>,
    /// Shared with others on this device (a person was chosen for it): saves itself and fetches
    /// what others saved (Phase 10a).
    pub shared: bool,
    /// When it was opened, an older plnwrk held the file (it locks a file while open).
    pub held_elsewhere: bool,
    /// The passphrase of the file as it lies on disk (the protection may be about to change):
    /// what reading it for a merge needs.
    file_secret: Option<SecretString>,
    /// The work as it was before a merge that could not take all of it (review 10a, F4): kept
    /// as a copy of its own, never deleted by the recovery copies' upkeep.
    lost_copy: Option<Document>,
}

/// How long saving waits for someone else's save to finish.
#[cfg(not(target_arch = "wasm32"))]
const LOCK_WAIT: std::time::Duration = std::time::Duration::from_millis(1500);

/// What the name of a copy of own work that a merge could not take carries (review 10a, F4).
#[cfg(not(target_arch = "wasm32"))]
const LOST_MARK: &str = " – nicht übernommen";

/// What saving or fetching took from the file: others' changes merged with the own.
pub type Merged = plnwrk_engine::sync::Rebased;

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
            base: saved,
            #[cfg(not(target_arch = "wasm32"))]
            stamp: None,
            shared: false,
            held_elsewhere: false,
            file_secret: None,
            lost_copy: None,
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
        let file_secret = match &protection {
            Protection::Passphrase(secret) => Some(secret.clone()),
            Protection::None => None,
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
        doc.file_secret = file_secret;
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

    /// Who this file is, whatever it is called or wherever it lies: the hash of its first change
    /// (Phase 10a – a person chosen for a shared file is remembered under it).
    pub fn identity(&self) -> Option<String> {
        let first = self.session.log().get(..1)?;
        plnwrk_store::chain::seal(first).ok()?.1
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
    /// Opens a file. It stays unlocked while open (Phase 10a: others may open it too); saving
    /// locks it for the moment of writing. An encrypted file without passphrase gives
    /// [`StoreError::PassphraseRequired`].
    pub fn open(path: &Path, passphrase: Option<SecretString>) -> Result<Self, StoreError> {
        use plnwrk_store::file;
        let path = file::resolve(path)?;
        let stamp = file::stamp(&path);
        let bytes = file::read(&path)?;
        if plnwrk_store::peek(&bytes)?.encrypted && passphrase.is_none() {
            return Err(StoreError::PassphraseRequired);
        }
        let mut doc = Self::from_bytes(&bytes, passphrase)?;
        doc.held_elsewhere = file::is_locked(&path);
        doc.path = Some(path);
        doc.stamp = stamp;
        Ok(doc)
    }

    /// Saves to its file, merging with what others saved there meanwhile; `merged` adds to the
    /// session what a merge calls for (comments on conflicts) before it is written. Fails without
    /// a file (use [`Self::save_as`]); never writes over a file it cannot read.
    pub fn save(
        &mut self,
        merged: impl FnOnce(&mut Session, &Merged),
    ) -> Result<Option<Merged>, StoreError> {
        let Some(path) = self.path.clone() else {
            return Err(StoreError::Io {
                reason: text::t().no_file_chosen.to_owned(),
            });
        };
        self.write_merged(&path, LOCK_WAIT, merged)
    }

    /// Like [`Self::save`], without waiting for someone else's save: a shared file saving itself
    /// tries again in a moment instead of holding up the window (review 10a, F5).
    pub fn save_now(
        &mut self,
        merged: impl FnOnce(&mut Session, &Merged),
    ) -> Result<Option<Merged>, StoreError> {
        let Some(path) = self.path.clone() else {
            return Err(StoreError::Io {
                reason: text::t().no_file_chosen.to_owned(),
            });
        };
        self.write_merged(&path, std::time::Duration::ZERO, merged)
    }

    /// The passphrase of the file on disk, given after it was protected by someone else
    /// meanwhile (review 10a, F7).
    pub fn use_file_passphrase(&mut self, secret: SecretString) {
        if !self.protection_changed {
            self.protection = Protection::Passphrase(secret.clone());
        }
        self.file_secret = Some(secret);
    }

    /// Locks `path`, reads it, merges own work since `base` onto it and writes the result –
    /// only if the file stayed as read (a writer that ignored the lock is noticed).
    fn write_merged(
        &mut self,
        path: &Path,
        wait: std::time::Duration,
        after_merge: impl FnOnce(&mut Session, &Merged),
    ) -> Result<Option<Merged>, StoreError> {
        use plnwrk_store::file;
        let _lock = file::lock_waiting(path, wait)?;
        let seen = file::stamp(path);
        let mut merged = None;
        // An encrypted file that did not change since it was last read or written here is not
        // read again: decrypting takes most of a second (review 10a, F6). Unencrypted files are
        // always read – cheap, and then not the stamp alone decides.
        let unchanged = self.file_secret.is_some() && seen.is_some() && seen == self.stamp;
        if seen.is_some() && !unchanged {
            let opened = self.open_file(path)?;
            self.follow(&opened);
            let theirs = opened.document.log.len();
            let same = theirs == self.base
                && opened.document.log.as_slice()
                    == self.session.log().get(..theirs).unwrap_or(&[]);
            if !same {
                let before = (self.session.log().len() > self.base)
                    .then(|| Document::from_session(&self.session));
                let rebased = self
                    .session
                    .rebase_onto(opened.document.workspace, opened.document.log, self.base)
                    .map_err(|_| StoreError::Io {
                        reason: text::t().shared_file_replaced.to_owned(),
                    })?;
                // From here the file's state is ours too, whether or not writing succeeds.
                self.base = theirs;
                // The earliest state is kept: it holds everything a later try lost too.
                if !rebased.lost.is_empty() && self.lost_copy.is_none() {
                    self.lost_copy = before;
                }
                after_merge(&mut self.session, &rebased);
                merged = Some(rebased);
            }
        }
        let bytes = self.to_bytes()?;
        file::replace_if_unchanged(path, &bytes, seen)?;
        self.written(path);
        Ok(merged)
    }

    /// The file at `path`, read and checked, opened with the passphrase of the file this
    /// workspace lives in. Changes nothing here.
    fn open_file(&self, path: &Path) -> Result<plnwrk_store::Opened, StoreError> {
        let bytes = plnwrk_store::file::read(path)?;
        plnwrk_store::open(&bytes, self.file_secret.as_ref())
    }

    /// Takes over the protection of the shared file (someone protected or unprotected it
    /// meanwhile) – unless it was changed here, then saving sets it visibly. Only for the file of
    /// the same work: never from another file (review 10a, F2).
    fn follow(&mut self, opened: &plnwrk_store::Opened) {
        if !self.protection_changed {
            self.protection = match &self.file_secret {
                Some(secret) if opened.manifest.encrypted => Protection::Passphrase(secret.clone()),
                _ => Protection::None,
            };
        }
    }

    /// Keeps the work as it was before a merge that could not take all of it (review 10a, F4):
    /// a new file in `folder`, with the protection here, offered under "Wiederherstellen". The
    /// path, if there was such work.
    pub fn keep_lost_copy(&mut self, folder: &Path) -> Result<Option<PathBuf>, StoreError> {
        let Some(before) = self.lost_copy.take() else {
            return Ok(None);
        };
        std::fs::create_dir_all(folder).map_err(|e| StoreError::Io {
            reason: e.to_string(),
        })?;
        let bytes = plnwrk_store::save(&before, &self.protection)?;
        let when = jiff::Zoned::now().strftime("%Y-%m-%d %H.%M.%S").to_string();
        let mut path = folder.join(format!("{}{LOST_MARK} {when}.plnwrk", self.name()));
        for n in 2.. {
            if !path.exists() {
                break;
            }
            path = folder.join(format!("{}{LOST_MARK} {when} ({n}).plnwrk", self.name()));
        }
        plnwrk_store::file::write_new(&path, &bytes)?;
        Ok(Some(path))
    }

    /// Takes what others saved in the file since it was last read or written here – only when
    /// nothing here is unsaved (the caller also checks typed input and open dialogs). `None`
    /// when the file did not change.
    pub fn fetch(&mut self) -> Result<Option<Merged>, StoreError> {
        let Some(path) = self.path.clone() else {
            return Ok(None);
        };
        if self.is_dirty() {
            return Ok(None);
        }
        let now = plnwrk_store::file::stamp(&path);
        if now.is_none() || now == self.stamp {
            return Ok(None);
        }
        let opened = self.open_file(&path)?;
        self.follow(&opened);
        let theirs = opened.document.log.len();
        // Replaced by another file entirely: said, not quietly taken over with a person it
        // does not have (review 10a, F8).
        let rebased = self
            .session
            .rebase_onto(opened.document.workspace, opened.document.log, self.base)
            .map_err(|_| StoreError::Io {
                reason: text::t().shared_file_replaced.to_owned(),
            })?;
        self.stamp = now;
        self.base = theirs;
        self.saved = theirs;
        Ok(Some(rebased))
    }

    /// Saves to a new place and keeps working on it. If a file of the same work is there (the
    /// shared file, saved into from a recovery copy), the own work is merged onto it instead of
    /// replacing what others saved (Phase 10a).
    pub fn save_as(
        &mut self,
        path: &Path,
        merged: impl FnOnce(&mut Session, &Merged),
    ) -> Result<Option<Merged>, StoreError> {
        let path = resolve_target(&with_extension(path))?;
        if self.path.as_deref() == Some(path.as_path()) {
            return self.save(merged);
        }
        // What is there decides (review 10a, F1–F3): the same work – merged onto it; another
        // work or nothing – written as it is, with the protection here (the system asked about
        // replacing); a file that cannot be read (protected, broken) – never written over.
        if !path.exists() {
            let result = self.write_fresh(&path).map(|()| None);
            if result.is_ok() {
                self.path = Some(path);
            }
            return result;
        }
        let opened = self.open_file(&path)?;
        let common = common_start(&opened.document.log, self.session.log());
        if common == 0 {
            self.write_fresh(&path)?;
            self.path = Some(path);
            return Ok(None);
        }
        let before = self.base;
        self.base = common;
        match self.write_merged(&path, LOCK_WAIT, merged) {
            Ok(merged) => {
                self.path = Some(path);
                Ok(merged)
            }
            Err(error) => {
                self.base = before;
                Err(error)
            }
        }
    }

    /// Writes the whole work to `path`, under the lock, replacing what is there.
    fn write_fresh(&mut self, path: &Path) -> Result<(), StoreError> {
        use plnwrk_store::file;
        let _lock = file::lock_waiting(path, LOCK_WAIT)?;
        let bytes = self.to_bytes()?;
        file::write_atomic(path, &bytes)?;
        self.written(path);
        Ok(())
    }

    /// The file at `path` now holds everything here, with the protection here.
    fn written(&mut self, path: &Path) {
        self.stamp = plnwrk_store::file::stamp(path);
        self.base = self.session.log().len();
        self.saved = self.base;
        self.protection_changed = false;
        self.file_secret = match &self.protection {
            Protection::Passphrase(secret) => Some(secret.clone()),
            Protection::None => None,
        };
    }
}

/// How many changes two logs share from the start.
#[cfg(not(target_arch = "wasm32"))]
fn common_start(a: &[plnwrk_engine::Change], b: &[plnwrk_engine::Change]) -> usize {
    a.iter().zip(b).take_while(|(x, y)| x == y).count()
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
        self.saved = usize::MAX;
        Some(path)
    }

    fn name_of(&self, path: &Path) -> String {
        path.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .and_then(|stem| {
                stem.split(" – Wiederherstellung")
                    .next()
                    .and_then(|stem| stem.split(LOST_MARK).next())
                    .map(str::to_owned)
            })
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
        assert!(doc.save(|_, _| ()).is_err(), "no file yet");
        doc.save_as(&dir.join("abteilung"), |_, _| ()).unwrap();
        assert!(!doc.is_dirty());
        assert_eq!(doc.name(), "abteilung");
        let path = doc.path().unwrap().to_path_buf();
        assert_eq!(path.extension().unwrap(), "plnwrk");

        // Open while still open here: allowed (Phase 10a – a shared file is locked only while
        // saving).
        assert!(OpenDoc::open(&path, None).is_ok());

        let workspace = doc.session.workspace().clone();
        drop(doc);
        let mut again = OpenDoc::open(&path, None).unwrap();
        assert_eq!(again.session.workspace(), &workspace);
        assert!(!again.is_dirty());
        add_person(&mut again, "Neu");
        again.save(|_, _| ()).unwrap();
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

    /// A saved sample on disk and two people who opened it.
    fn shared(name: &str) -> (PathBuf, OpenDoc, OpenDoc) {
        let dir = scratch(name);
        let path = dir.join("gemeinsam.plnwrk");
        let mut doc = OpenDoc::sample().unwrap();
        doc.save_as(&path, |_, _| ()).unwrap();
        drop(doc);
        let anna = OpenDoc::open(&path, None).unwrap();
        let ben = OpenDoc::open(&path, None).unwrap();
        (path, anna, ben)
    }

    fn rename(doc: &mut OpenDoc, from: &str, to: &str) {
        let id = doc
            .session
            .workspace()
            .tasks()
            .find(|t| t.title == from)
            .unwrap()
            .id;
        doc.session
            .run(
                "task.update",
                &json!({"task": id.to_string(), "title": to}),
                Origin::User,
            )
            .unwrap();
    }

    fn titles(path: &Path) -> Vec<String> {
        OpenDoc::open(path, None)
            .unwrap()
            .session
            .workspace()
            .tasks()
            .map(|t| t.title.clone())
            .collect()
    }

    /// Phase 10a: two people save one file one after the other – both changes are in it.
    #[test]
    fn two_people_save_one_file_and_both_changes_arrive() {
        let (path, mut anna, mut ben) = shared("both");
        rename(&mut anna, "Einbau Wache 11", "Einbau Wache 11 (Anna)");
        rename(&mut ben, "Schulung Einsatzkräfte", "Probebetrieb (Ben)");
        assert!(
            anna.save(|_, _| ()).unwrap().is_none(),
            "first: nothing to merge"
        );
        let merged = ben.save(|_, _| ()).unwrap().expect("merged");
        assert_eq!(merged.foreign.len(), 1);
        assert!(!ben.is_dirty());
        let saved = titles(&path);
        assert!(
            saved.contains(&"Einbau Wache 11 (Anna)".to_owned()),
            "{saved:?}"
        );
        assert!(
            saved.contains(&"Probebetrieb (Ben)".to_owned()),
            "{saved:?}"
        );
        // Anna saves again with nothing new: Ben's change is not lost.
        rename(&mut anna, "Bestandsaufnahme Geräte", "Leitstelle (Anna)");
        anna.save(|_, _| ()).unwrap();
        assert!(titles(&path).contains(&"Probebetrieb (Ben)".to_owned()));
    }

    /// Phase 10a: with nothing unsaved, what others saved is fetched; once only.
    #[test]
    fn what_others_saved_is_fetched_when_nothing_is_unsaved() {
        let (_, mut anna, mut ben) = shared("fetch");
        rename(&mut anna, "Einbau Wache 11", "Von Anna");
        anna.save(|_, _| ()).unwrap();
        let fetched = ben.fetch().unwrap().expect("fetched");
        assert_eq!(fetched.foreign.len(), 1);
        assert!(
            ben.session
                .workspace()
                .tasks()
                .any(|t| t.title == "Von Anna")
        );
        assert!(!ben.is_dirty());
        assert!(ben.fetch().unwrap().is_none(), "nothing new");
        // Something unsaved here: no fetching (saving merges).
        rename(&mut anna, "Von Anna", "Wieder Anna");
        anna.save(|_, _| ()).unwrap();
        rename(&mut ben, "Schulung Einsatzkräfte", "Ben tippt");
        assert!(ben.fetch().unwrap().is_none());
        assert!(
            ben.session
                .workspace()
                .tasks()
                .all(|t| t.title != "Wieder Anna")
        );
    }

    /// Phase 10a: while someone else saves (holds the lock), saving waits briefly, then says so
    /// and writes nothing.
    #[test]
    fn saving_waits_for_the_lock_and_writes_nothing_without_it() {
        let (path, _, mut ben) = shared("locked");
        let before = std::fs::read(&path).unwrap();
        let held = plnwrk_store::file::lock(&path).unwrap();
        rename(&mut ben, "Schulung Einsatzkräfte", "Gesperrt");
        let refused = ben.save(|_, _| ()).err();
        assert!(
            matches!(refused, Some(StoreError::InUse { .. })),
            "{refused:?}"
        );
        assert_eq!(std::fs::read(&path).unwrap(), before, "untouched");
        assert!(ben.is_dirty(), "still unsaved");
        drop(held);
        ben.save(|_, _| ()).unwrap();
        assert!(titles(&path).contains(&"Gesperrt".to_owned()));
    }

    /// Phase 10a (B1): "Save as" onto the shared file – e.g. from a recovery copy – merges
    /// instead of replacing what others saved there.
    #[test]
    fn save_as_onto_the_shared_file_merges() {
        let (path, mut anna, mut ben) = shared("save-as");
        let elsewhere = path.with_file_name("kopie.plnwrk");
        rename(&mut ben, "Schulung Einsatzkräfte", "Ben woanders");
        ben.save_as(&elsewhere, |_, _| ()).unwrap();
        rename(&mut anna, "Einbau Wache 11", "Anna im Original");
        anna.save(|_, _| ()).unwrap();
        let merged = ben.save_as(&path, |_, _| ()).unwrap().expect("merged");
        assert_eq!(merged.foreign.len(), 1);
        let saved = titles(&path);
        assert!(saved.contains(&"Anna im Original".to_owned()), "{saved:?}");
        assert!(saved.contains(&"Ben woanders".to_owned()), "{saved:?}");
    }

    /// Phase 10a (W7): someone writes the file while it is being saved here (a drive that
    /// ignores locks) – noticed right before replacing; their file stays, nothing is lost here.
    #[test]
    fn a_write_while_saving_is_noticed_and_not_overwritten() {
        let (path, mut anna, mut ben) = shared("meanwhile");
        rename(&mut anna, "Einbau Wache 11", "Anna zuerst");
        anna.save(|_, _| ()).unwrap();
        rename(&mut ben, "Schulung Einsatzkräfte", "Ben danach");
        let theirs = {
            let mut carla = OpenDoc::open(&path, None).unwrap();
            rename(&mut carla, "Ausschreibung vorbereiten", "Carla dazwischen");
            carla.to_bytes().unwrap()
        };
        let refused = ben.save(|_, _| {
            // Between reading and replacing: Carla's plnwrk writes, ignoring the lock.
            std::fs::write(&path, &theirs).unwrap();
        });
        assert!(
            matches!(refused, Err(StoreError::ChangedMeanwhile { .. })),
            "{refused:?}"
        );
        assert!(
            titles(&path).contains(&"Carla dazwischen".to_owned()),
            "hers stays"
        );
        assert!(ben.is_dirty(), "his still unsaved");
        ben.save(|_, _| ()).unwrap();
        let saved = titles(&path);
        assert!(
            saved.contains(&"Carla dazwischen".to_owned())
                && saved.contains(&"Ben danach".to_owned())
        );
    }

    /// Phase 10a (H1): a file that cannot be read is never written over.
    #[test]
    fn a_shared_file_that_cannot_be_read_is_never_overwritten() {
        let (path, _, mut ben) = shared("unreadable");
        std::fs::write(&path, b"kaputt").unwrap();
        rename(&mut ben, "Schulung Einsatzkräfte", "Nicht schreiben");
        assert!(ben.save(|_, _| ()).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"kaputt");
    }

    /// Phase 10a (W5): someone protected the shared file meanwhile – without the passphrase,
    /// saving does not write it back unprotected.
    #[test]
    fn a_file_protected_meanwhile_is_not_written_back_open() {
        let (path, mut anna, mut ben) = shared("protected");
        anna.set_protection(Protection::Passphrase(SecretString::from(
            "eine-lange-passphrase".to_owned(),
        )));
        anna.save(|_, _| ()).unwrap();
        rename(&mut ben, "Schulung Einsatzkräfte", "Offen?");
        assert_eq!(
            ben.save(|_, _| ()).err(),
            Some(StoreError::PassphraseRequired)
        );
        assert!(
            plnwrk_store::peek(&std::fs::read(&path).unwrap())
                .unwrap()
                .encrypted
        );
    }

    /// Phase 10a (W5): someone removed the protection meanwhile – saving here keeps it off
    /// instead of quietly encrypting the file again.
    #[test]
    fn a_file_unprotected_meanwhile_stays_unprotected() {
        let dir = scratch("unprotected");
        let path = dir.join("gemeinsam.plnwrk");
        let secret = SecretString::from("eine-lange-passphrase".to_owned());
        let mut doc = OpenDoc::sample().unwrap();
        doc.set_protection(Protection::Passphrase(secret.clone()));
        doc.save_as(&path, |_, _| ()).unwrap();
        drop(doc);
        let mut anna = OpenDoc::open(&path, Some(secret.clone())).unwrap();
        let mut ben = OpenDoc::open(&path, Some(secret)).unwrap();
        anna.set_protection(Protection::None);
        anna.save(|_, _| ()).unwrap();
        rename(&mut ben, "Schulung Einsatzkräfte", "Offen geblieben");
        ben.save(|_, _| ()).unwrap();
        assert!(
            !plnwrk_store::peek(&std::fs::read(&path).unwrap())
                .unwrap()
                .encrypted
        );
        assert!(!ben.encrypted());
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

    /// REVIEW R1: a failed "Save as" must not leave a wrong merge start behind – the next
    /// save would set the whole own history onto the file again.
    #[test]
    fn review_a_failed_save_as_does_not_replay_the_history_on_the_next_save() {
        let (path, _anna, mut ben) = shared("rev-failed-save-as");
        // An older copy of the same work: "Save as" there would merge from its start.
        let other = path.with_file_name("andere.plnwrk");
        std::fs::copy(&path, &other).unwrap();
        // Saved earlier: one title changed twice.
        rename(&mut ben, "Einbau Wache 11", "Zwischenstand");
        rename(&mut ben, "Zwischenstand", "Endstand");
        ben.save(|_, _| ()).unwrap();
        let before = ben.session.log().len();
        rename(&mut ben, "Schulung Einsatzkräfte", "Ben");
        // The copy is busy (e.g. open in plnwrk 0.2.0): "Save as" there fails.
        let held = plnwrk_store::file::lock(&other).unwrap();
        assert!(ben.save_as(&other, |_, _| ()).is_err());
        drop(held);
        // Back to the shared file with ⌘S: nothing of others to merge.
        let merged = ben.save(|_, _| ()).unwrap();
        let on_disk = OpenDoc::open(&path, None).unwrap();
        assert_eq!(
            on_disk.session.log().len(),
            before + 1,
            "merged: {:?}",
            merged.map(|m| (m.foreign.len(), m.moved.len(), m.conflicts.len()))
        );
    }

    /// REVIEW R2: "Save as" of a protected work onto another (unprotected) file must not write
    /// it unencrypted.
    #[test]
    fn review_save_as_of_a_protected_work_onto_another_file_stays_encrypted() {
        let dir = scratch("rev-protect-save-as");
        let secret = SecretString::from("eine-lange-passphrase".to_owned());
        let path = dir.join("geheim.plnwrk");
        let mut doc = OpenDoc::sample().unwrap();
        doc.set_protection(Protection::Passphrase(secret.clone()));
        doc.save_as(&path, |_, _| ()).unwrap();
        let other = dir.join("andere.plnwrk");
        let mut plain = OpenDoc::sample().unwrap();
        plain.save_as(&other, |_, _| ()).unwrap();
        drop(plain);
        doc.save_as(&other, |_, _| ()).unwrap();
        assert!(
            plnwrk_store::peek(&std::fs::read(&other).unwrap())
                .unwrap()
                .encrypted,
            "written in plain text (file)"
        );
        assert!(doc.encrypted(), "still protected here");
        assert!(
            plnwrk_store::peek(&std::fs::read(&other).unwrap())
                .unwrap()
                .encrypted,
            "written in plain text"
        );
    }

    /// REVIEW R3: a shared file saves itself after every change, in the window's thread – an
    /// encrypted one must not freeze the window for long each time.
    #[test]
    fn review_saving_an_encrypted_shared_file_is_quick() {
        let dir = scratch("rev-enc-time");
        let path = dir.join("gemeinsam.plnwrk");
        let secret = SecretString::from("eine-lange-passphrase".to_owned());
        let mut doc = OpenDoc::sample().unwrap();
        doc.set_protection(Protection::Passphrase(secret.clone()));
        doc.save_as(&path, |_, _| ()).unwrap();
        drop(doc);
        let mut anna = OpenDoc::open(&path, Some(secret)).unwrap();
        rename(&mut anna, "Einbau Wache 11", "Verschlüsselt");
        // Encrypting once is the price of the protection (the key is derived anew each time);
        // reading the unchanged file again on top of it is not (review 10a, F6).
        let started = std::time::Instant::now();
        anna.to_bytes().unwrap();
        let encrypting = started.elapsed();
        let started = std::time::Instant::now();
        anna.save(|_, _| ()).unwrap();
        let took = started.elapsed();
        assert!(
            took < encrypting * 3 / 2,
            "one autosave took {took:?}, encrypting alone {encrypting:?}"
        );
    }

    /// REVIEW R4: "Save as" from a recovery copy onto the shared file, which someone protected
    /// meanwhile – must not replace it (their protection and their changes).
    #[test]
    fn review_save_as_onto_the_shared_file_protected_meanwhile_does_not_replace_it() {
        let (path, mut anna, mut ben) = shared("rev-save-as-protected");
        // Ben works on from a recovery copy (no file of its own).
        let _ = ben.detach_as_untitled();
        rename(&mut ben, "Schulung Einsatzkräfte", "Ben aus der Kopie");
        anna.set_protection(Protection::Passphrase(SecretString::from(
            "eine-lange-passphrase".to_owned(),
        )));
        rename(&mut anna, "Einbau Wache 11", "Anna geschützt");
        anna.save(|_, _| ()).unwrap();
        let result = ben.save_as(&path, |_, _| ());
        let bytes = std::fs::read(&path).unwrap();
        assert!(
            plnwrk_store::peek(&bytes).unwrap().encrypted,
            "replaced in plain text (save_as: {:?})",
            result.as_ref().map(Option::is_some)
        );
    }
}
