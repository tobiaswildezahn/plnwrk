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

mod autosave;
mod dates;
mod doc;
mod host;
mod palette;
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

/// Whether two paths name the same place on disk (links resolved, e.g. `/var` and
/// `/private/var` on macOS).
fn same_place(a: &std::path::Path, b: &std::path::Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// Key of the list of recently opened files in the app's storage.
const RECENT_KEY: &str = "plnwrk.recent";
/// Key of the language of the interface in the app's storage.
const LANGUAGE_KEY: &str = "plnwrk.language";
/// Key of the theme (light, dark, system) in the app's storage.
const THEME_KEY: &str = "plnwrk.theme";
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
    Inline,
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
    /// Protect the file with this passphrase (and save).
    Protect(SecretString),
    /// Remove the protection (and save).
    Unprotect,
    /// Delete a recovery copy for good.
    DeleteCopy(PathBuf),
    /// Use this language for the interface.
    Language(text::Language),
    /// Use this theme.
    Theme(egui::ThemePreference),
    /// Show this part of the project view.
    Tab(Tab),
    /// Put the cursor into the field for a new task.
    FocusNewTask,
    /// Open the form to edit this project.
    EditProject(ProjectId),
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
    /// Recovery copies of the open workspace.
    autosave: autosave::Autosave,
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
        if let Some(storage) = cc.storage {
            app.load_settings(storage);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(folder) = doc::recovery_folder() {
            app.state.recovered = doc::recovery_copies(&folder);
            app.autosave = autosave::Autosave::for_app(folder);
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
            autosave: autosave::Autosave::default(),
        }
    }

    /// Takes the settings kept from last time: recent files, language, theme.
    fn load_settings(&mut self, storage: &dyn eframe::Storage) {
        if let Some(text) = storage.get_string(RECENT_KEY) {
            self.state.recent = serde_json::from_str(&text).unwrap_or_default();
        }
        if let Some(language) = storage
            .get_string(LANGUAGE_KEY)
            .and_then(|code| text::Language::from_code(&code))
        {
            self.set_language(language);
        }
        if let Some(theme) = storage.get_string(THEME_KEY).and_then(|s| theme_from(&s)) {
            self.state.theme = theme;
        }
    }

    /// Uses this language for the interface (also for texts made before the next frame).
    pub fn set_language(&mut self, language: text::Language) {
        self.state.language = language;
        text::set_language(language);
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
                    .is_some_and(state::TaskDraft::is_pending)
                || self.inline_pending().is_some()
                || self.project_pending().is_some())
    }

    /// A project form with typed changes: the project and the form, brought up to date with
    /// the stored project first (an undo while the form was out of sight must not count).
    fn project_pending(&self) -> Option<(ProjectId, state::ProjectForm)> {
        let (source, form) = self.state.forms.project.as_ref()?;
        let project = self.doc.as_ref()?.session.workspace().project(source.id)?;
        let mut form = form.clone();
        form.rebase(source, project);
        form.is_pending(project).then(|| (project.id, form))
    }

    /// Brings an open project form up to date with the stored project (undo, archive …) –
    /// also while another view is shown: untouched fields follow, typed ones stay. A form of
    /// a project that is gone is closed.
    fn sync_project_form(&mut self) {
        let Some((source, form)) = &mut self.state.forms.project else {
            return;
        };
        match self
            .doc
            .as_ref()
            .and_then(|d| d.session.workspace().project(source.id))
        {
            Some(project) if project != source => {
                form.rebase(source, project);
                *source = project.clone();
            }
            Some(_) => {}
            None => self.state.forms.project = None,
        }
    }

    /// A title typed in the list and not taken yet: the task and the new title.
    fn inline_pending(&self) -> Option<(TaskId, String)> {
        let (id, text) = self.state.forms.inline.as_ref()?;
        let task = self.doc.as_ref()?.session.workspace().task(*id)?;
        let text = text.trim();
        (!text.is_empty() && text != task.title).then(|| (*id, text.to_owned()))
    }

    /// Stores what is typed in the detail panel but not committed yet (the field still has the
    /// focus). Never blocks: what cannot be stored (a wrong date, an empty title) is put back
    /// to the stored value and named in a message, so saving and closing always work.
    /// Returns the warning about what was put back, if anything.
    fn flush_draft(&mut self) -> Option<String> {
        // What could not be taken, named in one message at the end (a later success must not
        // hide an earlier refusal).
        let mut refused: Vec<String> = Vec::new();
        // A title being renamed in the list counts like any typed text.
        if let Some((id, title)) = self.inline_pending() {
            self.run(
                "task.update",
                &serde_json::json!({"task": id.to_string(), "title": title}),
                Then::Nothing,
                Some(Clear::Inline),
            );
            if let Some(message) = self.state.message.as_ref().filter(|m| m.error) {
                refused.push(format!("{} {}", text::t().not_taken, message.text));
            }
        }
        self.state.forms.inline = None;
        // A half-edited project form counts too: taken like "Übernehmen", or put back and named.
        if let Some((id, form)) = self.project_pending() {
            match form.update_params(id) {
                Ok(params) => {
                    self.run(
                        "project.update",
                        &params,
                        Then::Nothing,
                        Some(Clear::ProjectEdit),
                    );
                    if let Some(message) = self.state.message.as_ref().filter(|m| m.error) {
                        refused.push(format!("{} {}", text::t().not_taken, message.text));
                        self.state.forms.project = None;
                    }
                }
                Err(reason) => {
                    refused.push(format!("{} {reason}", text::t().not_taken));
                    self.state.forms.project = None;
                }
            }
        }
        let project_warning = (!refused.is_empty()).then(|| refused.join(" · "));
        if let Some(text) = &project_warning {
            self.state.message = Some(Message::error(text.clone()));
        }
        let Some(doc) = self.doc.as_ref() else {
            return project_warning;
        };
        let Some(mut draft) = self.state.forms.task.take() else {
            return project_warning;
        };
        let Some(task) = doc.session.workspace().task(draft.source.id).cloned() else {
            return project_warning;
        };
        let mut warnings: Vec<String> = project_warning.into_iter().collect();
        warnings.extend(draft.drop_invalid_dates());
        let changes = draft.changes().unwrap_or_default();
        self.state.forms.task = Some(draft);
        if !changes.is_empty() {
            let mut params = changes;
            params.insert("task".to_owned(), Value::String(task.id.to_string()));
            self.run("task.update", &Value::Object(params), Then::Nothing, None);
            if let Some(message) = self.state.message.as_ref().filter(|m| m.error) {
                // Refused (e.g. empty title): back to what is stored, and say so.
                warnings.push(format!("{} {}", text::t().not_taken, message.text));
                self.state.forms.task = Some(state::TaskDraft::of(&task));
            } else if let Some(stored) = self
                .doc
                .as_ref()
                .and_then(|d| d.session.workspace().task(task.id))
                && let Some(draft) = &mut self.state.forms.task
            {
                // Taken: the draft now matches what is stored, also before the next frame.
                draft.rebase(stored);
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
    pub fn emergency_save(&mut self) -> Option<PathBuf> {
        if self.state.quitting {
            return None; // the person decided (saved or discarded)
        }
        self.flush_draft();
        let doc = self.doc.as_ref()?;
        if !doc.is_dirty() {
            return None;
        }
        self.autosave.write_now(doc)
    }

    /// Uses this autosave (e.g. another folder or interval, for tests).
    pub fn set_autosave(&mut self, folder: Option<PathBuf>, background: bool, interval: f64) {
        self.autosave = autosave::Autosave::new(folder, background, interval);
    }

    /// Whether the app decided to close the window (after the question about unsaved changes).
    pub fn wants_to_quit(&self) -> bool {
        self.state.quitting
    }

    /// Draws one frame and carries out what the person did. Kept apart from [`eframe::App`] so
    /// tests can run it without a window.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        text::set_language(self.state.language);
        if ui.ctx().options(|o| o.theme_preference) != self.state.theme {
            ui.ctx().set_theme(self.state.theme);
        }
        self.forget_vanished();
        let mut intents = Vec::new();
        if self.state.dialog.is_none() {
            views::shortcuts(ui.ctx(), self.doc.is_some(), &mut intents);
        }
        views::show(ui, self.doc.as_ref(), &mut self.state, &mut intents);
        for intent in intents {
            self.apply(intent);
        }
        self.autosave_tick(ui.ctx());
    }

    /// Lets the autosave write when due, and wakes the app up again when the next copy is due –
    /// also while the window is hidden (`eframe::App::logic`).
    fn autosave_tick(&mut self, ctx: &egui::Context) {
        if let Some(wait) = self.autosave_at(ctx.input(|i| i.time)) {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(wait.max(0.1)));
        }
    }

    /// The autosave at this egui time (seconds); how long until it is due again, if it is.
    fn autosave_at(&mut self, time: f64) -> Option<f64> {
        let before = self.autosave.last().cloned();
        let wait = self.autosave.tick(self.doc.as_ref(), time);
        let after = self.autosave.last().cloned();
        // Said once when it starts failing, without hiding another error that is shown.
        if after != before
            && let Some(autosave::Written::Failed(reason)) = &after
            && self.state.message.as_ref().is_none_or(|m| !m.error)
        {
            self.state.message = Some(Message::error(format!(
                "{} {reason}",
                text::t().recovery_failed
            )));
        }
        self.state.recovery = after;
        wait
    }

    /// Drops the selection or view of things that no longer exist (deleted, undone).
    fn forget_vanished(&mut self) {
        self.sync_project_form();
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
            Intent::Protect(secret) => {
                self.state.close_dialog();
                self.change_protection(
                    plnwrk_store::Protection::Passphrase(secret),
                    text::t().protected,
                );
            }
            Intent::DeleteCopy(path) => {
                self.state.close_dialog();
                #[cfg(not(target_arch = "wasm32"))]
                self.delete_copy(&path);
                #[cfg(target_arch = "wasm32")]
                let _ = path;
            }
            Intent::Language(language) => {
                self.set_language(language);
                // Untouched dates being edited follow the new notation; typed ones stay.
                if let Some(draft) = &mut self.state.forms.task {
                    let source = draft.source.clone();
                    draft.rebase(&source);
                }
                if let Some((source, form)) = &mut self.state.forms.project {
                    form.rebase(source, source);
                }
            }
            Intent::Theme(theme) => self.state.theme = theme,
            Intent::Tab(tab) => self.state.tab = tab,
            Intent::FocusNewTask => self.state.focus_new_task = true,
            Intent::EditProject(id) => {
                let open = self
                    .state
                    .forms
                    .project
                    .as_ref()
                    .is_some_and(|(p, _)| p.id == id);
                if !open
                    && let Some(project) = self
                        .doc
                        .as_ref()
                        .and_then(|d| d.session.workspace().project(id))
                {
                    self.state.forms.project =
                        Some((project.clone(), state::ProjectForm::of(project)));
                }
            }
            Intent::Unprotect => {
                // VS-NfD stays encrypted: saving without protection is refused by the store,
                // and the old protection is put back.
                self.state.close_dialog();
                self.change_protection(plnwrk_store::Protection::None, text::t().unprotected);
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
            self.state.message = Some(Message::info(text::t().already_open));
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
                self.drop_copy();
                self.doc = None;
                self.state.reset_views();
            }
            FileAction::Quit => {
                // Saved or discarded on purpose: no recovery copy is needed.
                self.drop_copy();
                self.state.quitting = true;
            }
        }
        // "Gespeichert." must not hide that a typed date was not taken.
        if let Some(warning) = warning {
            self.state.message = Some(Message::error(warning));
        }
    }

    /// The open work is dropped on purpose: its copy goes; a recovered copy is offered again.
    fn drop_copy(&mut self) {
        if let Some(kept) = self.autosave.discard() {
            self.state.recovered.retain(|p| !same_place(p, &kept));
            self.state.recovered.insert(0, kept);
        }
    }

    /// Deletes a recovery copy for good (asked first). Only files in the recovery folder.
    #[cfg(not(target_arch = "wasm32"))]
    fn delete_copy(&mut self, path: &std::path::Path) {
        let inside = self
            .autosave
            .folder()
            .is_some_and(|folder| path.parent().is_some_and(|p| same_place(p, folder)));
        if inside {
            match std::fs::remove_file(path) {
                Ok(()) => self.state.recovered.retain(|p| !same_place(p, path)),
                Err(e) => {
                    self.state.message = Some(Message::error(text::store_error(&StoreError::Io {
                        reason: e.to_string(),
                    })));
                }
            }
        }
    }

    /// Saves with a new protection; if that fails or is cancelled, the old one stays.
    fn change_protection(&mut self, protection: plnwrk_store::Protection, done: &str) {
        let warning = self.flush_draft();
        let Some(doc) = &mut self.doc else { return };
        let old = doc.set_protection(protection);
        if self.save(false) {
            // A typed date that was not taken is the more important news.
            self.state.message = Some(match warning {
                Some(warning) => Message::error(warning),
                None => Message::info(done),
            });
        } else {
            if let Some(doc) = &mut self.doc {
                doc.restore_protection(old);
            }
            if self.state.message.as_ref().is_none_or(|m| !m.error) {
                self.state.message = Some(Message::info(text::t().protection_unchanged));
            }
        }
    }

    fn replace(&mut self, doc: OpenDoc) {
        self.drop_copy();
        self.state.message = doc.upgraded.then(|| Message::info(text::t().upgraded));
        self.doc = Some(doc);
        self.state.reset_views();
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
            Ok(mut doc) => {
                // A recovery copy opens as an unsaved workspace: saving asks for a place, and
                // the copy goes once the work is saved somewhere.
                let recovered = self
                    .autosave
                    .folder()
                    .is_some_and(|folder| path.parent().is_some_and(|p| same_place(p, folder)));
                let copy = if recovered {
                    doc.detach_as_untitled()
                } else {
                    None
                };
                if let Some(path) = doc.path() {
                    self.state.remember(path.to_path_buf(), RECENT_MAX);
                }
                self.replace(doc);
                if let Some(copy) = copy {
                    self.state.recovered.retain(|p| !same_place(p, &copy));
                    self.autosave.adopt(copy);
                    self.state.message = Some(Message::info(text::t().recovered_opened));
                }
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
            reason: text::t().browser_files_later.to_owned(),
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
                self.autosave.saved();
                self.state.message = Some(Message::info(text::t().saved));
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
            reason: text::t().browser_files_later.to_owned(),
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

/// The stored name of a theme.
const fn theme_code(theme: egui::ThemePreference) -> &'static str {
    match theme {
        egui::ThemePreference::System => "system",
        egui::ThemePreference::Light => "light",
        egui::ThemePreference::Dark => "dark",
    }
}

/// The theme with this stored name.
fn theme_from(code: &str) -> Option<egui::ThemePreference> {
    [
        egui::ThemePreference::System,
        egui::ThemePreference::Light,
        egui::ThemePreference::Dark,
    ]
    .into_iter()
    .find(|t| theme_code(*t) == code)
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

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Called also while the window is minimised or covered, when `ui` is not.
        self.autosave_tick(ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if let Ok(text) = serde_json::to_string(&self.state.recent) {
            storage.set_string(RECENT_KEY, text);
        }
        storage.set_string(LANGUAGE_KEY, self.state.language.code().to_owned());
        storage.set_string(THEME_KEY, theme_code(self.state.theme).to_owned());
    }
}

#[cfg(test)]
mod model_tests;
#[cfg(test)]
mod tests;
