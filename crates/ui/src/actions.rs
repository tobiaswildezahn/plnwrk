//! Every action of the interface in one list (Phase 9b): the ribbon shows them by tab and
//! group, the command palette lists them with their place ("Start › Neu"). One source, so the
//! two never disagree and a new action cannot be forgotten in either.
//!
//! An action runs only [`Intent`]s – the same ones the views use – so questions (unsaved
//! changes, deleting) still come. docs/plans/2026-10-17-phase-9-gestaltung.md §6.2.

use egui::KeyboardShortcut;
use plnwrk_core::{ProjectStatus, TaskStatus};
use serde_json::json;

use crate::design::Icon;
use crate::doc::OpenDoc;
use crate::state::{Dialog, FileAction, ProjectForm, State, Tab, TemplateForm, View};
use crate::views::keys;
use crate::{Intent, text};

/// A tab of the ribbon. `Task` and `Schedule` are contextual: shown only while a task is
/// chosen or the schedule is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RibbonTab {
    File,
    #[default]
    Start,
    View,
    Task,
    Schedule,
}

impl RibbonTab {
    #[cfg(test)]
    pub const ALL: [RibbonTab; 5] = [
        RibbonTab::File,
        RibbonTab::Start,
        RibbonTab::View,
        RibbonTab::Task,
        RibbonTab::Schedule,
    ];

    pub fn name(self) -> &'static str {
        let t = text::t();
        match self {
            RibbonTab::File => t.file_menu,
            RibbonTab::Start => t.ribbon.start,
            RibbonTab::View => t.view_menu,
            RibbonTab::Task => t.ribbon.task,
            // Not "Zeitplan": that is the name of the view (tab of the project) already.
            RibbonTab::Schedule => t.ribbon.schedule_tools,
        }
    }

    /// Shown only in a context (a chosen task, the schedule), set apart in colour.
    pub fn contextual(self) -> bool {
        matches!(self, RibbonTab::Task | RibbonTab::Schedule)
    }
}

/// A group of a tab, in the order shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Group {
    FileNew,
    FileSave,
    FileProtect,
    FileClose,
    New,
    Undo,
    Find,
    Views,
    Areas,
    Appearance,
    Language,
    TaskStatus,
    TaskEdit,
    Zoom,
    Share,
}

impl Group {
    pub const ALL: [Group; 15] = [
        Group::FileNew,
        Group::FileSave,
        Group::FileProtect,
        Group::FileClose,
        Group::New,
        Group::Undo,
        Group::Find,
        Group::Views,
        Group::Areas,
        Group::Appearance,
        Group::Language,
        Group::TaskStatus,
        Group::TaskEdit,
        Group::Zoom,
        Group::Share,
    ];

    pub fn tab(self) -> RibbonTab {
        match self {
            Group::FileNew | Group::FileSave | Group::FileProtect | Group::FileClose => {
                RibbonTab::File
            }
            Group::New | Group::Undo | Group::Find => RibbonTab::Start,
            Group::Views | Group::Areas | Group::Appearance | Group::Language => RibbonTab::View,
            Group::TaskStatus | Group::TaskEdit => RibbonTab::Task,
            Group::Zoom | Group::Share => RibbonTab::Schedule,
        }
    }

    pub fn name(self) -> &'static str {
        let t = text::t();
        let r = &t.ribbon;
        match self {
            Group::FileNew => r.new,
            Group::FileSave => t.save,
            Group::FileProtect => r.protection,
            Group::FileClose => r.close,
            Group::New => r.new,
            Group::Undo => r.undo,
            Group::Find => r.find,
            Group::Views => r.views,
            Group::Areas => r.areas,
            Group::Appearance => t.appearance,
            Group::Language => t.language_menu,
            Group::TaskStatus => t.status,
            Group::TaskEdit => r.edit,
            Group::Zoom => r.zoom,
            Group::Share => r.share,
        }
    }

    /// Where the group is, as the palette shows it: "Start › Neu".
    pub fn place(self) -> String {
        format!("{} › {}", self.tab().name(), self.name())
    }
}

/// How large an entry is drawn: a large button (symbol over text) or a small one (symbol and
/// text in a row, three above each other).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    Large,
    Small,
}

/// One thing the person can do.
#[derive(Debug)]
pub struct Action {
    /// What it does (short, under a ribbon button).
    pub label: String,
    /// The full name where the group does not stand beside it (palette, tooltip).
    pub long: Option<String>,
    pub icon: Icon,
    pub group: Group,
    pub size: Size,
    pub shortcut: Option<KeyboardShortcut>,
    /// Commands of the register it runs, or whose place it opens (test: every command of the
    /// register is reachable).
    pub commands: &'static [&'static str],
    /// What happens when it is chosen.
    pub intents: Vec<Intent>,
    /// Possible now; otherwise the ribbon shows it greyed and the palette leaves it out.
    pub enabled: bool,
    /// The current choice of a group (view, theme, language): marked in the ribbon, left out
    /// of the palette.
    pub chosen: bool,
}

/// Collects actions in their order.
struct Actions(Vec<Action>);

impl Actions {
    fn push(&mut self, action: Action) {
        self.0.push(action);
    }
}

/// An action with sensible defaults: small, enabled, not chosen.
fn act(group: Group, icon: Icon, label: impl Into<String>, intents: Vec<Intent>) -> Action {
    Action {
        label: label.into(),
        long: None,
        icon,
        group,
        size: Size::Small,
        shortcut: None,
        commands: &[],
        intents,
        enabled: true,
        chosen: false,
    }
}

impl Action {
    fn large(mut self) -> Self {
        self.size = Size::Large;
        self
    }

    fn keys(mut self, shortcut: KeyboardShortcut) -> Self {
        self.shortcut = Some(shortcut);
        self
    }

    fn runs(mut self, commands: &'static [&'static str]) -> Self {
        self.commands = commands;
        self
    }

    fn when(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    fn chosen(mut self, chosen: bool) -> Self {
        self.chosen = chosen;
        self
    }

    fn long(mut self, long: impl Into<String>) -> Self {
        self.long = Some(long.into());
        self
    }

    /// The name to show without the group beside it.
    pub fn full_name(&self) -> &str {
        self.long.as_deref().unwrap_or(&self.label)
    }
}

/// The position in `all` of the `nth` action of a group.
pub fn index_of(all: &[Action], group: Group, nth: usize) -> Option<usize> {
    all.iter()
        .enumerate()
        .filter(|(_, a)| a.group == group)
        .map(|(i, _)| i)
        .nth(nth)
}

/// The tabs to show now: the fixed ones, and the contextual ones that apply.
pub fn tabs(doc: Option<&OpenDoc>, state: &State) -> Vec<RibbonTab> {
    let mut tabs = vec![RibbonTab::File, RibbonTab::Start, RibbonTab::View];
    let Some(doc) = doc else { return tabs };
    let ws = doc.session.workspace();
    if state.selected.and_then(|id| ws.task(id)).is_some() {
        tabs.push(RibbonTab::Task);
    }
    if matches!(state.view, View::Project(_)) && state.tab == Tab::Schedule {
        tabs.push(RibbonTab::Schedule);
    }
    tabs
}

/// Everything that can be done now, in the order of tabs and groups.
pub fn all(doc: Option<&OpenDoc>, state: &State) -> Vec<Action> {
    let t = text::t();
    let r = &t.ribbon;
    let mut a = Actions(Vec::new());
    let open = doc.is_some();

    // Datei
    a.push(
        act(
            Group::FileNew,
            Icon::Plus,
            t.new_file,
            vec![Intent::File(FileAction::New)],
        )
        .large(),
    );
    a.push(
        act(
            Group::FileNew,
            Icon::Open,
            t.open_file,
            vec![Intent::File(FileAction::Open)],
        )
        .large()
        .keys(keys::OPEN),
    );
    a.push(act(
        Group::FileNew,
        Icon::Case,
        t.open_sample,
        vec![Intent::File(FileAction::Sample)],
    ));
    a.push(
        act(
            Group::FileSave,
            Icon::Save,
            t.save,
            vec![Intent::File(FileAction::Save)],
        )
        .large()
        .keys(keys::SAVE)
        .when(open),
    );
    a.push(
        act(
            Group::FileSave,
            Icon::Save,
            t.save_as,
            vec![Intent::File(FileAction::SaveAs)],
        )
        .keys(keys::SAVE_AS)
        .when(open),
    );
    let encrypted = doc.is_some_and(OpenDoc::encrypted);
    if encrypted {
        a.push(
            act(
                Group::FileProtect,
                Icon::Settings,
                t.unprotect,
                vec![Intent::Open(Dialog::Unprotect)],
            )
            .when(open),
        );
    } else {
        let protect = Dialog::Protect {
            first: String::new(),
            second: String::new(),
        };
        a.push(
            act(
                Group::FileProtect,
                Icon::Settings,
                t.protect,
                vec![Intent::Open(protect)],
            )
            .when(open),
        );
    }
    a.push(
        act(
            Group::FileClose,
            Icon::Close,
            t.close_file,
            vec![Intent::File(FileAction::Close)],
        )
        .when(open),
    );
    // A browser tab is not quit from inside the page.
    if !state.browser {
        a.push(
            act(
                Group::FileClose,
                Icon::Close,
                t.quit,
                vec![Intent::File(FileAction::Quit)],
            )
            .keys(keys::QUIT),
        );
    }

    // Start
    let today = doc.map(|d| d.session.clock().today());
    let new_form = |kind: &str| {
        Intent::Open(Dialog::NewProject(ProjectForm {
            kind: kind.to_owned(),
            ..ProjectForm::default()
        }))
    };
    a.push(
        act(
            Group::New,
            Icon::Case,
            r.new_case,
            vec![new_form(r.case_kind)],
        )
        .long(r.new_case_long)
        .large()
        .runs(&["project.create"])
        .when(open),
    );
    a.push(
        act(Group::New, Icon::Project, r.new_project, vec![new_form("")])
            .long(r.new_project_long)
            .large()
            .runs(&["project.create"])
            .when(open),
    );
    let project = match state.view {
        View::Project(id) => doc.and_then(|d| d.session.workspace().project(id)),
        _ => None,
    };
    let task = vec![Intent::Tab(Tab::List), Intent::FocusNewTask];
    a.push(
        act(Group::New, Icon::Task, t.new_task, task)
            .runs(&["task.create"])
            .when(project.is_some()),
    );
    let from = Intent::Open(Dialog::FromTemplate(TemplateForm {
        start: today.map(crate::dates::format).unwrap_or_default(),
        ..TemplateForm::default()
    }));
    a.push(
        act(Group::New, Icon::Plus, r.from_template, vec![from])
            .long(t.from_template_title)
            .runs(&["project.instantiate"])
            .when(open),
    );
    let session = doc.map(|d| &d.session);
    a.push(
        act(
            Group::Undo,
            Icon::Undo,
            t.undo,
            vec![Intent::run("undo", json!({}))],
        )
        .keys(keys::UNDO)
        .runs(&["undo"])
        .when(session.is_some_and(plnwrk_engine::Session::can_undo)),
    );
    a.push(
        act(
            Group::Undo,
            Icon::Redo,
            t.redo,
            vec![Intent::run("redo", json!({}))],
        )
        .keys(keys::REDO)
        .runs(&["redo"])
        .when(session.is_some_and(plnwrk_engine::Session::can_redo)),
    );
    let palette = Intent::Open(Dialog::Palette {
        query: String::new(),
        selected: 0,
    });
    a.push(
        act(Group::Find, Icon::Search, r.find_action, vec![palette])
            .large()
            .keys(keys::PALETTE),
    );

    // Ansicht
    a.push(
        act(
            Group::Views,
            Icon::Overview,
            t.overview,
            vec![Intent::Go(View::Overview)],
        )
        .large()
        .when(open)
        .chosen(open && state.view == View::Overview),
    );
    if let Some(project) = project {
        let here = |tab| state.tab == tab;
        let tabs: [(Tab, Icon, &str, &'static [&'static str]); 4] = [
            (Tab::List, Icon::List, t.list, &[]),
            (Tab::Board, Icon::Board, t.board, &[]),
            (Tab::Milestones, Icon::Milestone, t.milestones, &[]),
            (
                Tab::Schedule,
                Icon::Schedule,
                t.schedule_tab,
                &["task.reschedule"],
            ),
        ];
        for (tab, icon, name, commands) in tabs {
            a.push(
                act(Group::Views, icon, name, vec![Intent::Tab(tab)])
                    .large()
                    .runs(commands)
                    .chosen(here(tab)),
            );
        }
        // While the project form is open, its head shows the form instead of the buttons
        // "Bearbeiten" and "Archivieren"; the actions follow that.
        let editing = state
            .forms
            .project
            .as_ref()
            .is_some_and(|(p, _)| p.id == project.id);
        a.push(
            act(
                Group::Areas,
                Icon::Settings,
                t.edit_project,
                vec![Intent::EditProject(project.id)],
            )
            .runs(&["project.update"])
            .when(!editing),
        );
        let archive = vec![Intent::run(
            "project.archive",
            json!({"project": project.id.to_string()}),
        )];
        a.push(
            act(Group::Areas, Icon::Export, t.archive_project, archive)
                .runs(&["project.archive"])
                .when(!editing && project.status != ProjectStatus::Archived),
        );
    }
    a.push(
        act(
            Group::Areas,
            Icon::Case,
            t.templates,
            vec![Intent::Go(View::Templates)],
        )
        .when(open)
        .chosen(open && state.view == View::Templates),
    );
    a.push(
        act(
            Group::Areas,
            Icon::Person,
            t.people,
            vec![Intent::Go(View::People)],
        )
        .runs(&["person.create", "person.update"])
        .when(open)
        .chosen(open && state.view == View::People),
    );
    for (theme, name) in crate::views::themes() {
        a.push(
            act(
                Group::Appearance,
                Icon::Settings,
                name,
                vec![Intent::Theme(theme)],
            )
            .long(format!("{}: {name}", t.appearance))
            .chosen(state.theme == theme),
        );
    }
    for language in text::Language::ALL {
        a.push(
            act(
                Group::Language,
                Icon::Help,
                language.name(),
                vec![Intent::Language(language)],
            )
            .long(format!("{}: {}", t.language_menu, language.name()))
            .chosen(state.language == language),
        );
    }

    // Aufgabe (while one is chosen)
    if let Some(task) =
        doc.and_then(|d| state.selected.and_then(|id| d.session.workspace().task(id)))
    {
        for status in TaskStatus::ALL {
            let run = vec![Intent::run(
                "task.set_status",
                json!({"task": task.id.to_string(), "status": status.name()}),
            )];
            let icon = if status == TaskStatus::Done {
                Icon::Done
            } else {
                Icon::Task
            };
            let name = text::task_status(status);
            let action = act(Group::TaskStatus, icon, name, run)
                .long(format!("{}: {name}", t.status))
                .runs(&["task.set_status"])
                .chosen(task.status == status);
            a.push(if status == TaskStatus::Done {
                action.large()
            } else {
                action
            });
        }
        a.push(
            act(
                Group::TaskEdit,
                Icon::Delete,
                t.delete_task,
                vec![Intent::Open(Dialog::DeleteTask(task.id))],
            )
            .large()
            .runs(&["task.delete"]),
        );
    }

    // Zeitplan (while it is shown)
    if let Some(project) = project
        && state.tab == Tab::Schedule
    {
        a.push(
            act(
                Group::Zoom,
                Icon::ZoomIn,
                t.zoom_in,
                vec![Intent::Zoom(project.id, 1)],
            )
            .long(format!("{}: {}", r.zoom, t.zoom_in)),
        );
        a.push(
            act(
                Group::Zoom,
                Icon::ZoomOut,
                t.zoom_out,
                vec![Intent::Zoom(project.id, -1)],
            )
            .long(format!("{}: {}", r.zoom, t.zoom_out)),
        );
        a.push(
            act(
                Group::Share,
                Icon::Export,
                t.export_schedule,
                vec![Intent::ExportSchedule(project.id)],
            )
            .large(),
        );
    }
    a.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_group_lies_in_a_tab_and_has_a_name() {
        for group in Group::ALL {
            assert!(!group.name().is_empty(), "{group:?}");
            assert!(RibbonTab::ALL.contains(&group.tab()));
        }
        for tab in RibbonTab::ALL {
            assert!(
                Group::ALL.iter().any(|g| g.tab() == tab),
                "{tab:?} has no group"
            );
        }
    }

    #[test]
    fn without_a_file_only_file_and_view_actions_can_be_used() {
        let state = State::default();
        for action in all(None, &state) {
            if action.enabled {
                assert!(
                    matches!(
                        action.group.tab(),
                        RibbonTab::File | RibbonTab::View | RibbonTab::Start
                    ),
                    "{}",
                    action.label
                );
            }
            assert!(
                !matches!(action.group.tab(), RibbonTab::Task | RibbonTab::Schedule),
                "{}",
                action.label
            );
        }
    }
}
