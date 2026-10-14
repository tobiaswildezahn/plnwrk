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
    harness.run();
    // Replace what is there, like selecting the field's text before typing.
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.run();
    let node = harness
        .query_by_label(label)
        .unwrap_or_else(|| harness.get_by(|n| n.placeholder() == Some(label)));
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
    type_into(&mut harness, "TT.MM.JJJJ", "30.13.2026", false);
    click(&mut harness, text::NEW_MILESTONE);
    let milestones = |app: &PlnwrkApp| tasks(app).into_iter().filter(|t| t.milestone).count();
    assert_eq!(milestones(harness.state()), 0);
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
    let milestone = tasks(harness.state())
        .into_iter()
        .find(|t| t.milestone)
        .unwrap();
    assert_eq!(
        milestone.due.map(|d| d.to_string()),
        Some("2026-11-30".to_owned())
    );
    assert_eq!(milestone.parent, None);
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

#[test]
fn a_click_on_a_board_card_opens_its_details() {
    let mut harness = with_project("Umzug");
    type_into(&mut harness, text::NEW_TASK_HINT, "Kisten", true);
    click(&mut harness, text::BOARD);
    let id = task_named(harness.state(), "Kisten").unwrap().id;
    // Click on the card body below the title, like a person would.
    let card = harness.get_by_label("Kisten").rect();
    let below = egui::pos2(card.center().x, card.bottom() + 8.0);
    harness.hover_at(below);
    harness.run();
    harness.drag_at(below);
    harness.run();
    harness.drop_at(below);
    harness.run();
    assert_eq!(harness.state().state.selected, Some(id));
    assert!(harness.query_by_label(text::DELETE_TASK).is_some());
}

/// Saves the sample to a file and selects a task in its list.
fn saved_sample_with_task(name: &str, task: &str) -> (Harness<'static, PlnwrkApp>, PathBuf) {
    let dir = scratch(name);
    let path = dir.join("abteilung.plnwrk");
    let host = Scripted {
        save: vec![path.clone(), path.clone()],
        open: vec![path.clone()],
    };
    let mut harness = harness(PlnwrkApp::with_host(Box::new(host)));
    click(&mut harness, text::OPEN_SAMPLE);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run();
    assert!(!harness.state().has_unsaved_changes());
    // The project is in the sidebar and on its card; either opens it.
    harness
        .get_all_by_label("Digitalfunk Umrüstung Wachen")
        .next()
        .unwrap()
        .click();
    harness.run();
    click(&mut harness, task);
    (harness, path)
}

fn reopen(path: &std::path::Path) -> crate::OpenDoc {
    crate::OpenDoc::open(path, None).unwrap()
}

#[test]
fn typed_text_is_saved_by_the_shortcut_even_while_the_field_has_focus() {
    let (mut harness, path) = saved_sample_with_task("typed-save", "Einbau Wache 11");
    type_into(
        &mut harness,
        text::DESCRIPTION,
        "Schlüssel bei Wache holen",
        false,
    );
    assert!(harness.state().has_unsaved_changes(), "typed text counts");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run();
    assert!(!harness.state().has_unsaved_changes());
    let task = task_named(harness.state(), "Einbau Wache 11").unwrap();
    assert_eq!(task.description, "Schlüssel bei Wache holen");
    drop(harness);
    let stored = reopen(&path);
    assert!(
        stored
            .session
            .workspace()
            .tasks()
            .any(|t| t.description == "Schlüssel bei Wache holen")
    );
}

#[test]
fn closing_with_typed_text_asks_first() {
    let (mut harness, _) = saved_sample_with_task("typed-close", "Einbau Wache 11");
    type_into(&mut harness, text::DUE, "24.12.2026", false);
    assert!(!harness.state_mut().close_requested(), "asks");
    harness.run();
    assert!(harness.query_by_label(text::UNSAVED_TITLE).is_some());
    let task = task_named(harness.state(), "Einbau Wache 11").unwrap();
    assert_eq!(
        task.due.map(|d| d.to_string()),
        Some("2026-12-24".to_owned())
    );
}

#[test]
fn a_wrong_typed_date_never_blocks_saving_or_closing() {
    let (mut harness, _) = saved_sample_with_task("typed-wrong", "Einbau Wache 11");
    let before = task_named(harness.state(), "Einbau Wache 11").unwrap().due;
    type_into(&mut harness, text::DESCRIPTION, "Wichtig", false);
    let draft = harness.state_mut().state.forms.task.as_mut().unwrap();
    draft.due = "31.02.2026".to_owned();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run();
    let app = harness.state();
    assert!(!app.has_unsaved_changes(), "saved anyway");
    let message = app.state.message.clone().unwrap();
    assert!(
        message.error && message.text.contains("31.02.2026"),
        "{message:?}"
    );
    let task = task_named(app, "Einbau Wache 11").unwrap();
    assert_eq!(task.due, before, "wrong date not taken");
    assert_eq!(task.description, "Wichtig", "the rest is");
    assert!(harness.state_mut().close_requested(), "closing works");
}

#[test]
fn closing_stores_typed_text_before_it_asks() {
    let (mut harness, _) = saved_sample_with_task("close-flush", "Einbau Wache 11");
    type_into(&mut harness, text::DUE, "24.12.2026", false);
    assert!(!harness.state_mut().close_requested());
    // Stored by the close request itself, before any further frame.
    let task = task_named(harness.state(), "Einbau Wache 11").unwrap();
    assert_eq!(
        task.due.map(|d| d.to_string()),
        Some("2026-12-24".to_owned())
    );
}

#[test]
fn choosing_another_task_stores_what_was_typed() {
    let (mut harness, _) = saved_sample_with_task("select-flush", "Einbau Wache 11");
    type_into(&mut harness, text::DESCRIPTION, "Notiz für Wache 11", false);
    let other = task_named(harness.state(), "Schulung Einsatzkräfte")
        .unwrap()
        .id;
    harness
        .state_mut()
        .apply(crate::Intent::Select(Some(other)));
    assert_eq!(
        task_named(harness.state(), "Einbau Wache 11")
            .unwrap()
            .description,
        "Notiz für Wache 11"
    );
    assert_eq!(
        task_named(harness.state(), "Schulung Einsatzkräfte")
            .unwrap()
            .description,
        ""
    );
}

#[test]
fn a_recovery_copy_is_written_only_when_something_would_be_lost() {
    let dir = scratch("recovery");
    // Untitled and changed: a copy that opens again.
    let mut app = harness_with_sample();
    let copy = app.state_mut().emergency_save_to(&dir).unwrap();
    let reopened = crate::OpenDoc::open(&copy, None).unwrap();
    assert!(reopened.session.workspace().projects().count() >= 3);
    assert_eq!(crate::doc::recovery_copies(&dir), vec![copy.clone()]);
    drop(reopened);
    // The person chose to discard: nothing is written.
    app.key_press_modifiers(Modifiers::COMMAND, Key::Q);
    app.run();
    click(&mut app, text::DISCARD);
    let fresh = scratch("recovery-discarded");
    assert!(app.state_mut().emergency_save_to(&fresh).is_none());
    assert!(crate::doc::recovery_copies(&fresh).is_empty());
    // Nothing changed: nothing is written.
    let mut clean = harness(PlnwrkApp::default());
    click(&mut clean, text::NEW_FILE);
    assert!(clean.state_mut().emergency_save_to(&dir).is_none());
    assert_eq!(crate::doc::recovery_copies(&dir).len(), 1);
    // The start page offers it.
    let mut start = PlnwrkApp::default();
    start.state.recovered = crate::doc::recovery_copies(&dir);
    let start = harness(start);
    assert!(start.query_by_label(text::RECOVERED).is_some());
}

#[test]
fn every_dialog_focuses_its_first_field_each_time() {
    let mut harness = harness(PlnwrkApp::default());
    click(&mut harness, text::NEW_FILE);
    for _ in 0..2 {
        click(&mut harness, text::NEW_PROJECT);
        harness.run();
        assert!(harness.get_by_label(text::TITLE).is_focused());
        click(&mut harness, text::CANCEL);
    }
}

#[test]
fn typing_survives_a_change_of_the_task_from_elsewhere() {
    let (mut harness, _) = saved_sample_with_task("rebase", "Einbau Wache 11");
    type_into(&mut harness, text::TAGS, "eilig, intern", false);
    let id = task_named(harness.state(), "Einbau Wache 11").unwrap().id;
    // The progress changes meanwhile (as by the slider or a drag on the board).
    harness
        .state_mut()
        .doc
        .as_mut()
        .unwrap()
        .session
        .run(
            "task.set_progress",
            &serde_json::json!({"task": id.to_string(), "progress": 50}),
            plnwrk_engine::Origin::User,
        )
        .unwrap();
    harness.run();
    let draft = harness.state().state.forms.task.clone().unwrap();
    assert_eq!(draft.tags, "eilig, intern", "typed text kept");
    assert_eq!(draft.progress, 50, "untouched field follows");
    harness.key_press(Key::Enter);
    harness.run();
    let task = task_named(harness.state(), "Einbau Wache 11").unwrap();
    assert_eq!(task.tags, vec!["eilig", "intern"]);
    assert_eq!(task.progress, 50);
}

#[test]
fn undo_with_the_detail_panel_open_shows_the_old_value() {
    let mut harness = with_project("Umzug");
    type_into(&mut harness, text::NEW_TASK_HINT, "Kisten", true);
    click(&mut harness, "Kisten");
    type_into(&mut harness, text::DUE, "01.12.2026", true);
    assert_eq!(
        harness.get_by_label(text::DUE).value().as_deref(),
        Some("01.12.2026")
    );
    click(&mut harness, &format!("⟲ {}", text::UNDO));
    let task = task_named(harness.state(), "Kisten").unwrap();
    assert_eq!(task.due, None);
    assert_eq!(harness.get_by_label(text::DUE).value().as_deref(), Some(""));
}

#[test]
fn every_action_that_drops_the_file_asks_first() {
    use crate::state::FileAction;
    for action in [
        FileAction::New,
        FileAction::Sample,
        FileAction::Open,
        FileAction::OpenPath(PathBuf::from("/nirgends.plnwrk")),
        FileAction::Close,
        FileAction::Quit,
    ] {
        let other = scratch("other").join("andere.plnwrk");
        let host = Scripted {
            open: vec![other],
            save: Vec::new(),
        };
        let mut harness = harness(PlnwrkApp::with_host(Box::new(host)));
        click(&mut harness, text::OPEN_SAMPLE);
        harness.state_mut().file(action.clone());
        harness.run();
        assert!(
            harness.query_by_label(text::UNSAVED_TITLE).is_some(),
            "{action:?} asks"
        );
        click(&mut harness, text::CANCEL);
        assert!(
            projects(harness.state()).len() >= 3,
            "{action:?} kept the data"
        );
        assert!(!harness.state().wants_to_quit());
    }
}

#[test]
fn the_quit_shortcut_asks_about_unsaved_changes() {
    let mut harness = harness(PlnwrkApp::default());
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Q);
    harness.run();
    assert!(harness.state().wants_to_quit(), "nothing to lose");

    let mut harness = harness_with_sample();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Q);
    harness.run();
    assert!(!harness.state().wants_to_quit());
    assert!(harness.query_by_label(text::UNSAVED_TITLE).is_some());
    click(&mut harness, text::DISCARD);
    assert!(harness.state().wants_to_quit());
}

fn harness_with_sample() -> Harness<'static, PlnwrkApp> {
    let mut harness = harness(PlnwrkApp::default());
    click(&mut harness, text::OPEN_SAMPLE);
    harness
}

#[test]
fn shortcuts_wait_while_a_dialog_is_open() {
    let mut harness = with_project("Umzug");
    type_into(&mut harness, text::NEW_TASK_HINT, "Kisten", true);
    click(&mut harness, "Kisten");
    click(&mut harness, text::DELETE_TASK);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    assert!(
        task_named(harness.state(), "Kisten").is_some(),
        "no undo behind the dialog"
    );
    click(&mut harness, text::DELETE);
    assert!(task_named(harness.state(), "Kisten").is_none());
    assert!(harness.state().state.message.is_none());
}

#[test]
fn a_half_written_comment_never_lands_on_another_task() {
    let mut harness = with_project("Umzug");
    type_into(&mut harness, text::NEW_TASK_HINT, "Kisten", true);
    type_into(&mut harness, text::NEW_TASK_HINT, "Transporter", true);
    click(&mut harness, "Kisten");
    type_into(
        &mut harness,
        text::NEW_COMMENT_HINT,
        "nur für Kisten",
        false,
    );
    click(&mut harness, "Transporter");
    assert!(harness.state().state.forms.comment.is_empty());
    let ws = harness.state().doc().unwrap().session.workspace();
    assert_eq!(ws.comments().count(), 0);
}

#[test]
fn cards_are_reordered_within_a_column() {
    let mut harness = with_project("Umzug");
    for title in ["Erste", "Zweite", "Dritte"] {
        type_into(&mut harness, text::NEW_TASK_HINT, title, true);
    }
    click(&mut harness, text::BOARD);
    let third = harness.get_by_label("Dritte").rect().center();
    let first = harness.get_by_label("Erste").rect();
    let above_first = egui::pos2(first.center().x, first.top() + 2.0);
    harness.drag_at(third);
    harness.run();
    harness.hover_at(third + egui::vec2(5.0, -5.0));
    harness.run();
    harness.hover_at(above_first);
    harness.run();
    harness.drop_at(above_first);
    harness.run();
    let mut order: Vec<Task> = tasks(harness.state());
    order.sort_by(|a, b| a.rank.cmp(&b.rank));
    let titles: Vec<&str> = order.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(titles, vec!["Dritte", "Erste", "Zweite"]);
    assert!(order.iter().all(|t| t.status == TaskStatus::Todo));
}

#[test]
fn encrypted_files_ask_for_the_passphrase_and_again_when_it_is_wrong() {
    let dir = scratch("passphrase");
    let path = dir.join("geheim.plnwrk");
    let doc = plnwrk_store::Document::from_session(&crate::OpenDoc::sample().unwrap().session);
    let secret = plnwrk_store::SecretString::from("richtig-lange-passphrase".to_owned());
    let bytes = plnwrk_store::save_with(
        &doc,
        &plnwrk_store::Protection::Passphrase(secret),
        plnwrk_store::SaveOptions {
            work_factor: Some(10),
        },
    )
    .unwrap();
    std::fs::write(&path, bytes).unwrap();
    let host = Scripted {
        open: vec![path],
        save: Vec::new(),
    };
    let mut harness = harness(PlnwrkApp::with_host(Box::new(host)));
    harness.key_press_modifiers(Modifiers::COMMAND, Key::O);
    harness.run();
    assert!(harness.query_by_label(text::PASSPHRASE_QUESTION).is_some());

    let field = harness.get_by_role(egui::accesskit::Role::PasswordInput);
    field.focus();
    field.type_text("falsch-falsch-falsch");
    harness.run();
    click(&mut harness, "Öffnen");
    assert!(harness.state().doc().is_none());
    assert!(
        harness
            .state()
            .state
            .message
            .as_ref()
            .is_some_and(|m| m.error)
    );
    assert!(
        harness.query_by_label(text::PASSPHRASE_QUESTION).is_some(),
        "asked again"
    );

    let field = harness.get_by_role(egui::accesskit::Role::PasswordInput);
    field.focus();
    field.type_text("richtig-lange-passphrase");
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    let app = harness.state();
    assert!(app.doc().is_some_and(crate::OpenDoc::encrypted));
    assert!(app.state.dialog.is_none());
}

#[test]
fn save_as_onto_the_open_file_and_opening_it_again_are_harmless() {
    let (mut harness, path) = saved_sample_with_task("same-file", "Einbau Wache 11");
    // The second scripted save answer is the same path.
    harness.get_by_label(text::FILE_MENU).click();
    harness.run();
    click(&mut harness, text::SAVE_AS);
    let app = harness.state();
    assert!(
        app.state.message.as_ref().is_some_and(|m| !m.error),
        "{:?}",
        app.state.message
    );
    // Opening the very file that is open says so instead of a lock error.
    harness.state_mut().open_path(path);
    harness.run();
    let message = harness.state().state.message.clone().unwrap();
    assert_eq!(message.text, text::ALREADY_OPEN);
}

#[test]
fn the_project_form_never_writes_back_outdated_values() {
    let mut harness = with_project("Umzug");
    let id = projects(harness.state()).first().unwrap().id;
    // "Bearbeiten" is also the menu; the button of the project head comes last.
    harness.get_all_by_label(text::EDIT).last().unwrap().click();
    harness.run();
    assert!(harness.state().state.forms.project.is_some());
    // Changed meanwhile (e.g. undo, or another field): the open form must follow.
    harness
        .state_mut()
        .doc
        .as_mut()
        .unwrap()
        .session
        .run(
            "project.update",
            &serde_json::json!({"project": id.to_string(), "reference": "AZ 7"}),
            plnwrk_engine::Origin::User,
        )
        .unwrap();
    harness.run();
    click(&mut harness, text::APPLY);
    let project = projects(harness.state()).into_iter().next().unwrap();
    assert_eq!(project.reference.as_deref(), Some("AZ 7"));
}

#[test]
fn opening_the_open_file_again_does_not_ask_or_drop_anything() {
    let (mut harness, path) = saved_sample_with_task("reopen-dirty", "Einbau Wache 11");
    type_into(&mut harness, text::NEW_TASK_HINT, "Neu", true);
    assert!(harness.state().has_unsaved_changes());
    harness.state_mut().open_path(path);
    harness.run();
    assert!(harness.state().state.dialog.is_none(), "no question");
    assert_eq!(
        harness.state().state.message.clone().unwrap().text,
        text::ALREADY_OPEN
    );
    assert!(task_named(harness.state(), "Neu").is_some());
}

#[test]
fn a_refused_typed_title_is_put_back_and_does_not_block_closing() {
    let (mut harness, _) = saved_sample_with_task("empty-title", "Einbau Wache 11");
    type_into(&mut harness, text::DESCRIPTION, "x", false);
    harness.state_mut().state.forms.task.as_mut().unwrap().title = "   ".to_owned();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run();
    assert!(
        harness
            .state()
            .state
            .message
            .as_ref()
            .is_some_and(|m| m.error)
    );
    let draft = harness.state().state.forms.task.clone().unwrap();
    assert_eq!(draft.title, "Einbau Wache 11", "put back");
    assert!(task_named(harness.state(), "Einbau Wache 11").is_some());
    assert!(
        harness.state_mut().close_requested(),
        "nothing left that blocks"
    );
}

#[test]
fn a_deadline_is_picked_in_the_calendar_and_gets_a_time() {
    let mut harness = harness(PlnwrkApp::default());
    click(&mut harness, text::NEW_FILE);
    click(&mut harness, text::NEW_PROJECT);
    type_into(&mut harness, text::TITLE, "Übung", false);
    // Start and deadline each have a calendar button; the deadline's is the second.
    harness.get_all_by_label("📅").nth(1).unwrap().click();
    harness.run();
    click(&mut harness, "15");
    let today = jiff::Zoned::now().date();
    let expected = format!("15.{:02}.{}", today.month(), today.year());
    let crate::state::Dialog::NewProject(form) = harness.state().state.dialog.clone().unwrap()
    else {
        panic!("dialog")
    };
    assert_eq!(form.due, expected);
    // No reference: the time field opens on request.
    assert!(
        harness
            .query_by(|n| n.placeholder() == Some(text::TIME_HINT))
            .is_none()
    );
    harness.get_by_label("🕑").click();
    harness.run();
    type_into(&mut harness, text::TIME_HINT, "14:30", false);
    click(&mut harness, text::CREATE);
    let project = projects(harness.state()).into_iter().next().unwrap();
    assert_eq!(project.due.map(|d| d.day()), Some(15));
    assert_eq!(
        project.due_time.map(|t| t.to_string()).as_deref(),
        Some("14:30:00")
    );
}

#[test]
fn cases_show_the_time_field_and_store_it_from_the_detail_panel() {
    let mut harness = with_project("SKA 23/9");
    let id = projects(harness.state()).first().unwrap().id;
    harness
        .state_mut()
        .doc
        .as_mut()
        .unwrap()
        .session
        .run(
            "project.update",
            &serde_json::json!({"project": id.to_string(), "reference": "23/9"}),
            plnwrk_engine::Origin::User,
        )
        .unwrap();
    type_into(&mut harness, text::NEW_TASK_HINT, "Antwort", true);
    click(&mut harness, "Antwort");
    // A case: the time field is there right away.
    type_into(&mut harness, text::DUE, "20.11.2026", false);
    type_into(&mut harness, text::TIME_HINT, "12:00", true);
    let task = task_named(harness.state(), "Antwort").unwrap();
    assert_eq!(
        task.due.map(|d| d.to_string()),
        Some("2026-11-20".to_owned())
    );
    assert_eq!(
        task.due_time.map(|t| t.to_string()).as_deref(),
        Some("12:00:00")
    );
}

#[test]
fn a_task_becomes_a_milestone_but_not_a_subtask() {
    let mut harness = with_project("Umzug");
    type_into(&mut harness, text::NEW_TASK_HINT, "Übergabe", true);
    type_into(&mut harness, text::NEW_TASK_HINT, "Packen", true);
    harness.get_all_by_label("+").last().unwrap().click();
    harness.run();
    type_into(&mut harness, text::NEW_SUBTASK_HINT, "Kartons", true);
    click(&mut harness, "Übergabe");
    harness
        .get_by_role_and_label(egui::accesskit::Role::CheckBox, text::MILESTONE)
        .click();
    harness.run();
    assert!(task_named(harness.state(), "Übergabe").unwrap().milestone);
    // The list marks it.
    assert!(harness.query_by_label("♦ Übergabe").is_some());
    // A subtask cannot be one: the box is off.
    click(&mut harness, "Kartons");
    harness
        .get_by_role_and_label(egui::accesskit::Role::CheckBox, text::MILESTONE)
        .click();
    harness.run();
    assert!(!task_named(harness.state(), "Kartons").unwrap().milestone);
    // Nor a task with subtasks.
    click(&mut harness, "Packen");
    harness
        .get_by_role_and_label(egui::accesskit::Role::CheckBox, text::MILESTONE)
        .click();
    harness.run();
    assert!(!task_named(harness.state(), "Packen").unwrap().milestone);
}

#[test]
fn a_version_1_file_opens_upgraded_and_counts_as_changed() {
    let dir = scratch("v1-open");
    let path = dir.join("alt.plnwrk");
    std::fs::copy(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../store/tests/fixtures/v1-sample.plnwrk"),
        &path,
    )
    .unwrap();
    let host = Scripted {
        open: vec![path],
        save: Vec::new(),
    };
    let mut harness = harness(PlnwrkApp::with_host(Box::new(host)));
    harness.key_press_modifiers(Modifiers::COMMAND, Key::O);
    harness.run();
    let app = harness.state();
    assert_eq!(app.state.message.clone().unwrap().text, text::UPGRADED);
    assert!(app.has_unsaved_changes(), "save to keep the new format");
    let ws = app.doc().unwrap().session.workspace();
    assert_eq!(ws.milestones().count(), 0);
    assert_eq!(ws.tasks().filter(|t| t.milestone).count(), 4);
}
