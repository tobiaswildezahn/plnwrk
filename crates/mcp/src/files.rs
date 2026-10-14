//! Opening and saving plnwrk files for the MCP server, with the refusals that keep data out of
//! reach of an AI: nothing classified VS-NfD (now or ever, in its history), nothing encrypted
//! (decision 2026-10-14, docs/plans/2026-10-14-phase-6a.md). Every check runs before anything
//! is read out or run.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};

use plnwrk_core::Classification;
use plnwrk_engine::Session;
use plnwrk_platform::{RandomIds, SystemClock};
use plnwrk_store::{Document, Protection, StoreError, file};

/// Why a file was not opened or saved, worded for the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The path is not usable (empty, relative).
    Path(String),
    /// Classified VS-NfD or higher, now or in its history.
    Classified,
    /// A command would classify something VS-NfD or higher.
    ClassifiedCommand,
    /// Protected against writing by its owner.
    ReadOnly,
    /// Encrypted with a passphrase.
    Encrypted,
    /// Another plnwrk (the app) has it open for changes.
    InUse,
    /// Reading, checking or writing failed.
    Store(StoreError),
}

impl Refusal {
    /// The message for the model.
    pub fn message(&self) -> String {
        match self {
            Self::Path(reason) => reason.clone(),
            Self::Classified => "This file is classified VS-NfD (or was at some point in its \
                history). plnwrk never hands classified content to an AI: work on it in the \
                plnwrk app."
                .to_owned(),
            Self::ClassifiedCommand => "Nothing can be classified VS-NfD over MCP: classified \
                content never goes through an AI. Do it in the plnwrk app."
                .to_owned(),
            Self::ReadOnly => "This file is write-protected. plnwrk does not change it over \
                MCP; remove the protection in the Finder first if it should be changed."
                .to_owned(),
            Self::Encrypted => "This file is encrypted with a passphrase. plnwrk does not open \
                encrypted files over MCP: work on it in the plnwrk app."
                .to_owned(),
            Self::InUse => "This file is open in the plnwrk app (or another plnwrk program). \
                Changes over MCP would clash with it: close it there first, or make the change \
                there."
                .to_owned(),
            Self::Store(error) => format!("The file cannot be used: {error}"),
        }
    }
}

impl From<StoreError> for Refusal {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::InUse { .. } => Self::InUse,
            other => Self::Store(other),
        }
    }
}

/// The file a tool names: `~/` expanded, made absolute and resolved (links followed).
pub fn path(given: &str) -> Result<PathBuf, Refusal> {
    let given = given.trim();
    if given.is_empty() {
        return Err(Refusal::Path("Give the path of a .plnwrk file.".to_owned()));
    }
    let expanded = match given.strip_prefix("~/") {
        Some(rest) => match std::env::var_os("HOME") {
            Some(home) => PathBuf::from(home).join(rest),
            None => PathBuf::from(given),
        },
        None => PathBuf::from(given),
    };
    if !expanded.is_absolute() {
        return Err(Refusal::Path(format!(
            "Give an absolute path (or one starting with ~/), not {given:?}."
        )));
    }
    Ok(file::resolve(&expanded)?)
}

/// An opened file: its session (older data brought up to date, not saved yet) and whether
/// someone else has it open.
pub struct Opened {
    pub session: Session,
    /// The data was converted from an older format on opening.
    pub upgraded: bool,
}

/// Reads and checks a file: the head first (so a classified or encrypted file is refused
/// before its content is looked at), then the content and its history.
fn load(path: &Path) -> Result<Opened, Refusal> {
    check(&file::read(path)?)
}

/// Checks and opens the bytes of a file (see [`load`]).
fn check(bytes: &[u8]) -> Result<Opened, Refusal> {
    let head = plnwrk_store::peek(bytes)?;
    if head.classification >= Classification::VsNfd {
        return Err(Refusal::Classified);
    }
    if head.encrypted {
        return Err(Refusal::Encrypted);
    }
    let opened = plnwrk_store::open(bytes, None)?;
    if plnwrk_store::classification_of(&opened.document) >= Classification::VsNfd {
        return Err(Refusal::Classified);
    }
    let (session, upgraded) = opened
        .document
        .open_session(Box::new(SystemClock), Box::new(RandomIds::default()))
        .map_err(|e| {
            Refusal::Store(StoreError::Corrupt {
                reason: format!("cannot bring older data up to date: {e}"),
            })
        })?;
    Ok(Opened { session, upgraded })
}

/// Opens a file to read. Returns whether another plnwrk has it open (then what is read is the
/// state last saved there). Leaves nothing behind.
pub fn read(path: &Path) -> Result<(Opened, bool), Refusal> {
    let opened = load(path)?;
    Ok((opened, file::is_locked(path)))
}

/// Files whose session is kept at most (the oldest is let go first).
const KEPT_FILES: usize = 16;

/// The sessions of files changed in this conversation, with the bytes last read or written.
/// While a file on disk is still exactly that, the next change continues the same session, so
/// `undo` and `redo` reach back over earlier calls – but only to steps made here: a file
/// changed elsewhere in between starts afresh, with nothing to undo.
#[derive(Default)]
pub struct Kept {
    files: BTreeMap<PathBuf, (Vec<u8>, Session)>,
    /// Paths in the order they were kept, oldest first.
    order: VecDeque<PathBuf>,
}

impl Kept {
    fn take(&mut self, path: &Path) -> Option<(Vec<u8>, Session)> {
        self.order.retain(|p| p != path);
        self.files.remove(path)
    }

    fn put(&mut self, path: PathBuf, bytes: Vec<u8>, session: Session) {
        self.take(&path);
        self.order.push_back(path.clone());
        self.files.insert(path, (bytes, session));
        while self.order.len() > KEPT_FILES {
            if let Some(oldest) = self.order.pop_front() {
                self.files.remove(&oldest);
            }
        }
    }
}

/// A file opened for a change: held against other plnwrk programs until it is dropped.
pub struct Changing {
    pub opened: Opened,
    path: PathBuf,
    /// The file as read.
    bytes: Vec<u8>,
    _lock: file::Lock,
}

impl Changing {
    /// Locks and opens `path`, continuing the kept session if the file has not changed since.
    pub fn open(path: &Path, kept: &mut Kept) -> Result<Self, Refusal> {
        // Everything is checked before the lock, so a refusal leaves nothing (not even a lock
        // file) next to something that is not a plnwrk file we may change.
        let metadata = std::fs::metadata(path).map_err(|e| StoreError::Io {
            reason: e.to_string(),
        })?;
        if !metadata.is_file() {
            return Err(Refusal::Store(StoreError::Io {
                reason: format!("not an ordinary file: {}", path.display()),
            }));
        }
        // Can this user write it? (Not only "nobody may": group rights, the Finder's lock.)
        if let Err(e) = std::fs::OpenOptions::new().write(true).open(path) {
            return Err(if e.kind() == std::io::ErrorKind::PermissionDenied {
                Refusal::ReadOnly
            } else {
                Refusal::Store(StoreError::Io {
                    reason: e.to_string(),
                })
            });
        }
        let known = kept.take(path);
        let bytes = file::read(path)?;
        let continued = known.filter(|(known, _)| *known == bytes);
        let opened = match continued {
            Some((_, session)) => Opened {
                session,
                upgraded: false,
            },
            None => check(&bytes)?,
        };
        let lock = file::lock(path)?;
        // Changed between the check and the lock: checked again as it is now.
        let now = file::read(path)?;
        let opened = if now == bytes { opened } else { check(&now)? };
        Ok(Self {
            opened,
            path: path.to_path_buf(),
            bytes: now,
            _lock: lock,
        })
    }

    /// Saves the session back, atomically, and keeps it for the next call. Refused if it now
    /// holds anything classified (a last line: commands asking for it are refused before they
    /// run).
    pub fn save(self, kept: &mut Kept) -> Result<(), Refusal> {
        let doc = Document::from_session(&self.opened.session);
        if plnwrk_store::classification_of(&doc) >= Classification::VsNfd {
            return Err(Refusal::Classified);
        }
        let bytes = plnwrk_store::save(&doc, &Protection::None)?;
        file::write_atomic(&self.path, &bytes)?;
        kept.put(self.path, bytes, self.opened.session);
        Ok(())
    }

    /// Nothing changed: keeps the session for the next call.
    pub fn keep(self, kept: &mut Kept) {
        kept.put(self.path, self.bytes, self.opened.session);
    }
}
