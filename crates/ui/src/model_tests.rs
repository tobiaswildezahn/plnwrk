//! A state machine test of autosave and "unsaved": random sequences of what a person (and the
//! machine) does – change, type, wait, undo, save, close, crash, restart – checked against a
//! small model after every step. Three review rounds found edge cases here by reading; this
//! looks for the ones nobody thought of.

use std::path::{Path, PathBuf};

use plnwrk_core::{TaskId, Workspace};
use proptest::prelude::*;
use serde_json::json;

use crate::host::Scripted;
use crate::state::FileAction;
use crate::{Intent, OpenDoc, PlnwrkApp, autosave};

/// What happens next.
#[derive(Debug, Clone)]
enum Step {
    /// A change through a command.
    Change,
    /// A title typed in the list, not taken yet.
    Type,
    /// A description typed in the detail panel, not taken yet.
    Describe,
    /// Time passes (seconds).
    Wait(u16),
    Undo,
    Redo,
    /// Cmd+S.
    Save,
    /// Close and discard, then open the file again.
    CloseDiscard,
    /// The app ends. `asked`: through `on_exit` (the emergency save); otherwise a hard kill.
    /// `recover`: on restart, the offered copy is opened and saved over the file; otherwise
    /// it is deleted and the file opened.
    Crash {
        asked: bool,
        recover: bool,
    },
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        4 => Just(Step::Change),
        2 => Just(Step::Type),
        2 => Just(Step::Describe),
        4 => (0u16..150).prop_map(Step::Wait),
        2 => Just(Step::Undo),
        1 => Just(Step::Redo),
        2 => Just(Step::Save),
        1 => Just(Step::CloseDiscard),
        1 => (any::<bool>(), any::<bool>()).prop_map(|(asked, recover)| Step::Crash { asked, recover }),
    ]
}

/// Reads a file without locking it (the app may hold the lock).
fn read(path: &Path) -> Workspace {
    let bytes = std::fs::read(path).unwrap();
    OpenDoc::from_bytes(&bytes, None)
        .unwrap()
        .session
        .workspace()
        .clone()
}

/// The test world: the app, its folders and what the model expects.
struct World {
    dir: PathBuf,
    file: PathBuf,
    recovery: PathBuf,
    background: bool,
    app: PlnwrkApp,
    task: TaskId,
    time: f64,
    serial: u32,
    /// What the file holds.
    saved: Workspace,
    /// Changes in the data since the file was saved or opened.
    changed: bool,
    /// A title typed in the list and not taken.
    typed: Option<String>,
    /// A description typed in the detail panel and not taken.
    described: Option<String>,
    /// When the data last changed.
    last_change: f64,
    /// Every state of the data since the file was last saved or opened (a copy holds one).
    states: Vec<Workspace>,
}

impl World {
    fn new(name: &str, background: bool) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "plnwrk-model-{}-{name}-{}",
            plnwrk_platform::instance(),
            rand_suffix()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("lage.plnwrk");
        let recovery = dir.join("Notkopien");
        // A file with a project and a task, saved and closed.
        let mut doc = OpenDoc::new();
        doc.session
            .run(
                "project.create",
                &json!({"title": "Lage"}),
                plnwrk_engine::Origin::User,
            )
            .unwrap();
        let project = doc.session.workspace().projects().next().unwrap().id;
        doc.session
            .run(
                "task.create",
                &json!({"project": project.to_string(), "title": "Aufgabe"}),
                plnwrk_engine::Origin::User,
            )
            .unwrap();
        let task = doc.session.workspace().tasks().next().unwrap().id;
        doc.save_as(&file).unwrap();
        drop(doc);
        let saved = read(&file);
        let app = Self::start(&file, &recovery, background);
        let mut world = Self {
            dir,
            file,
            recovery,
            background,
            app,
            task,
            time: 0.0,
            serial: 0,
            saved: saved.clone(),
            changed: false,
            typed: None,
            described: None,
            last_change: 0.0,
            states: vec![saved],
        };
        world.app.open_path(world.file.clone());
        world.tick();
        world
    }

    /// A freshly started app (like after a restart), offering the copies it finds.
    fn start(file: &Path, recovery: &Path, background: bool) -> PlnwrkApp {
        let host = Scripted {
            open: Vec::new(),
            save: vec![file.to_path_buf(); 64],
            ..Scripted::default()
        };
        let mut app = PlnwrkApp::with_host(Box::new(host));
        app.set_autosave(Some(recovery.to_path_buf()), background, autosave::INTERVAL);
        app.state.recovered = crate::doc::recovery_copies(recovery);
        app
    }

    fn workspace(&self) -> Workspace {
        self.app.doc().unwrap().session.workspace().clone()
    }

    fn copies(&self) -> Vec<PathBuf> {
        crate::doc::recovery_copies(&self.recovery)
    }

    /// A frame at the current time; background writes are waited for.
    fn tick(&mut self) {
        self.app.autosave_at(self.time);
        if self.background {
            self.app.autosave.settle();
            self.app.autosave_at(self.time);
        }
    }

    fn data_changed(&mut self) {
        self.changed = true;
        self.last_change = self.time;
        let now = self.workspace();
        self.states.push(now);
    }

    /// The file was saved or opened: this is the new starting point.
    fn fresh(&mut self) {
        self.saved = read(&self.file);
        self.changed = false;
        self.typed = None;
        self.described = None;
        self.states = vec![self.saved.clone()];
        self.last_change = self.time;
    }

    fn run(&mut self, step: &Step) {
        self.serial += 1;
        match step {
            Step::Change => {
                self.app.apply(Intent::run(
                    "person.create",
                    json!({"name": format!("P{}", self.serial)}),
                ));
                self.data_changed();
            }
            Step::Type => {
                let title = format!("T{}", self.serial);
                self.app.state.forms.inline = Some((self.task, title.clone()));
                self.typed = Some(title);
            }
            Step::Describe => {
                let text = format!("D{}", self.serial);
                let task = self.workspace().task(self.task).cloned().unwrap();
                self.app.state.select(Some(self.task));
                let mut draft = crate::state::TaskDraft::of(&task);
                draft.description.clone_from(&text);
                self.app.state.forms.task = Some(draft);
                self.described = Some(text);
            }
            Step::Wait(seconds) => self.time += f64::from(*seconds),
            Step::Undo | Step::Redo => {
                let session = &self.app.doc().unwrap().session;
                let (possible, command) = match step {
                    Step::Undo => (session.can_undo(), "undo"),
                    _ => (session.can_redo(), "redo"),
                };
                if possible {
                    self.app.apply(Intent::run(command, json!({})));
                    self.data_changed();
                }
            }
            Step::Save => {
                let typed = self.typed.clone();
                let described = self.described.clone();
                self.app.apply(Intent::File(FileAction::Save));
                if let Some(text) = described {
                    let stored = read(&self.file)
                        .task(self.task)
                        .unwrap()
                        .description
                        .clone();
                    assert_eq!(stored, text, "typed description is saved");
                }
                assert_eq!(read(&self.file), self.workspace(), "saved what is shown");
                if let Some(title) = typed {
                    let stored = read(&self.file).task(self.task).unwrap().title.clone();
                    assert_eq!(stored, title, "typed text is saved");
                }
                self.fresh();
                self.tick();
                assert!(self.copies().is_empty(), "saved: no copy is left");
            }
            Step::CloseDiscard => {
                self.app.apply(Intent::Discard(FileAction::Close));
                assert!(self.app.doc().is_none());
                assert!(self.copies().is_empty(), "discarded: its copy goes");
                assert!(!self.app.has_unsaved_changes());
                self.app.open_path(self.file.clone());
                self.fresh();
                assert_eq!(self.workspace(), self.saved, "the file is as saved");
            }
            Step::Crash { asked, recover } => self.crash(*asked, *recover),
        }
        self.tick();
        self.check();
    }

    fn crash(&mut self, asked: bool, recover: bool) {
        let unsaved = self.app.has_unsaved_changes();
        if asked {
            let copy = self.app.emergency_save();
            assert_eq!(
                copy.is_some(),
                unsaved,
                "a copy exactly when work would be lost"
            );
            if let Some(copy) = &copy {
                // Typed text was taken first, so the copy holds it.
                assert_eq!(read(copy), self.workspace(), "the copy holds what is shown");
                if let Some(title) = &self.typed {
                    assert_eq!(&read(copy).task(self.task).unwrap().title, title);
                }
                if let Some(text) = &self.described {
                    assert_eq!(&read(copy).task(self.task).unwrap().description, text);
                }
                self.states.push(read(copy));
            }
        }
        let copies = self.copies();
        assert!(copies.len() <= 1, "one copy per open workspace: {copies:?}");
        drop(std::mem::replace(
            &mut self.app,
            Self::start(&self.file, &self.recovery, self.background),
        ));
        assert_eq!(self.app.state.recovered, copies, "every copy is offered");
        assert_eq!(
            read(&self.file),
            self.saved,
            "a crash never touches the file"
        );
        match copies.first() {
            Some(copy) if recover => {
                let content = read(copy);
                assert!(
                    self.states.contains(&content),
                    "a copy holds a state the person had"
                );
                self.app.open_path(copy.clone());
                assert_eq!(self.workspace(), content);
                assert!(self.app.has_unsaved_changes(), "recovered work is unsaved");
                self.app.apply(Intent::File(FileAction::Save));
                assert_eq!(read(&self.file), content, "saved over the file");
                assert!(!copy.exists(), "recovered and saved: the copy goes");
            }
            _ => {
                for copy in &copies {
                    self.app.apply(Intent::DeleteCopy(copy.clone()));
                    assert!(!copy.exists());
                }
                self.app.open_path(self.file.clone());
            }
        }
        self.fresh();
    }

    /// What must always hold.
    fn check(&self) {
        let doc = self.app.doc().unwrap();
        assert_eq!(read(&self.file), self.saved, "only saving writes the file");
        assert_eq!(doc.is_dirty(), self.changed, "data changed since saving");
        assert_eq!(
            self.app.has_unsaved_changes(),
            self.changed || self.typed.is_some() || self.described.is_some(),
            "unsaved = changed or typed"
        );
        let copies = self.copies();
        assert!(copies.len() <= 1, "one copy per open workspace: {copies:?}");
        if let Some(copy) = copies.first() {
            assert!(
                self.states.contains(&read(copy)),
                "a copy holds a state the person had"
            );
        }
        if !self.changed {
            assert!(copies.is_empty(), "nothing to keep: no copy");
        }
        if self.changed && self.time - self.last_change >= autosave::INTERVAL {
            let copy = copies
                .first()
                .expect("a minute after a change, a copy exists");
            assert_eq!(read(copy), self.workspace(), "and it is up to date");
        }
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn rand_suffix() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    N.fetch_add(1, Ordering::Relaxed)
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 48,
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn nothing_is_lost_whatever_happens(
        steps in prop::collection::vec(step(), 1..30),
        background in any::<bool>(),
    ) {
        let mut world = World::new("any", background);
        for step in &steps {
            world.run(step);
        }
    }
}
