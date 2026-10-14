//! Autosave: a recovery copy of the open workspace, written in the background at most a minute
//! after a change, so a crash or a power cut loses at most that minute. One copy per opened
//! workspace in the recovery folder; deleted once the file is saved or deliberately dropped.
//! Never written into the file itself.

use std::path::PathBuf;

use crate::doc::OpenDoc;

/// Seconds between a change and its recovery copy.
pub const INTERVAL: f64 = 60.0;

/// The autosave of the app.
#[derive(Debug, Default)]
pub struct Autosave {
    /// Where copies go; `None` switches autosave off (browser, most tests).
    folder: Option<PathBuf>,
    /// Write in a background thread (the app) or right away (tests).
    background: bool,
    /// Seconds to wait after a change.
    interval: f64,
    /// The copy of the open workspace, once written (or the recovered file it came from).
    copy: Option<PathBuf>,
    /// The state (log length) the copy holds.
    written: Option<usize>,
    /// When the first change not yet in the copy was seen (egui time, seconds).
    since: Option<f64>,
    /// A copy being written.
    #[cfg(not(target_arch = "wasm32"))]
    worker: Option<std::thread::JoinHandle<()>>,
}

impl Autosave {
    /// An autosave into `folder`.
    pub fn new(folder: Option<PathBuf>, background: bool, interval: f64) -> Self {
        Self {
            folder,
            background,
            interval,
            ..Self::default()
        }
    }

    /// The recovery folder, if autosave is on.
    pub fn folder(&self) -> Option<&std::path::Path> {
        self.folder.as_deref()
    }

    /// The copy of the open workspace, if one was written.
    #[cfg(test)]
    pub fn copy(&self) -> Option<&std::path::Path> {
        self.copy.as_deref()
    }

    /// Takes over an existing copy (a recovered file that was opened): later autosaves replace
    /// it, saving deletes it.
    pub fn adopt(&mut self, copy: PathBuf) {
        self.finish();
        self.copy = Some(copy);
        self.written = None;
        self.since = None;
    }

    /// Called every frame: writes a copy when changes have waited long enough. Returns how long
    /// until it should be called again (for a repaint), if a copy is due.
    pub fn tick(&mut self, doc: Option<&OpenDoc>, now: f64) -> Option<f64> {
        #[cfg(not(target_arch = "wasm32"))]
        if self
            .worker
            .as_ref()
            .is_some_and(std::thread::JoinHandle::is_finished)
        {
            self.finish();
        }
        let doc = doc?;
        let folder = self.folder.clone()?;
        if !doc.is_dirty() || self.written == Some(doc.log_len()) {
            self.since = None;
            return None;
        }
        let since = *self.since.get_or_insert(now);
        let due = since + self.interval;
        if now < due {
            return Some(due - now);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if self.worker.is_some() {
            return Some(1.0);
        }
        self.write(doc, &folder);
        None
    }

    /// Writes the copy now and waits for it (when the app ends without asking).
    pub fn write_now(&mut self, doc: &OpenDoc) -> Option<PathBuf> {
        let folder = self.folder.clone()?;
        self.finish();
        let background = std::mem::replace(&mut self.background, false);
        self.write(doc, &folder);
        self.background = background;
        self.copy.clone()
    }

    /// The workspace was saved or deliberately dropped: the copy goes.
    pub fn discard(&mut self) {
        self.finish();
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(copy) = self.copy.take() {
            let _ = std::fs::remove_file(copy);
        }
        self.copy = None;
        self.written = None;
        self.since = None;
    }

    /// Waits for a copy being written.
    fn finish(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn write(&mut self, doc: &OpenDoc, folder: &std::path::Path) {
        let path = self
            .copy
            .clone()
            .unwrap_or_else(|| doc.recovery_path(folder));
        let (data, protection) = doc.snapshot();
        let folder = folder.to_path_buf();
        let target = path.clone();
        let job = move || {
            // Saving encrypts (about a second) – that is why it runs apart from the window.
            if std::fs::create_dir_all(&folder).is_ok()
                && let Ok(bytes) = plnwrk_store::save(&data, &protection)
            {
                let _ = plnwrk_store::file::write_atomic(&target, &bytes);
            }
        };
        if self.background {
            self.worker = Some(std::thread::spawn(job));
        } else {
            job();
        }
        self.copy = Some(path);
        self.written = Some(doc.log_len());
        self.since = None;
    }

    #[cfg(target_arch = "wasm32")]
    fn write(&mut self, _doc: &OpenDoc, _folder: &std::path::Path) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use plnwrk_engine::Origin;
    use serde_json::json;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("plnwrk-autosave-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn change(doc: &mut OpenDoc, name: &str) {
        doc.session
            .run("person.create", &json!({"name": name}), Origin::User)
            .unwrap();
    }

    #[test]
    fn a_copy_follows_a_minute_after_a_change_and_only_then() {
        let dir = scratch("timing");
        let mut autosave = Autosave::new(Some(dir.clone()), false, INTERVAL);
        let mut doc = OpenDoc::new();
        assert_eq!(autosave.tick(Some(&doc), 0.0), None, "nothing changed");
        change(&mut doc, "A");
        assert_eq!(autosave.tick(Some(&doc), 10.0), Some(INTERVAL));
        assert_eq!(autosave.tick(Some(&doc), 69.0), Some(1.0));
        assert!(autosave.copy().is_none());
        assert_eq!(autosave.tick(Some(&doc), 70.0), None);
        let copy = autosave.copy().unwrap().to_path_buf();
        let reopened = OpenDoc::open(&copy, None).unwrap();
        assert!(reopened.session.workspace().people().any(|p| p.name == "A"));
        drop(reopened);
        // Nothing new: nothing written; the next change waits its minute again.
        assert_eq!(autosave.tick(Some(&doc), 200.0), None);
        change(&mut doc, "B");
        assert_eq!(autosave.tick(Some(&doc), 300.0), Some(INTERVAL));
        assert_eq!(autosave.tick(Some(&doc), 360.0), None);
        assert_eq!(
            autosave.copy(),
            Some(copy.as_path()),
            "the same copy, replaced"
        );
        let reopened = OpenDoc::open(&copy, None).unwrap();
        assert!(reopened.session.workspace().people().any(|p| p.name == "B"));
        drop(reopened);
        assert_eq!(crate::doc::recovery_copies(&dir).len(), 1);
        autosave.discard();
        assert!(!copy.exists());
        assert!(crate::doc::recovery_copies(&dir).is_empty());
    }

    #[test]
    fn background_copies_are_complete_before_discard_returns() {
        let dir = scratch("background");
        let mut autosave = Autosave::new(Some(dir.clone()), true, 0.0);
        let mut doc = OpenDoc::new();
        change(&mut doc, "A");
        autosave.tick(Some(&doc), 0.0);
        let copy = autosave.copy().unwrap().to_path_buf();
        // Saving the file now must not leave a copy written later by the thread.
        autosave.discard();
        assert!(!copy.exists());
        assert!(crate::doc::recovery_copies(&dir).is_empty());
    }

    #[test]
    fn switched_off_it_never_writes() {
        let mut autosave = Autosave::default();
        let mut doc = OpenDoc::new();
        change(&mut doc, "A");
        assert_eq!(autosave.tick(Some(&doc), 1_000.0), None);
        assert!(autosave.write_now(&doc).is_none());
    }
}
