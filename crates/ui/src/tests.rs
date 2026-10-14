//! Tests of the whole app without a window, through the accessibility tree (egui_kittest):
//! they click and type like a person and check the data afterwards.

use std::path::PathBuf;

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use plnwrk_core::{Project, Task, TaskStatus};

use crate::host::Scripted;
use crate::state::{Tab, View};
use crate::{PlnwrkApp, text};

fn harness(app: PlnwrkApp) -> Harness<'static, PlnwrkApp> {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1400.0, 900.0))
        .build_ui_state(|ui, app: &mut PlnwrkApp| app.show(ui), app);
    harness.run();
    harness
}

fn click(harness: &mut Harness<'_, PlnwrkApp>, label: &str) {
    harness.get_by_label(label).click();
    harness.run();
}

/// Types into the field with this label (or placeholder) and optionally presses Enter.
fn type_into(harness: &mut Harness<'_, PlnwrkApp>, label: &str, text: &str, enter: bool) {
    let node = harness
        .query_by_label(label)
        .unwrap_or_else(|| harness.get_by(|n| n.placeholder() == Some(label)));
    node.focus();
    node.type_text(text);
    harness.run();
    if enter {
        harness.key_press(Key::Enter);
        harness.run();
    }
}

fn projects(app: &PlnwrkApp) -> Vec<Project> {
    app.doc()
        .map(|d| d.session.workspace().projects().cloned().collect())
        .unwrap_or_default()
}

fn tasks(app: &PlnwrkApp) -> Vec<Task> {
    app.doc()
        .map(|d| d.session.workspace().tasks().cloned().collect())
        .unwrap_or_default()
}

fn task_named(app: &PlnwrkApp, title: &str) -> Option<Task> {
    tasks(app).into_iter().find(|t| t.title == title)
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("plnwrk-ui-app-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// An empty file with one project, shown in its list.
fn with_project(title: &str) -> Harness<'static, PlnwrkApp> {
    let mut harness = harness(PlnwrkApp::default());
    click(&mut harness, text::NEW_FILE);
    click(&mut harness, text::NEW_PROJECT);
    type_into(&mut harness, text::TITLE, title, false);
    click(&mut harness, text::CREATE);
    harness
}

#[test]
fn the_sample_opens_and_shows_its_projects() {
    let mut harness = harness(PlnwrkApp::default());
    assert!(harness.state().doc().is_none());
    assert_eq!(harness.state().title(), crate::APP_NAME);
    click(&mut harness, text::OPEN_SAMPLE);
    let app = harness.state();
    assert!(app.doc().is_some_and(crate::OpenDoc::is_dirty));
    assert!(app.title().starts_with('●'));
    assert!(
        harness
            .query_all_by_label_contains("Digitalfunk Umrüstung Wachen")
            .count()
            >= 1
    );
    assert!(harness.query_by_label(text::OVERDUE).is_some());
}

#[test]
fn a_project_is_created_in_the_dialog_and_shown() {
    let mut harness = harness(PlnwrkApp::default());
    click(&mut harness, text::NEW_FILE);
    click(&mut harness, text::NEW_PROJECT);
    type_into(&mut harness, text::TITLE, "Umzug Leitstelle", false);
    type_into(&mut harness, text::REFERENCE, "112-4/26", false);
    type_into(&mut harness, text::DUE, "31.12.2026", false);
    click(&mut harness, text::CREATE);
    let all = projects(harness.state());
    let [project] = all.as_slice() else {
        panic!("{all:?}")
    };
    assert_eq!(project.title, "Umzug Leitstelle");
    assert_eq!(project.reference.as_deref(), Some("112-4/26"));
    assert_eq!(
        project.due.map(|d| d.to_string()),
        Some("2026-12-31".to_owned())
    );
    assert_eq!(harness.state().shown_project(), Some(project.id));
    assert!(harness.state().state.dialog.is_none());
}

#[test]
fn a_wrong_date_keeps_the_dialog_open_and_says_why() {
    let mut harness = harness(PlnwrkApp::default());
    click(&mut harness, text::NEW_FILE);
    click(&mut harness, text::NEW_PROJECT);
    type_into(&mut harness, text::TITLE, "Umzug", false);
    type_into(&mut harness, text::DUE, "31.02.2026", false);
    click(&mut harness, text::CREATE);
    let app = harness.state();
    assert!(projects(app).is_empty());
    assert!(app.state.dialog.is_some(), "input stays");
    assert!(app.state.message.as_ref().is_some_and(|m| m.error));
}

#[test]
fn tasks_subtasks_and_milestones_are_added_in_the_project() {
    let mut harness = with_project("Übung Hochwasser");
    type_into(&mut harness, text::NEW_TASK_HINT, "Lage vorbereiten", true);
    type_into(&mut harness, text::NEW_TASK_HINT, "Einladung", true);
    let parent = task_named(harness.state(), "Lage vorbereiten").unwrap();
    assert!(task_named(harness.state(), "Einladung").is_some());

    harness.get_all_by_label("+").next().unwrap().click();
    harness.run();
    type_into(&mut harness, text::NEW_SUBTASK_HINT, "Karten drucken", true);
    let child = task_named(harness.state(), "Karten drucken").unwrap();
    assert_eq!(child.parent, Some(parent.id));

    // Mark the subtask done in the list.
    let checkboxes: Vec<_> = harness
        .get_all_by_role(egui::accesskit::Role::CheckBox)
        .collect();
    assert_eq!(checkboxes.len(), 3);
    checkboxes.get(1).unwrap().click();
    harness.run();
    assert_eq!(
        task_named(harness.state(), "Karten drucken")
            .unwrap()
            .status,
        TaskStatus::Done
    );

    click(&mut harness, text::MILESTONES);
    assert_eq!(harness.state().state.tab, Tab::Milestones);
    type_into(&mut harness, text::TITLE, "Übungstag", false);
    let date_hint = format!("{} TT.MM.JJJJ", text::DATE);
    type_into(&mut harness, &date_hint, "30.13.2026", false);
    click(&mut harness, text::NEW_MILESTONE);
    let doc = harness.state().doc().unwrap();
    assert_eq!(doc.session.workspace().milestones().count(), 0);
    assert!(
        harness
            .state()
            .state
            .message
            .as_ref()
            .is_some_and(|m| m.error)
    );
    assert_eq!(
        harness.state().state.forms.milestone_title,
        "Übungstag",
        "input stays"
    );

    harness.state_mut().state.forms.milestone_date = "30.11.2026".to_owned();
    click(&mut harness, text::NEW_MILESTONE);
    let doc = harness.state().doc().unwrap();
    let milestone = doc.session.workspace().milestones().next().unwrap();
    assert_eq!(milestone.title, "Übungstag");
    assert!(harness.state().state.forms.milestone_title.is_empty());
}

#[test]
fn undo_and_redo_buttons_change_the_data_but_typing_keeps_its_own_undo() {
    let mut harness = with_project("Umzug");
    type_into(&mut harness, text::NEW_TASK_HINT, "Kisten", true);
    assert!(task_named(harness.state(), "Kisten").is_some());

    // The new-task field still has the focus: Cmd+Z belongs to the text field.
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    assert!(task_named(harness.state(), "Kisten").is_some());

    click(&mut harness, &format!("⟲ {}", text::UNDO));
    assert!(task_named(harness.state(), "Kisten").is_none());
    click(&mut harness, &format!("⟳ {}", text::REDO));
    assert!(task_named(harness.state(), "Kisten").is_some());

    // Undo everything: the project disappears and the view falls back to the overview.
    while harness.state().doc().is_some_and(|d| d.session.can_undo()) {
        click(&mut harness, &format!("⟲ {}", text::UNDO));
    }
    assert!(projects(harness.state()).is_empty());
    assert_eq!(harness.state().state.view, View::Overview);
}

#[test]
fn the_shortcut_undoes_when_no_field_is_edited() {
    let mut harness = with_project("Umzug");
    // Nothing is focused right after the dialog closed.
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    assert!(projects(harness.state()).is_empty());
    harness.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
    harness.run();
    assert_eq!(projects(harness.state()).len(), 1);
}

#[test]
fn a_case_is_created_from_the_template() {
    let mut harness = harness(PlnwrkApp::default());
    click(&mut harness, text::OPEN_SAMPLE);
    click(&mut harness, text::FROM_TEMPLATE);
    harness.get_by_value("Bitte wählen").click();
    harness.run();
    click(&mut harness, "SKA beantworten");
    type_into(&mut harness, text::TITLE, "SKA 23/2000 Warn-App", false);
    type_into(&mut harness, text::REFERENCE, "23/2000", false);
    type_into(&mut harness, text::DUE, "20.11.2026", false);
    click(&mut harness, text::CREATE);
    let case = projects(harness.state())
        .into_iter()
        .find(|p| p.title == "SKA 23/2000 Warn-App")
        .unwrap();
    assert_eq!(case.reference.as_deref(), Some("23/2000"));
    assert!(!case.template);
    assert_eq!(harness.state().shown_project(), Some(case.id));
    let ws = harness.state().doc().unwrap().session.workspace();
    assert!(ws.tasks_of(case.id).count() >= 3, "tasks of the template");
}

#[test]
fn the_detail_panel_edits_comments_and_shows_history() {
    let mut harness = with_project("Umzug");
    type_into(&mut harness, text::NEW_TASK_HINT, "Kisten packen", true);
    click(&mut harness, "Kisten packen");
    let id = task_named(harness.state(), "Kisten packen").unwrap().id;
    assert_eq!(harness.state().state.selected, Some(id));

    type_into(
        &mut harness,
        text::NEW_COMMENT_HINT,
        "Bitte bis Freitag",
        false,
    );
    click(&mut harness, text::ADD_COMMENT);
    let ws = harness.state().doc().unwrap().session.workspace();
    assert_eq!(ws.comments().filter(|c| c.task == id).count(), 1);
    assert!(harness.query_by_label("Bitte bis Freitag").is_some());

    // A due date typed in the panel is stored when the field loses focus.
    type_into(&mut harness, text::DUE, "01.12.2026", true);
    let task = task_named(harness.state(), "Kisten packen").unwrap();
    assert_eq!(
        task.due.map(|d| d.to_string()),
        Some("2026-12-01".to_owned())
    );

    click(&mut harness, text::HISTORY);
    assert!(harness.query_by_label("angelegt").is_some());
    assert!(harness.query_by_label("geändert: due").is_some());

    click(&mut harness, text::DELETE_TASK);
    click(&mut harness, text::DELETE);
    assert!(task_named(harness.state(), "Kisten packen").is_none());
    assert_eq!(harness.state().state.selected, None);
}

#[test]
fn files_are_saved_reopened_and_remembered() {
    let dir = scratch("save");
    let path = dir.join("abteilung.plnwrk");
    let host = Scripted {
        save: vec![dir.join("abteilung")],
        open: vec![path.clone()],
    };
    let mut harness = harness(PlnwrkApp::with_host(Box::new(host)));
    click(&mut harness, text::OPEN_SAMPLE);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run();
    let app = harness.state();
    assert!(path.exists());
    assert!(!app.has_unsaved_changes());
    assert_eq!(app.state.recent, vec![path.canonicalize().unwrap()]);
    assert_eq!(app.title(), format!("abteilung – {}", crate::APP_NAME));

    harness.get_by_label(text::FILE_MENU).click();
    harness.run();
    click(&mut harness, text::CLOSE_FILE);
    assert!(harness.state().doc().is_none());
    // Recently opened files appear on the start page.
    assert!(harness.query_by_label("abteilung.plnwrk").is_some());
    harness.key_press_modifiers(Modifiers::COMMAND, Key::O);
    harness.run();
    let app = harness.state();
    assert!(app.doc().is_some_and(|d| !d.is_dirty()));
    assert!(projects(app).len() >= 3);
}

#[test]
fn unsaved_changes_are_never_dropped_without_asking() {
    let mut harness = harness(PlnwrkApp::default());
    click(&mut harness, text::OPEN_SAMPLE);
    harness.get_by_label(text::FILE_MENU).click();
    harness.run();
    click(&mut harness, text::NEW_FILE);
    assert!(harness.query_by_label(text::UNSAVED_TITLE).is_some());
    click(&mut harness, text::CANCEL);
    assert!(projects(harness.state()).len() >= 3, "kept");

    assert!(!harness.state_mut().close_requested(), "asks first");
    harness.run();
    click(&mut harness, text::CANCEL);
    assert!(!harness.state().wants_to_quit());

    harness.get_by_label(text::FILE_MENU).click();
    harness.run();
    click(&mut harness, text::NEW_FILE);
    click(&mut harness, text::DISCARD);
    assert!(projects(harness.state()).is_empty());
    assert!(harness.state_mut().close_requested(), "nothing to lose");
}

#[test]
fn saving_without_a_place_asks_and_a_cancelled_dialog_changes_nothing() {
    let mut harness = harness(PlnwrkApp::default());
    click(&mut harness, text::OPEN_SAMPLE);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run();
    assert!(
        harness.state().has_unsaved_changes(),
        "no dialogs: cancelled"
    );

    // Quit → save → the save dialog is cancelled → the app stays open.
    assert!(!harness.state_mut().close_requested());
    harness.run();
    click(&mut harness, text::SAVE);
    assert!(!harness.state().wants_to_quit());
}

#[test]
fn cards_move_on_the_board_by_drag_and_drop() {
    let mut harness = with_project("Umzug");
    type_into(&mut harness, text::NEW_TASK_HINT, "Kisten", true);
    type_into(&mut harness, text::NEW_TASK_HINT, "Transporter", true);
    click(&mut harness, text::BOARD);
    let card = harness.get_by_label("Kisten").rect().center();
    let target = harness.get_by_label("Erledigt (0)").rect().center() + egui::vec2(0.0, 60.0);
    harness.drag_at(card);
    harness.run();
    harness.hover_at(card + egui::vec2(10.0, 10.0));
    harness.run();
    harness.hover_at(target);
    harness.run();
    harness.drop_at(target);
    harness.run();
    let task = task_named(harness.state(), "Kisten").unwrap();
    assert_eq!(task.status, TaskStatus::Done);
    assert_eq!(
        task_named(harness.state(), "Transporter").unwrap().status,
        TaskStatus::Todo
    );
}
