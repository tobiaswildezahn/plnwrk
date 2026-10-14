//! Autosave: a recovery copy of the open workspace, written in the background at most a minute
//! after a change, so a crash or a power cut loses at most that minute. One copy per opened
//! workspace in the recovery folder; deleted once the file is saved or deliberately dropped.
//! Never written into the file itself.
//!
//! It runs from `eframe::App::logic`, which is called even while the window is minimised or
//! covered, and measures time with a real clock there (egui's frame time stands still while the
//! window is hidden). Tests drive it with egui time instead.

use std::path::PathBuf;

use crate::doc::OpenDoc;

/// Seconds between a change and its recovery copy.
pub const INTERVAL: f64 = 60.0;

/// What the last write did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Written {
    /// The copy is up to date (local time of day, `HH:MM`).
    At(String),
    /// It could not be written; why.
    Failed(String),
}

/// The autosave of the app.
#[derive(Debug, Default)]
pub struct Autosave {
    /// Where copies go; `None` switches autosave off (browser, most tests).
    folder: Option<PathBuf>,
    /// Write in a background thread (the app) or right away (tests).
    background: bool,
    /// Measure time with a real clock (the app) instead of egui time (tests).
    #[cfg(not(target_arch = "wasm32"))]
    started: Option<std::time::Instant>,
    /// Seconds to wait after a change.
    interval: f64,
    /// The copy of the open workspace, once written (or the recovered file it came from).
    copy: Option<PathBuf>,
    /// The copy is a recovered file that was opened: it is only removed once saved elsewhere.
    adopted: bool,
    /// The state (log length) the copy holds.
    written: Option<usize>,
    /// When the first change not yet in the copy was seen (seconds).
    since: Option<f64>,
    /// The outcome of the last write, for the status bar.
    last: Option<Written>,
    /// A copy being written: its result and the state it holds.
    #[cfg(not(target_arch = "wasm32"))]
    worker: Option<(std::thread::JoinHandle<Result<String, String>>, usize)>,
}

impl Autosave {
    /// An autosave into `folder`, timed by egui (for tests).
    pub fn new(folder: Option<PathBuf>, background: bool, interval: f64) -> Self {
        Self {
            folder,
            background,
            interval,
            ..Self::default()
        }
    }

    /// The autosave of the desktop app: background thread, real clock.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn for_app(folder: PathBuf) -> Self {
        Self {
            started: Some(std::time::Instant::now()),
            ..Self::new(Some(folder), true, INTERVAL)
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

    /// Waits for a copy being written in the background (tests).
    #[cfg(test)]
    pub fn settle(&mut self) {
        self.finish();
    }

    /// What the last write did, for the status bar.
    pub fn last(&self) -> Option<&Written> {
        self.last.as_ref()
    }

    /// Takes over an existing copy (a recovered file that was opened): later autosaves replace
    /// it, saving elsewhere deletes it, dropping the work keeps it.
    pub fn adopt(&mut self, copy: PathBuf) {
        self.finish();
        self.copy = Some(copy);
        self.adopted = true;
        self.written = None;
        self.since = None;
        self.last = None;
    }

    /// Called regularly: writes a copy when changes have waited long enough. `egui_time` is
    /// used unless a real clock runs. Returns how long until it should be called again, if a
    /// copy is due.
    pub fn tick(&mut self, doc: Option<&OpenDoc>, egui_time: f64) -> Option<f64> {
        let now = self.now(egui_time);
        self.reap();
        let doc = doc?;
        let folder = self.folder.clone()?;
        if !doc.is_dirty() {
            // Back at the saved state (e.g. undone): an own copy is no longer needed.
            if self.copy.is_some() && !self.adopted {
                let _ = self.discard();
            }
            self.since = None;
            return None;
        }
        if self.written == Some(doc.log_len()) {
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
        // After a failure, try again an interval later (not every frame).
        if matches!(self.last, Some(Written::Failed(_))) {
            self.since = Some(now);
            return Some(self.interval);
        }
        // A background write: come back soon to take in its result (also while hidden).
        #[cfg(not(target_arch = "wasm32"))]
        if self.worker.is_some() {
            return Some(1.0);
        }
        None
    }

    /// Writes the copy now and waits for it (when the app ends without asking). The copy, if it
    /// was really written.
    pub fn write_now(&mut self, doc: &OpenDoc) -> Option<PathBuf> {
        let folder = self.folder.clone()?;
        self.finish();
        let background = std::mem::replace(&mut self.background, false);
        self.write(doc, &folder);
        self.background = background;
        match self.last {
            Some(Written::At(_)) => self.copy.clone(),
            _ => None,
        }
    }

    /// The work was deliberately dropped (closed, discarded, replaced): the copy goes – unless
    /// it is a recovered file, which stays until its content is saved somewhere. Returns such a
    /// kept copy, so it can be offered again.
    pub fn discard(&mut self) -> Option<PathBuf> {
        self.finish();
        let kept = match self.copy.take() {
            Some(copy) if self.adopted => Some(copy),
            Some(copy) => {
                #[cfg(not(target_arch = "wasm32"))]
                let _ = std::fs::remove_file(copy);
                #[cfg(target_arch = "wasm32")]
                let _ = copy;
                None
            }
            None => None,
        };
        self.reset();
        kept
    }

    /// The work was saved: the copy goes, a recovered one too.
    pub fn saved(&mut self) {
        self.adopted = false;
        let _ = self.discard();
    }

    fn reset(&mut self) {
        self.copy = None;
        self.adopted = false;
        self.written = None;
        self.since = None;
        self.last = None;
    }

    fn now(&self, egui_time: f64) -> f64 {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(started) = self.started {
            return started.elapsed().as_secs_f64();
        }
        egui_time
    }

    /// Takes in the result of a finished background write.
    fn reap(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        if self
            .worker
            .as_ref()
            .is_some_and(|(worker, _)| worker.is_finished())
        {
            self.finish();
        }
    }

    /// Waits for a copy being written and takes in its result.
    fn finish(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some((worker, state)) = self.worker.take() {
            let result = worker
                .join()
                .unwrap_or_else(|_| Err(crate::text::RECOVERY_WRITER_STOPPED.to_owned()));
            self.record(result, state);
        }
    }

    /// Takes in a write: the time it was written (`HH:MM`) or why it failed.
    fn record(&mut self, result: Result<String, String>, state: usize) {
        match result {
            Ok(at) => {
                self.written = Some(state);
                self.last = Some(Written::At(at));
            }
            Err(reason) => {
                self.written = None;
                self.last = Some(Written::Failed(reason));
            }
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
        let job = move || -> Result<String, String> {
            // Saving encrypts (about a second) – that is why it runs apart from the window.
            let failed = |e: plnwrk_store::StoreError| crate::text::store_error(&e);
            std::fs::create_dir_all(&folder).map_err(|e| {
                failed(plnwrk_store::StoreError::Io {
                    reason: e.to_string(),
                })
            })?;
            let bytes = plnwrk_store::save(&data, &protection).map_err(failed)?;
            plnwrk_store::file::write_atomic(&target, &bytes).map_err(failed)?;
            Ok(jiff::Zoned::now().strftime("%H:%M").to_string())
        };
        self.copy = Some(path);
        self.since = None;
        let state = doc.log_len();
        if self.background {
            // Counted as written while it runs, so it is not started twice for the same state.
            self.written = Some(state);
            self.worker = Some((std::thread::spawn(job), state));
        } else {
            let result = job();
            self.record(result, state);
        }
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
        assert!(matches!(autosave.last(), Some(Written::At(_))));
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
        assert_eq!(autosave.discard(), None);
        assert!(!copy.exists());
        assert!(crate::doc::recovery_copies(&dir).is_empty());
    }

    #[test]
    fn a_running_background_copy_is_waited_for_before_it_is_removed() {
        let dir = scratch("background");
        let mut autosave = Autosave::new(Some(dir.clone()), true, 0.0);
        let mut doc = OpenDoc::new();
        // Encrypted: writing takes about a second, long enough to race with the removal.
        doc.set_protection(plnwrk_store::Protection::Passphrase(
            plnwrk_store::SecretString::from("richtig-lange-passphrase".to_owned()),
        ));
        change(&mut doc, "A");
        autosave.tick(Some(&doc), 0.0);
        let copy = autosave.copy().unwrap().to_path_buf();
        assert_eq!(autosave.discard(), None);
        // Had the thread been left running, it would write the copy after the removal.
        std::thread::sleep(std::time::Duration::from_millis(2_500));
        assert!(!copy.exists());
        assert!(crate::doc::recovery_copies(&dir).is_empty());
    }

    #[test]
    fn a_failed_copy_is_reported_and_retried_later() {
        let dir = scratch("failed");
        let mut autosave = Autosave::new(Some(dir.clone()), false, INTERVAL);
        let mut doc = OpenDoc::new();
        change(&mut doc, "A");
        // VS-NfD may only be written encrypted; this workspace has no passphrase.
        let created = doc
            .session
            .run(
                "project.create",
                &json!({"title": "Geheim", "classification": "vs_nfd"}),
                Origin::User,
            )
            .unwrap();
        assert!(!created.created.is_empty());
        autosave.tick(Some(&doc), 0.0);
        assert_eq!(
            autosave.tick(Some(&doc), 60.0),
            Some(INTERVAL),
            "retry later"
        );
        assert!(matches!(autosave.last(), Some(Written::Failed(_))));
        assert!(autosave.write_now(&doc).is_none(), "no copy claimed");
        // In the background as well: the result is taken in a second later, not left waiting.
        let mut background = Autosave::new(Some(dir.clone()), true, 0.0);
        assert_eq!(
            background.tick(Some(&doc), 0.0),
            Some(1.0),
            "come back for the result"
        );
        let mut waited = 0;
        while !matches!(background.last(), Some(Written::Failed(_))) && waited < 50 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            background.tick(Some(&doc), 1.0);
            waited += 1;
        }
        assert!(matches!(background.last(), Some(Written::Failed(_))));
        assert!(crate::doc::recovery_copies(&dir).is_empty());
    }

    #[test]
    fn back_at_the_saved_state_the_copy_goes_but_a_recovered_one_stays() {
        let dir = scratch("undone");
        let mut autosave = Autosave::new(Some(dir.clone()), false, 0.0);
        let mut doc = OpenDoc::new();
        change(&mut doc, "A");
        autosave.tick(Some(&doc), 0.0);
        let copy = autosave.copy().unwrap().to_path_buf();
        assert!(copy.exists());
        let clean = OpenDoc::new();
        autosave.tick(Some(&clean), 1.0);
        assert!(!copy.exists(), "nothing unsaved any more");
        // A recovered copy that is dropped stays for later; saved elsewhere, it goes.
        std::fs::create_dir_all(&dir).unwrap();
        let recovered = dir.join("alt.plnwrk");
        std::fs::write(&recovered, b"x").unwrap();
        autosave.adopt(recovered.clone());
        assert_eq!(autosave.discard(), Some(recovered.clone()), "offered again");
        assert!(recovered.exists());
        autosave.adopt(recovered.clone());
        autosave.saved();
        assert!(!recovered.exists());
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
