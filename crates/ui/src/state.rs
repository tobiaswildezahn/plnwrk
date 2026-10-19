//! What the views keep between frames, apart from the data itself: where the person is, what
//! is selected, half-typed input and open dialogs.

use std::path::PathBuf;

use plnwrk_core::{PersonId, Project, ProjectId, ProjectStatus, Task, TaskId};
use serde_json::{Map, Value, json};
use zeroize::Zeroize as _;

use crate::Clear;
use crate::dates;

/// The main area.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum View {
    /// All projects and cases.
    #[default]
    Overview,
    /// One project or case (also a template).
    Project(ProjectId),
    /// Templates.
    Templates,
    /// People.
    People,
}

/// A column of the schedule's table beside the titles (Phase 9c).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScheduleColumn {
    Start,
    Due,
    Assignee,
}

impl ScheduleColumn {
    pub const ALL: [ScheduleColumn; 3] = [
        ScheduleColumn::Start,
        ScheduleColumn::Due,
        ScheduleColumn::Assignee,
    ];

    /// Its heading.
    pub fn name(self) -> &'static str {
        let t = crate::text::t();
        match self {
            ScheduleColumn::Start => t.start,
            ScheduleColumn::Due => t.due,
            ScheduleColumn::Assignee => t.assignee,
        }
    }

    /// Its width in points.
    pub fn width(self) -> f32 {
        match self {
            ScheduleColumn::Start | ScheduleColumn::Due => 88.0,
            ScheduleColumn::Assignee => 112.0,
        }
    }
}

/// What the ribbon asks of the schedule (Phase 9c).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleRequest {
    /// Show this zoom level (index, coarse to fine).
    Level(usize),
    /// Bring today into view.
    Today,
    /// Fit the whole project into view.
    Whole,
}

/// The parts of a project view.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Tab {
    /// Task tree.
    #[default]
    List,
    /// Kanban board.
    Board,
    /// Milestones.
    Milestones,
    /// Schedule (Gantt).
    Schedule,
}

/// Where a file to open comes from: a path (desktop) or an upload (browser).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileSource {
    Path(PathBuf),
    Upload(crate::host::Upload),
}

impl FileSource {
    /// How it is named for people: the path, or the file name.
    pub fn label(&self) -> String {
        match self {
            Self::Path(path) => path.display().to_string(),
            Self::Upload(upload) => upload.name.clone(),
        }
    }
}

/// Something to do with the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileAction {
    New,
    Sample,
    Open,
    OpenPath(PathBuf),
    /// A file chosen in the browser.
    Upload(crate::host::Upload),
    Save,
    SaveAs,
    Close,
    Quit,
}

impl FileAction {
    /// Whether it drops the open workspace (and so must ask about unsaved changes).
    pub const fn replaces_doc(&self) -> bool {
        matches!(
            self,
            Self::New
                | Self::Sample
                | Self::Open
                | Self::OpenPath(_)
                | Self::Upload(_)
                | Self::Close
                | Self::Quit
        )
    }
}

/// A modal question.
#[derive(Debug, Clone)]
pub enum Dialog {
    /// Create a project or case.
    NewProject(ProjectForm),
    /// Create a project or case from a template.
    FromTemplate(TemplateForm),
    /// Ask for the passphrase of an encrypted file.
    Passphrase { source: FileSource, text: String },
    /// Unsaved changes would be lost by this action.
    Unsaved(FileAction),
    /// Confirm deleting a task.
    DeleteTask(TaskId),
    /// A change of milestones, asked before it is made (Phase 9d): the command as it would
    /// run, and what it would change.
    ConfirmMilestone {
        command: &'static str,
        params: serde_json::Value,
        then: crate::Then,
        clear: Option<crate::Clear>,
        changes: Vec<plnwrk_engine::milestones::MilestoneChange>,
    },
    /// A task goes to "done" on the board while subtasks are open: close them too?
    /// `params` are those of the `task.move` (without `close_subtasks`), `open` how many.
    CloseSubtasks {
        task: TaskId,
        params: serde_json::Value,
        open: usize,
    },
    /// Protect the file with a passphrase (typed twice).
    Protect { first: String, second: String },
    /// Confirm removing the protection.
    Unprotect,
    /// Confirm deleting a recovery copy.
    DeleteCopy(PathBuf),
    /// Browser: confirm deleting the recovery copy under this key (and its name).
    DeleteStoredCopy { key: String, name: String },
    /// Browser: confirm deleting every recovery copy offered.
    DeleteAllStoredCopies,
    /// The command palette: what is typed and the highlighted line.
    Palette { query: String, selected: usize },
    /// Who works here? Asked once per device for a file with people (Phase 10a); `name` is a
    /// person being added.
    WhoAmI { name: String },
}

/// What the last action has to say: a refusal or a note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub text: String,
    pub error: bool,
    /// Where it came from, and so where it is said (set by [`State::say`]).
    pub source: Source,
}

impl Message {
    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            error: true,
            source: Source::View,
        }
    }

    pub fn info(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            error: false,
            source: Source::View,
        }
    }
}

/// The last message and where it stands; its fields are reached only through `State`.
#[derive(Debug, Default)]
pub struct Said {
    message: Option<Message>,
    /// The message was shown in its dialog (so it goes with it).
    seen: bool,
    /// The field of a [`Source::Field`] message was drawn this frame.
    field_drawn: bool,
}

/// Where a message came from (Phase 9g, STATUS 4.16): one message, one place. It is said
/// where it came from and nowhere else – and never lost on the way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The view: said in the message bar under the ribbon.
    View,
    /// The app as a whole (a recovery copy failing, an upload, typed text not taken on the way
    /// into a dialog): in the bar too, once no dialog covers it – and no command's success takes
    /// it away before it was seen (Nachreview 9g, NK1); the close button or the next message do.
    App,
    /// The open dialog: said in it (refusals). Once seen there it goes with the dialog; what it
    /// could not show any more comes to the bar – so none outlives its dialog.
    Dialog,
    /// A field, by an id of its own: said under it while it is drawn; when it is not (another
    /// task was chosen), in the bar.
    Field(egui::Id),
}

/// The fields of a project or case while they are typed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectForm {
    pub title: String,
    pub kind: String,
    pub reference: String,
    pub start: String,
    pub due: String,
    pub due_time: String,
    pub owner: Option<PersonId>,
    pub status: ProjectStatus,
    pub template: bool,
    /// A person being added right in the form ("Person anlegen…"): the name as typed.
    pub new_person: Option<String>,
}

fn optional(text: &str) -> Value {
    let text = text.trim();
    if text.is_empty() {
        Value::Null
    } else {
        Value::String(text.to_owned())
    }
}

impl ProjectForm {
    /// The form filled with a project.
    pub fn of(project: &Project) -> Self {
        Self {
            title: project.title.clone(),
            kind: project.kind.clone().unwrap_or_default(),
            reference: project.reference.clone().unwrap_or_default(),
            start: dates::format_opt(project.start),
            due: dates::format_opt(project.due),
            due_time: dates::format_time_opt(project.due_time),
            owner: project.owner,
            status: project.status,
            template: project.template,
            new_person: None,
        }
    }

    /// Every field as command parameters; empty fields are `null`. Wrong dates are refused.
    fn fields(&self) -> Result<Map<String, Value>, String> {
        let mut map = Map::new();
        map.insert("title".to_owned(), json!(self.title.trim()));
        map.insert("kind".to_owned(), optional(&self.kind));
        map.insert("reference".to_owned(), optional(&self.reference));
        map.insert("start".to_owned(), dates::param(dates::parse(&self.start)?));
        map.insert(
            "due".to_owned(),
            dates::deadline_param(dates::parse_deadline(&self.due, &self.due_time)?),
        );
        map.insert(
            "owner".to_owned(),
            self.owner.map_or(Value::Null, |id| json!(id.to_string())),
        );
        map.insert("template".to_owned(), json!(self.template));
        Ok(map)
    }

    /// Whether the form describes a case (Vorgang): the one rule, [`plnwrk_core::is_case`].
    pub fn is_case(&self) -> bool {
        plnwrk_core::is_case(Some(&self.kind), Some(&self.reference))
    }

    /// Parameters of `project.create` (empty fields left out).
    pub fn create_params(&self) -> Result<Value, String> {
        let mut map = self.fields()?;
        map.retain(|_, value| !value.is_null());
        Ok(Value::Object(map))
    }

    /// Whether something is typed that differs from `project` (dates compared as dates, so the
    /// notation of the language does not count). A wrong date counts as typed.
    pub fn is_pending(&self, project: &Project) -> bool {
        self.update_params(project.id) != Self::of(project).update_params(project.id)
    }

    /// Follows a change of the stored project (undo, archive) without losing what is typed:
    /// fields the person has not touched take the new value.
    pub fn rebase(&mut self, old: &Project, new: &Project) {
        let before = Self::of(old);
        let after = Self::of(new);
        let keep = |mine: &mut String, before: String, after: String| {
            if *mine == before {
                *mine = after;
            }
        };
        keep(&mut self.title, before.title, after.title);
        keep(&mut self.kind, before.kind, after.kind);
        keep(&mut self.reference, before.reference, after.reference);
        if dates::parse(&self.start) == Ok(old.start) {
            self.start = after.start;
        }
        if dates::parse_deadline(&self.due, &self.due_time)
            == Ok(old.due.map(|d| (d, old.due_time)))
        {
            self.due = after.due;
            self.due_time = after.due_time;
        }
        if self.owner == before.owner {
            self.owner = after.owner;
        }
        if self.status == before.status {
            self.status = after.status;
        }
        if self.template == before.template {
            self.template = after.template;
        }
    }

    /// Parameters of `project.update` (empty fields clear).
    pub fn update_params(&self, project: ProjectId) -> Result<Value, String> {
        let mut map = self.fields()?;
        map.insert("project".to_owned(), json!(project.to_string()));
        map.insert("status".to_owned(), json!(self.status.name()));
        Ok(Value::Object(map))
    }
}

/// The fields for creating from a template.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TemplateForm {
    pub template: Option<ProjectId>,
    pub title: String,
    pub reference: String,
    pub start: String,
    pub due: String,
    pub due_time: String,
    pub owner: Option<PersonId>,
}

impl TemplateForm {
    /// Parameters of `project.instantiate`.
    pub fn params(&self) -> Result<Value, String> {
        let template = self
            .template
            .ok_or_else(|| crate::text::t().choose_template.to_owned())?;
        let start =
            dates::parse(&self.start)?.ok_or_else(|| crate::text::t().start_needed.to_owned())?;
        let mut map = Map::new();
        map.insert("template".to_owned(), json!(template.to_string()));
        map.insert("title".to_owned(), json!(self.title.trim()));
        map.insert("start".to_owned(), dates::param(Some(start)));
        if let Some(due) = dates::parse_deadline(&self.due, &self.due_time)? {
            map.insert("due".to_owned(), dates::deadline_param(Some(due)));
        }
        if !self.reference.trim().is_empty() {
            map.insert("reference".to_owned(), json!(self.reference.trim()));
        }
        if let Some(owner) = self.owner {
            map.insert("owner".to_owned(), json!(owner.to_string()));
        }
        Ok(Value::Object(map))
    }
}

/// The text fields of the selected task while they are edited. Rebuilt whenever the stored
/// task changes, so undo and other edits show up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskDraft {
    /// The task as it was when the draft was made.
    pub source: Task,
    pub title: String,
    pub description: String,
    pub tags: String,
    pub start: String,
    pub due: String,
    pub due_time: String,
    pub progress: u8,
}

impl TaskDraft {
    /// Text typed but not stored yet, as parameters of `task.update` (without `task`).
    /// A wrong date is refused.
    pub fn changes(&self) -> Result<Map<String, Value>, String> {
        let stored = &self.source;
        let mut map = Map::new();
        if self.title.trim() != stored.title {
            map.insert("title".to_owned(), json!(self.title.trim()));
        }
        if self.description != stored.description {
            map.insert("description".to_owned(), json!(self.description));
        }
        if tags(&self.tags) != stored.tags {
            map.insert("tags".to_owned(), json!(tags(&self.tags)));
        }
        let start = dates::parse(&self.start)?;
        if start != stored.start {
            map.insert("start".to_owned(), dates::param(start));
        }
        let due = dates::parse_deadline(&self.due, &self.due_time)?;
        let stored_due = stored.due.map(|day| (day, stored.due_time));
        if due != stored_due {
            map.insert("due".to_owned(), dates::deadline_param(due));
        }
        Ok(map)
    }

    /// Puts back dates that are not valid, so they can never block saving or closing.
    /// Returns what was put back, for a message.
    pub fn drop_invalid_dates(&mut self) -> Option<String> {
        let mut dropped = Vec::new();
        if dates::parse(&self.start).is_err() {
            dropped.push(format!(
                "{} ({})",
                crate::text::quoted(self.start.trim()),
                crate::text::t().start
            ));
            self.start = dates::format_opt(self.source.start);
        }
        if dates::parse_deadline(&self.due, &self.due_time).is_err() {
            let typed = format!("{} {}", self.due.trim(), self.due_time.trim());
            dropped.push(format!(
                "{} ({})",
                crate::text::quoted(typed.trim()),
                crate::text::t().due
            ));
            self.due = dates::format_opt(self.source.due);
            self.due_time = dates::format_time_opt(self.source.due_time);
        }
        (!dropped.is_empty())
            .then(|| format!("{} {}", crate::text::t().invalid_dates, dropped.join(", ")))
    }

    /// Whether something was typed that is not stored yet.
    pub fn is_pending(&self) -> bool {
        self.changes().map_or(true, |map| !map.is_empty())
    }

    /// Follows a change of the stored task (undo, another field, a drag on the board) without
    /// losing what is being typed: fields the person has not touched take the new value.
    /// Dates count as untouched when they read as the stored date in any notation, so a change
    /// of language (`01.12.2026` → `2026-12-01`) does not make them look typed.
    pub fn rebase(&mut self, task: &Task) {
        let old = Self::of(&self.source);
        let new = Self::of(task);
        let keep = |mine: &mut String, before: &String, after: String| {
            if mine == before {
                *mine = after;
            }
        };
        keep(&mut self.title, &old.title, new.title);
        keep(&mut self.description, &old.description, new.description);
        keep(&mut self.tags, &old.tags, new.tags);
        if dates::parse(&self.start) == Ok(self.source.start) {
            self.start = new.start;
        }
        let stored_due = self.source.due.map(|day| (day, self.source.due_time));
        if dates::parse_deadline(&self.due, &self.due_time) == Ok(stored_due) {
            self.due = new.due;
            self.due_time = new.due_time;
        }
        if self.progress == old.progress {
            self.progress = new.progress;
        }
        self.source = task.clone();
    }

    pub fn of(task: &Task) -> Self {
        Self {
            source: task.clone(),
            title: task.title.clone(),
            description: task.description.clone(),
            tags: task.tags.join(", "),
            start: dates::format_opt(task.start),
            due: dates::format_opt(task.due),
            due_time: dates::format_time_opt(task.due_time),
            progress: task.progress,
        }
    }
}

/// Splits `"a, b ,c"` into tags.
pub fn tags(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect()
}

/// A new person while typed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PersonForm {
    pub name: String,
    pub role: String,
    pub initials: String,
    pub email: String,
}

impl PersonForm {
    /// Parameters of `person.create`.
    pub fn params(&self) -> Value {
        let mut map = Map::new();
        map.insert("name".to_owned(), json!(self.name.trim()));
        for (key, value) in [
            ("role", &self.role),
            ("initials", &self.initials),
            ("email", &self.email),
        ] {
            if !value.trim().is_empty() {
                map.insert(key.to_owned(), json!(value.trim()));
            }
        }
        Value::Object(map)
    }
}

/// Half-typed input of the views.
#[derive(Debug, Clone, Default)]
pub struct Forms {
    pub new_task: String,
    pub new_subtask: Option<(TaskId, String)>,
    /// A title being edited right in the list.
    pub inline: Option<(TaskId, String)>,
    pub detail_subtask: String,
    pub milestone_title: String,
    pub milestone_date: String,
    pub milestone_time: String,
    /// A new card being typed at the top of a board column: its project, status and title.
    pub board_new: Option<(plnwrk_core::ProjectId, plnwrk_core::TaskStatus, String)>,
    /// A milestone's date being changed on its card: the milestone, day and time as typed.
    pub milestone_edit: Option<(TaskId, String, String)>,
    pub comment: String,
    pub person: PersonForm,
    pub task: Option<TaskDraft>,
    /// The project being edited (as it was when the form opened) and the form.
    pub project: Option<(Project, ProjectForm)>,
}

/// Everything the views keep between frames.
#[derive(Debug, Default)]
pub struct State {
    pub view: View,
    pub tab: Tab,
    pub selected: Option<TaskId>,
    /// The last message: only through [`State::say`] and its kin, which know where it is said.
    pub said: Said,
    pub dialog: Option<Dialog>,
    pub recent: Vec<PathBuf>,
    /// Recovery copies written when the app ended without asking.
    pub recovered: Vec<PathBuf>,
    pub forms: Forms,
    /// The next file action may drop unsaved changes (the person said so).
    pub force: bool,
    /// The window is closing.
    pub quitting: bool,
    /// Counts opened dialogs, so each one can focus its first field once.
    pub dialog_serial: u64,
    /// How the recovery copy stands, for the status bar.
    pub recovery: Option<crate::autosave::Written>,
    /// The language of the interface.
    pub language: crate::text::Language,
    /// Light, dark or like the system.
    pub theme: egui::ThemePreference,
    /// The field for a new task takes the cursor in the next frame.
    pub focus_new_task: bool,
    /// Who works in which file on this device (Phase 10a): the file's identity → the person's
    /// id, or empty for "only I use this file". Kept in the app's settings.
    pub actors: std::collections::BTreeMap<String, String>,
    /// The title in the details takes the cursor in the next frame (a task chosen in the
    /// palette: the keyboard goes on there, not from the top of the window).
    pub focus_details: bool,
    /// The field of a new board card takes the focus in the next frame ("+" was chosen).
    pub focus_board_new: bool,
    /// The title of a new milestone, or the name of a new person, takes the focus next frame.
    pub focus_new_milestone: bool,
    pub focus_new_person: bool,
    /// A task just made from the list, lit up briefly so the eye finds it (Phase 9f).
    pub fresh: Option<TaskId>,
    /// Files are uploaded and downloaded (the browser).
    pub browser: bool,
    /// Browser: recovery copies in the browser's storage (key and file), newest first.
    pub stored_copies: Vec<crate::browser_copy::StoredCopy>,
    /// The tab of the ribbon shown.
    pub ribbon_tab: crate::actions::RibbonTab,
    /// The ribbon shows only its tabs: as the person chose, or (`None`) collapsed in the
    /// browser and in low windows.
    pub ribbon_collapsed: Option<bool>,
    /// Collapsed, but opened for a moment by a click on a tab.
    pub ribbon_peek: bool,
    /// Whether the ribbon starts collapsed, decided once from the window height (until the
    /// person chooses).
    pub ribbon_default: Option<bool>,
    /// What the ribbon asks of the schedule of a project, taken in the next frame.
    pub schedule_request: Option<(plnwrk_core::ProjectId, ScheduleRequest)>,
    /// The zoom level of each project's schedule (index into the levels of the schedule).
    pub zoom_levels: std::collections::HashMap<plnwrk_core::ProjectId, usize>,
    /// Columns of the schedule's table the person switched off.
    pub hidden_columns: std::collections::BTreeSet<ScheduleColumn>,
    /// Width of the title column of the schedule (the person can drag it); `None`: standard.
    pub title_width: Option<f32>,
    /// Tasks whose subtasks the schedule hides.
    pub folded: std::collections::BTreeSet<TaskId>,
    /// The schedule does not mark the critical path in red.
    pub hide_critical: bool,
    /// What the board of each project shows (Phase 9e; per project, Tobi 2026-10-17).
    pub board_filters: std::collections::BTreeMap<plnwrk_core::ProjectId, BoardFilter>,
    /// Cards on the board with their subtasks folded away.
    pub board_folded: std::collections::BTreeSet<TaskId>,
}

/// The filter above the board: who does it, only what is overdue, words in the title.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BoardFilter {
    /// `None`: everyone; `Some(None)`: nobody in charge; `Some(Some(id))`: this person.
    pub assignee: Option<Option<plnwrk_core::PersonId>>,
    pub overdue: bool,
    pub search: String,
}

impl BoardFilter {
    /// Whether anything is filtered at all.
    pub fn active(&self) -> bool {
        self.assignee.is_some() || self.overdue || !self.search.trim().is_empty()
    }

    /// Whether the card stays in view.
    pub fn keeps(&self, card: &plnwrk_engine::query::TaskCard) -> bool {
        self.keeps_parts(
            card.assignee.as_ref().map(|p| p.id),
            card.overdue,
            &card.title,
        )
    }

    /// Whether a new card with this title (nobody in charge, no deadline) would be in view.
    pub fn keeps_new(&self, title: &str) -> bool {
        self.keeps_parts(None, false, title)
    }

    fn keeps_parts(
        &self,
        person: Option<plnwrk_core::PersonId>,
        overdue: bool,
        title: &str,
    ) -> bool {
        let words = self.search.trim().to_lowercase();
        self.assignee.is_none_or(|wanted| wanted == person)
            && (!self.overdue || overdue)
            && (words.is_empty() || title.to_lowercase().contains(&words))
    }
}

impl State {
    /// Back to the overview with empty forms (after another file was opened).
    pub fn reset_views(&mut self) {
        self.view = View::Overview;
        self.tab = Tab::List;
        self.selected = None;
        self.close_dialog();
        self.forms = Forms::default();
        // Another file: its projects start unfiltered.
        self.board_filters.clear();
        self.board_folded.clear();
    }

    /// Shows a dialog.
    pub fn open_dialog(&mut self, dialog: Dialog) {
        self.close_dialog();
        self.dialog = Some(dialog);
        self.dialog_serial = self.dialog_serial.wrapping_add(1);
    }

    /// Closes the dialog; a typed passphrase is wiped from memory.
    pub fn close_dialog(&mut self) {
        match &mut self.dialog {
            Some(Dialog::Passphrase { text, .. }) => text.zeroize(),
            Some(Dialog::Protect { first, second }) => {
                first.zeroize();
                second.zeroize();
            }
            _ => {}
        }
        // What the dialog said goes with it once seen there; what it could not show any more
        // (said as it closed) is said in the bar instead.
        if let Some(message) = &mut self.said.message
            && message.source == Source::Dialog
        {
            if self.said.seen {
                self.said.message = None;
            } else {
                message.source = Source::View;
            }
        }
        self.dialog = None;
    }

    /// Says a message where it came from: in the open dialog, otherwise in the message bar.
    pub fn say(&mut self, mut message: Message) {
        message.source = match self.dialog {
            Some(_) => Source::Dialog,
            None => Source::View,
        };
        self.put(Some(message));
    }

    /// Says what concerns the whole app, not the open dialog (a recovery copy failing, an
    /// upload): in the bar, once no dialog covers it (review 9g, K1: said in a dialog as its
    /// refusal, it went with "Abbrechen").
    pub fn say_in_bar(&mut self, mut message: Message) {
        message.source = Source::App;
        self.put(Some(message));
    }

    /// Says a message under a field (`field` names it, see [`Source::Field`]); the field shows
    /// it with [`State::said_at`].
    pub fn say_at(&mut self, field: egui::Id, mut message: Message) {
        message.source = Source::Field(field);
        self.put(Some(message));
    }

    /// Says the message, if any, where it came from – or nothing (the last one is gone).
    pub fn say_maybe(&mut self, message: Option<Message>) {
        match message {
            Some(message) => self.say(message),
            None => self.put(None),
        }
    }

    /// A command was done with nothing to say: the last message goes – except a field's,
    /// which stands while its field still shows what could not be read (review 9g, K2; the
    /// field takes it back itself once it reads), and one of the app's (NK1).
    pub fn done(&mut self) {
        if !self
            .said
            .message
            .as_ref()
            .is_some_and(|m| matches!(m.source, Source::Field(_) | Source::App))
        {
            self.put(None);
        }
    }

    /// The last message is gone (closed, or about what was left).
    pub fn unsay(&mut self) {
        self.put(None);
    }

    /// The field was left on purpose ("Abbrechen"): what it said goes with it.
    pub fn unsay_at(&mut self, field: egui::Id) {
        if self
            .said
            .message
            .as_ref()
            .is_some_and(|m| m.source == Source::Field(field))
        {
            self.put(None);
        }
    }

    /// Takes the message away to put it back later with [`State::put_back`].
    pub fn take_message(&mut self) -> Option<Message> {
        self.said.seen = false;
        self.said.message.take()
    }

    /// Puts a taken message back, from where it came.
    pub fn put_back(&mut self, message: Option<Message>) {
        self.put(message);
    }

    fn put(&mut self, message: Option<Message>) {
        self.said.message = message;
        self.said.seen = false;
    }

    /// The last message, wherever it is said.
    pub fn message(&self) -> Option<&Message> {
        self.said.message.as_ref()
    }

    /// The refusal the open dialog says: one that came up in it, never one from before.
    pub fn said_in_dialog(&mut self) -> Option<Message> {
        let message = self
            .said
            .message
            .clone()
            .filter(|m| m.error && self.dialog.is_some() && m.source == Source::Dialog);
        if message.is_some() {
            self.said.seen = true;
        }
        message
    }

    /// The message for the bar under the ribbon: one from a view, while no dialog is open.
    pub fn said_in_bar(&self) -> Option<&Message> {
        self.said
            .message
            .as_ref()
            .filter(|m| matches!(m.source, Source::View | Source::App) && self.dialog.is_none())
    }

    /// The message said under the field `field`; drawing it counts as shown.
    pub fn said_at(&mut self, field: egui::Id) -> Option<Message> {
        let message = self
            .said
            .message
            .clone()
            .filter(|m| m.source == Source::Field(field));
        if message.is_some() {
            self.said.field_drawn = true;
        }
        message
    }

    /// At the start of a frame: no field has shown its message yet.
    pub fn frame_begins(&mut self) {
        self.said.field_drawn = false;
    }

    /// At the end of a frame: a field message whose field was not drawn (another task chosen,
    /// the form closed) goes to the bar – never lost. Whether the bar must be drawn again.
    pub fn frame_ends(&mut self) -> bool {
        match &mut self.said.message {
            Some(message)
                if matches!(message.source, Source::Field(_)) && !self.said.field_drawn =>
            {
                message.source = Source::View;
                true
            }
            _ => false,
        }
    }

    /// Selects a task (or none). Input meant for the previous task is dropped, so it never
    /// lands on another one.
    pub fn select(&mut self, task: Option<TaskId>) {
        if task != self.selected {
            self.forms.comment.clear();
            self.forms.detail_subtask.clear();
            self.forms.task = None;
        }
        self.selected = task;
    }

    /// Empties a form after its command succeeded.
    pub fn clear(&mut self, clear: Clear) {
        match clear {
            Clear::NewTask => self.forms.new_task.clear(),
            Clear::BoardNew => self.forms.board_new = None,
            Clear::NewSubtask => self.forms.new_subtask = None,
            Clear::Inline => self.forms.inline = None,
            Clear::DetailSubtask => self.forms.detail_subtask.clear(),
            Clear::Milestone => {
                self.forms.milestone_title.clear();
                self.forms.milestone_date.clear();
                self.forms.milestone_time.clear();
            }
            Clear::Comment => self.forms.comment.clear(),
            Clear::Person => self.forms.person = PersonForm::default(),
            Clear::Dialog => self.close_dialog(),
            Clear::ProjectEdit => self.forms.project = None,
        }
    }

    /// Puts a file first in the list of recent files.
    pub fn remember(&mut self, path: PathBuf, max: usize) {
        self.recent.retain(|p| p != &path);
        self.recent.insert(0, path);
        self.recent.truncate(max);
    }
}

#[cfg(test)]
mod tests {
    fn project(title: &str, due: Option<&str>) -> Project {
        let mut doc = crate::OpenDoc::new();
        doc.session
            .run(
                "project.create",
                &json!({"title": title, "due": due}),
                plnwrk_engine::Origin::User,
            )
            .unwrap();
        doc.session.workspace().projects().next().unwrap().clone()
    }

    #[test]
    fn a_project_form_keeps_what_is_typed_and_follows_the_rest() {
        let old = project("Umzug", Some("2026-12-01"));
        let mut new = old.clone();
        new.title = "Umzug alt".to_owned();
        new.due = None;
        let mut form = ProjectForm::of(&old);
        form.title = "Umzug NEU".to_owned();
        assert!(form.is_pending(&old));
        form.rebase(&old, &new);
        assert_eq!(form.title, "Umzug NEU", "typed");
        assert_eq!(form.due, "", "untouched: follows the undo");
    }

    use super::*;

    #[test]
    fn tags_are_split_and_trimmed() {
        assert_eq!(tags(" a, b ,,c "), vec!["a", "b", "c"]);
        assert!(tags("  ").is_empty());
    }

    #[test]
    fn project_forms_refuse_wrong_dates_and_clear_empty_fields() {
        let mut form = ProjectForm {
            title: " Umzug ".to_owned(),
            due: "31.12.2026".to_owned(),
            ..ProjectForm::default()
        };
        assert_eq!(
            form.create_params().unwrap(),
            json!({"title": "Umzug", "due": "2026-12-31", "template": false})
        );
        let id: ProjectId = "01K00000000000000000000001".parse().unwrap();
        let update = form.update_params(id).unwrap();
        assert_eq!(update["kind"], Value::Null);
        assert_eq!(update["status"], json!("active"));
        form.due = "31.13.2026".to_owned();
        assert!(form.create_params().is_err());
    }

    #[test]
    fn template_forms_need_template_and_start() {
        let mut form = TemplateForm {
            title: "SKA 23/1".to_owned(),
            ..TemplateForm::default()
        };
        assert!(form.params().is_err());
        form.template = Some("01K00000000000000000000001".parse().unwrap());
        assert!(form.params().is_err());
        form.start = "1.11.2026".to_owned();
        form.reference = "23/1".to_owned();
        let params = form.params().unwrap();
        assert_eq!(params["start"], json!("2026-11-01"));
        assert_eq!(params["reference"], json!("23/1"));
        assert!(params.get("due").is_none());
    }

    #[test]
    fn recent_files_are_unique_newest_first_and_bounded() {
        let mut state = State::default();
        for name in ["a", "b", "a", "c"] {
            state.remember(PathBuf::from(name), 2);
        }
        assert_eq!(state.recent, vec![PathBuf::from("c"), PathBuf::from("a")]);
    }
}
