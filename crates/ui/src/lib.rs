//! egui user interface of plnwrk (layer 4), shared by the desktop and web apps.
//!
//! See `docs/PLAN.md` §3.6. The views only read the workspace (through the typed queries of the
//! engine) and describe what the person wants as [`Intent`]s; [`PlnwrkApp`] carries them out
//! after drawing, changing data only through `Session::run`.

// In the browser, opening and saving files follows in Phase 4 (upload/download); until then the
// file code is only used on desktop systems.
#![cfg_attr(
    target_arch = "wasm32",
    allow(dead_code, reason = "file access in the browser follows in Phase 4")
)]

mod dates;
mod doc;
mod host;
mod state;
mod text;
mod views;

use std::path::PathBuf;

use plnwrk_core::{EntityKey, PersonId, ProjectId, TaskId};
use plnwrk_engine::Origin;
use plnwrk_store::{SecretString, StoreError};
use serde_json::Value;

pub use doc::OpenDoc;
pub use host::{Host, NoDialogs};
use state::{Dialog, FileAction, Message, State, Tab, View};

/// Name shown in the window title and the app header.
pub const APP_NAME: &str = "plnwrk";

/// Key of the list of recently opened files in the app's storage.
const RECENT_KEY: &str = "plnwrk.recent";
/// How many recent files are kept.
const RECENT_MAX: usize = 8;

/// What to do after a command succeeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Then {
    /// Nothing more.
    Nothing,
    /// Show the project it created.
    ShowProject,
}

/// Which form to empty after a command succeeded (on failure the input stays).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Clear {
    NewTask,
    NewSubtask,
    DetailSubtask,
    Milestone,
    Comment,
    Person,
    Dialog,
    ProjectEdit,
}

/// What a view asks for. Carried out after the frame is drawn.
#[derive(Debug)]
pub(crate) enum Intent {
    /// Run a command.
    Run {
        command: &'static str,
        params: Value,
        then: Then,
        clear: Option<Clear>,
    },
    /// Show another view.
    Go(View),
    /// Select a task (detail panel) or none.
    Select(Option<TaskId>),
    /// Open a dialog.
    Open(Dialog),
    /// Close the dialog.
    CloseDialog,
    /// A file action (new, open, save …).
    File(FileAction),
    /// Open an encrypted file with this passphrase.
    Unlock(PathBuf, SecretString),
    /// Save first, then carry out the action.
    SaveThen(FileAction),
    /// Carry out the action and drop unsaved changes.
    Discard(FileAction),
    /// Set who is working.
    Actor(Option<PersonId>),
}

impl Intent {
    /// A command without follow-up.
    pub(crate) fn run(command: &'static str, params: Value) -> Self {
        Self::Run {
            command,
            params,
            then: Then::Nothing,
            clear: None,
        }
    }
}

/// The whole application: the open workspace (if any), the dialogs of the platform and the
/// state of the views.
pub struct PlnwrkApp {
    doc: Option<OpenDoc>,
    host: Box<dyn Host>,
    state: State,
    /// The window title last sent, so it is only sent when it changes.
    shown_title: String,
}

impl std::fmt::Debug for PlnwrkApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlnwrkApp")
            .field("open", &self.doc.is_some())
            .finish_non_exhaustive()
    }
}

impl Default for PlnwrkApp {
    fn default() -> Self {
        Self::with_host(Box::new(NoDialogs))
    }
}

impl PlnwrkApp {
    /// Creates the app when the desktop or web shell starts it.
    pub fn new(cc: &eframe::CreationContext<'_>, host: Box<dyn Host>) -> Self {
        let mut app = Self::with_host(host);
        if let Some(storage) = cc.storage
            && let Some(text) = storage.get_string(RECENT_KEY)
        {
            app.state.recent = serde_json::from_str(&text).unwrap_or_default();
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(folder) = doc::recovery_folder() {
            app.state.recovered = doc::recovery_copies(&folder);
        }
        app
    }

    /// An app without stored settings, using these dialogs.
    pub fn with_host(host: Box<dyn Host>) -> Self {
        Self {
            doc: None,
            host,
            state: State::default(),
            shown_title: String::new(),
        }
    }

    /// The open workspace.
    pub fn doc(&self) -> Option<&OpenDoc> {
        self.doc.as_ref()
    }

    /// Whether closing the window may lose changes: unsaved data, or text typed in the detail
    /// panel that is not stored yet.
    pub fn has_unsaved_changes(&self) -> bool {
        self.doc.is_some()
            && (self.doc.as_ref().is_some_and(OpenDoc::is_dirty)
                || self
                    .state
                    .forms
                    .task
                    .as_ref()
                    .is_some_and(state::TaskDraft::is_pending))
    }

    /// Stores what is typed in the detail panel but not committed yet (the field still has the
    /// focus). Never blocks: what cannot be stored (a wrong date, an empty title) is put back
    /// to the stored value and named in a message, so saving and closing always work.
    /// Returns the warning about what was put back, if anything.
    fn flush_draft(&mut self) -> Option<String> {
        let doc = self.doc.as_ref()?;
        let mut draft = self.state.forms.task.take()?;
        let task = doc.session.workspace().task(draft.source.id).cloned()?;
        let mut warnings: Vec<String> = draft.drop_invalid_dates().into_iter().collect();
        let changes = draft.changes().unwrap_or_default();
        self.state.forms.task = Some(draft);
        if !changes.is_empty() {
            let mut params = changes;
            params.insert("task".to_owned(), Value::String(task.id.to_string()));
            self.run("task.update", &Value::Object(params), Then::Nothing, None);
            if let Some(message) = self.state.message.as_ref().filter(|m| m.error) {
                // Refused (e.g. empty title): back to what is stored, and say so.
                warnings.push(format!("Nicht übernommen: {}", message.text));
                self.state.forms.task = Some(state::TaskDraft::of(&task));
            }
        }
        let warning = (!warnings.is_empty()).then(|| warnings.join(" · "));
        if let Some(text) = &warning {
            self.state.message = Some(Message::error(text.clone()));
        }
        warning
    }

    /// The window title: file name, a dot for unsaved changes, app name.
    pub fn title(&self) -> String {
        match &self.doc {
            Some(doc) if self.has_unsaved_changes() => format!("● {} – {APP_NAME}", doc.name()),
            Some(doc) => format!("{} – {APP_NAME}", doc.name()),
            None => APP_NAME.to_owned(),
        }
    }

    /// Opens a file given from outside (e.g. on the command line).
    pub fn open_path(&mut self, path: PathBuf) {
        self.file(FileAction::OpenPath(path));
    }

    /// The person asked to close the window. Returns whether it may close now; otherwise the
    /// app asks about unsaved changes and closes itself afterwards.
    pub fn close_requested(&mut self) -> bool {
        if self.state.quitting {
            return true;
        }
        self.flush_draft();
        if !self.has_unsaved_changes() {
            return true;
        }
        self.state.open_dialog(Dialog::Unsaved(FileAction::Quit));
        false
    }

    /// Last chance when the app ends without asking (e.g. "Beenden" in the Dock, logging out):
    /// unsaved changes go to a recovery copy in [`doc::recovery_folder`], listed on the start
    /// page next time.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn emergency_save(&mut self) {
        if let Some(folder) = doc::recovery_folder() {
            self.emergency_save_to(&folder);
        }
    }

    /// [`Self::emergency_save`] into a given folder. Returns the copy, if one was written.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn emergency_save_to(&mut self, folder: &std::path::Path) -> Option<PathBuf> {
        if self.state.quitting {
            return None; // the person decided (saved or discarded)
        }
        self.flush_draft();
        let doc = self.doc.as_ref()?;
        if !doc.is_dirty() {
            return None;
        }
        doc.write_recovery_copy(folder).ok()
    }

    /// Whether the app decided to close the window (after the question about unsaved changes).
    pub fn wants_to_quit(&self) -> bool {
        self.state.quitting
    }

    /// Draws one frame and carries out what the person did. Kept apart from [`eframe::App`] so
    /// tests can run it without a window.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        self.forget_vanished();
        let mut intents = Vec::new();
        if self.state.dialog.is_none() {
            views::shortcuts(ui.ctx(), self.doc.is_some(), &mut intents);
        }
        views::show(ui, self.doc.as_ref(), &mut self.state, &mut intents);
        for intent in intents {
            self.apply(intent);
        }
    }

    /// Drops the selection or view of things that no longer exist (deleted, undone).
    fn forget_vanished(&mut self) {
        let Some(doc) = &self.doc else { return };
        let ws = doc.session.workspace();
        if let Some(id) = self.state.selected
            && ws.task(id).is_none()
        {
            self.state.select(None);
        }
        if let View::Project(id) = self.state.view
            && ws.project(id).is_none()
        {
            self.state.view = View::Overview;
        }
    }

    fn apply(&mut self, intent: Intent) {
        match intent {
            Intent::Run {
                command,
                params,
                then,
                clear,
            } => self.run(command, &params, then, clear),
            Intent::Go(view) => {
                self.flush_draft();
                self.state.view = view;
                self.state.select(None);
                if matches!(view, View::Project(_)) {
                    self.state.tab = Tab::List;
                }
            }
            Intent::Select(task) => {
                if task != self.state.selected {
                    self.flush_draft();
                }
                self.state.select(task);
            }
            Intent::Open(dialog) => self.state.open_dialog(dialog),
            Intent::CloseDialog => self.state.close_dialog(),
            Intent::File(action) => self.file(action),
            Intent::Unlock(path, secret) => {
                self.state.close_dialog();
                self.open(path, Some(secret));
            }
            Intent::SaveThen(action) => {
                self.state.close_dialog();
                if self.save(false) {
                    self.state.force = true;
                    self.file(action);
                }
            }
            Intent::Discard(action) => {
                self.state.close_dialog();
                self.state.force = true;
                self.file(action);
            }
            Intent::Actor(person) => {
                if let Some(doc) = &mut self.doc {
                    doc.session.set_actor(person);
                }
            }
        }
    }

    fn run(&mut self, command: &str, params: &Value, then: Then, clear: Option<Clear>) {
        let Some(doc) = &mut self.doc else { return };
        match doc.session.run(command, params, Origin::User) {
            Ok(outcome) => {
                self.state.message = None;
                if let (Then::ShowProject, Some(EntityKey::Project(id))) =
                    (then, outcome.created.first())
                {
                    self.state.view = View::Project(*id);
                    self.state.tab = Tab::List;
                    self.state.select(None);
                }
                if let Some(clear) = clear {
                    self.state.clear(clear);
                }
            }
            Err(error) => self.state.message = Some(Message::error(text::command_error(&error))),
        }
    }

    /// Carries out a file action; actions that would drop unsaved changes ask first.
    fn file(&mut self, action: FileAction) {
        let force = std::mem::take(&mut self.state.force);
        // Choose the file first, then ask about unsaved changes – and not at all if it is the
        // file that is already open.
        if action == FileAction::Open {
            if let Some(path) = self.host.pick_open() {
                self.state.force = force;
                self.file(FileAction::OpenPath(path));
            }
            return;
        }
        if let FileAction::OpenPath(path) = &action
            && self.is_open(path)
        {
            self.state.message = Some(Message::info(text::ALREADY_OPEN));
            return;
        }
        // Typed text counts like any change: stored first, so saving includes it and leaving
        // asks about it. Not when the person chose to discard.
        let warning = if force { None } else { self.flush_draft() };
        if action.replaces_doc() && !force && self.has_unsaved_changes() {
            self.state.open_dialog(Dialog::Unsaved(action));
            return;
        }
        match action {
            FileAction::New => self.replace(OpenDoc::new()),
            FileAction::Sample => match OpenDoc::sample() {
                Ok(doc) => self.replace(doc),
                Err(error) => {
                    self.state.message = Some(Message::error(text::command_error(&error)));
                }
            },
            FileAction::Open => {}
            FileAction::OpenPath(path) => self.open(path, None),
            FileAction::Save => {
                self.save(false);
            }
            FileAction::SaveAs => {
                self.save(true);
            }
            FileAction::Close => {
                self.doc = None;
                self.state.reset_views();
            }
            FileAction::Quit => self.state.quitting = true,
        }
        // "Gespeichert." must not hide that a typed date was not taken.
        if let Some(warning) = warning {
            self.state.message = Some(Message::error(warning));
        }
    }

    fn replace(&mut self, doc: OpenDoc) {
        self.doc = Some(doc);
        self.state.reset_views();
        self.state.message = None;
    }

    /// Whether this path is the file that is open now.
    fn is_open(&self, path: &std::path::Path) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        let path = plnwrk_store::file::resolve(path).unwrap_or_else(|_| path.to_path_buf());
        #[cfg(target_arch = "wasm32")]
        let path = path.to_path_buf();
        self.doc.as_ref().and_then(OpenDoc::path) == Some(path.as_path())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn open(&mut self, path: PathBuf, passphrase: Option<SecretString>) {
        let retry = passphrase.is_some();
        match OpenDoc::open(&path, passphrase) {
            Ok(doc) => {
                if let Some(path) = doc.path() {
                    self.state.remember(path.to_path_buf(), RECENT_MAX);
                }
                self.replace(doc);
            }
            Err(StoreError::PassphraseRequired) => {
                self.state.open_dialog(Dialog::Passphrase {
                    path,
                    text: String::new(),
                });
            }
            Err(StoreError::WrongPassphrase) if retry => {
                self.state.open_dialog(Dialog::Passphrase {
                    path,
                    text: String::new(),
                });
                self.state.message = Some(Message::error(text::store_error(
                    &StoreError::WrongPassphrase,
                )));
            }
            Err(error) => self.state.message = Some(Message::error(text::store_error(&error))),
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn open(&mut self, _path: PathBuf, _passphrase: Option<SecretString>) {
        self.state.message = Some(Message::error(text::store_error(&StoreError::Io {
            reason: "Dateien öffnen folgt im Browser in Phase 4".to_owned(),
        })));
    }

    /// Saves (asking for a place if needed or wanted). Returns whether it was saved.
    #[cfg(not(target_arch = "wasm32"))]
    fn save(&mut self, choose_place: bool) -> bool {
        let Some(doc) = &mut self.doc else {
            return false;
        };
        let result = if choose_place || doc.path().is_none() {
            let name = format!("{}.plnwrk", doc.name());
            let Some(path) = self.host.pick_save(&name) else {
                return false;
            };
            doc.save_as(&path)
        } else {
            doc.save()
        };
        match result {
            Ok(()) => {
                if let Some(path) = doc.path() {
                    self.state.remember(path.to_path_buf(), RECENT_MAX);
                }
                self.state.message = Some(Message::info(text::SAVED));
                true
            }
            Err(error) => {
                self.state.message = Some(Message::error(text::store_error(&error)));
                false
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn save(&mut self, _choose_place: bool) -> bool {
        self.state.message = Some(Message::error(text::store_error(&StoreError::Io {
            reason: "Speichern folgt im Browser in Phase 4".to_owned(),
        })));
        false
    }

    /// The project shown, if a project view is open.
    pub fn shown_project(&self) -> Option<ProjectId> {
        match self.state.view {
            View::Project(id) => Some(id),
            _ => None,
        }
    }
}

impl eframe::App for PlnwrkApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if ctx.input(|i| i.viewport().close_requested()) && !self.close_requested() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        self.show(ui);
        let title = self.title();
        // The browser shows the page title of index.html; only desktop windows have titles.
        if title != self.shown_title && !cfg!(target_arch = "wasm32") {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.shown_title = title;
        }
        if self.state.quitting {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        #[cfg(not(target_arch = "wasm32"))]
        self.emergency_save();
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if let Ok(text) = serde_json::to_string(&self.state.recent) {
            storage.set_string(RECENT_KEY, text);
        }
    }
}

#[cfg(test)]
mod tests;
