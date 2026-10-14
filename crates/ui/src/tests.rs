//! Tests of the whole app without a window, through the accessibility tree (egui_kittest):
//! they click and type like a person and check the data afterwards.

use std::path::PathBuf;

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use plnwrk_core::{Project, Task, TaskId, TaskStatus};

use crate::host::Scripted;
use crate::state::{Tab, View};
use crate::{PlnwrkApp, text};

fn harness(app: PlnwrkApp) -> Harness<'static, PlnwrkApp> {
    // Short steps, like real frames: two clicks a few steps apart count as a double click.
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1400.0, 900.0))
        .with_step_dt(1.0 / 60.0)
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
    let dir = std::env::temp_dir().join(format!(
        "plnwrk-ui-app-{}-{name}",
        plnwrk_platform::instance()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// An empty file with one project, shown in its list.
fn with_project(title: &str) -> Harness<'static, PlnwrkApp> {
    let mut harness = harness(PlnwrkApp::default());
    click(&mut harness, text::t().new_file);
    click(&mut harness, text::t().new_project);
    type_into(&mut harness, text::t().title, title, false);
    click(&mut harness, text::t().create);
    harness
}

#[test]
fn the_sample_opens_and_shows_its_projects() {
    let mut harness = harness(PlnwrkApp::default());
    assert!(harness.state().doc().is_none());
    assert_eq!(harness.state().title(), crate::APP_NAME);
    click(&mut harness, text::t().open_sample);
    let app = harness.state();
    assert!(app.doc().is_some_and(crate::OpenDoc::is_dirty));
    assert!(app.title().starts_with('●'));
    assert!(
        harness
            .query_all_by_label_contains("Digitalfunk Umrüstung Wachen")
            .count()
            >= 1
    );
    assert!(harness.query_by_label(text::t().overdue).is_some());
}

#[test]
fn a_project_is_created_in_the_dialog_and_shown() {
    let mut harness = harness(PlnwrkApp::default());
    click(&mut harness, text::t().new_file);
    click(&mut harness, text::t().new_project);
    type_into(&mut harness, text::t().title, "Umzug Leitstelle", false);
    type_into(&mut harness, text::t().reference, "112-4/26", false);
    type_into(&mut harness, text::t().due, "31.12.2026", false);
    click(&mut harness, text::t().create);
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
    click(&mut harness, text::t().new_file);
    click(&mut harness, text::t().new_project);
    type_into(&mut harness, text::t().title, "Umzug", false);
    type_into(&mut harness, text::t().due, "31.02.2026", false);
    click(&mut harness, text::t().create);
    let app = harness.state();
    assert!(projects(app).is_empty());
    assert!(app.state.dialog.is_some(), "input stays");
    assert!(app.state.message.as_ref().is_some_and(|m| m.error));
}

#[test]
fn tasks_subtasks_and_milestones_are_added_in_the_project() {
    let mut harness = with_project("Übung Hochwasser");
    type_into(
        &mut harness,
        text::t().new_task_hint,
        "Lage vorbereiten",
        true,
    );
    type_into(&mut harness, text::t().new_task_hint, "Einladung", true);
    let parent = task_named(harness.state(), "Lage vorbereiten").unwrap();
    assert!(task_named(harness.state(), "Einladung").is_some());

    harness
        .get_all_by_label(text::t().add_subtask)
        .next()
        .unwrap()
        .click();
    harness.run();
    type_into(
        &mut harness,
        text::t().new_subtask_hint,
        "Karten drucken",
        true,
    );
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

    click(&mut harness, text::t().milestones);
    assert_eq!(harness.state().state.tab, Tab::Milestones);
    type_into(&mut harness, text::t().title, "Übungstag", false);
    type_into(&mut harness, "TT.MM.JJJJ", "30.13.2026", false);
    click(&mut harness, text::t().new_milestone);
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
    click(&mut harness, text::t().new_milestone);
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
    type_into(&mut harness, text::t().new_task_hint, "Kisten", true);
    assert!(task_named(harness.state(), "Kisten").is_some());

    // The new-task field still has the focus: Cmd+Z belongs to the text field.
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    assert!(task_named(harness.state(), "Kisten").is_some());

    click(&mut harness, &format!("⟲ {}", text::t().undo));
    assert!(task_named(harness.state(), "Kisten").is_none());
    click(&mut harness, &format!("⟳ {}", text::t().redo));
    assert!(task_named(harness.state(), "Kisten").is_some());

    // Undo everything: the project disappears and the view falls back to the overview.
    while harness.state().doc().is_some_and(|d| d.session.can_undo()) {
        click(&mut harness, &format!("⟲ {}", text::t().undo));
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
    click(&mut harness, text::t().open_sample);
    click(&mut harness, text::t().from_template);
    harness.get_by_value("Bitte wählen").click();
    harness.run();
    click(&mut harness, "SKA beantworten");
    type_into(&mut harness, text::t().title, "SKA 23/2000 Warn-App", false);
    type_into(&mut harness, text::t().reference, "23/2000", false);
    type_into(&mut harness, text::t().due, "20.11.2026", false);
    click(&mut harness, text::t().create);
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
    type_into(&mut harness, text::t().new_task_hint, "Kisten packen", true);
    click(&mut harness, "Kisten packen");
    let id = task_named(harness.state(), "Kisten packen").unwrap().id;
    assert_eq!(harness.state().state.selected, Some(id));

    type_into(
        &mut harness,
        text::t().new_comment_hint,
        "Bitte bis Freitag",
        false,
    );
    click(&mut harness, text::t().add_comment);
    let ws = harness.state().doc().unwrap().session.workspace();
    assert_eq!(ws.comments().filter(|c| c.task == id).count(), 1);
    assert!(harness.query_by_label("Bitte bis Freitag").is_some());

    // A due date typed in the panel is stored when the field loses focus.
    type_into(&mut harness, text::t().due, "01.12.2026", true);
    let task = task_named(harness.state(), "Kisten packen").unwrap();
    assert_eq!(
        task.due.map(|d| d.to_string()),
        Some("2026-12-01".to_owned())
    );

    click(&mut harness, text::t().history);
    assert!(harness.query_by_label("angelegt").is_some());
    assert!(harness.query_by_label("geändert: Frist").is_some());

    click(&mut harness, text::t().delete_task);
    click(&mut harness, text::t().delete);
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
    click(&mut harness, text::t().open_sample);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run();
    let app = harness.state();
    assert!(path.exists());
    assert!(!app.has_unsaved_changes());
    assert_eq!(app.state.recent, vec![path.canonicalize().unwrap()]);
    assert_eq!(app.title(), format!("abteilung – {}", crate::APP_NAME));

    harness.get_by_label(text::t().file_menu).click();
    harness.run();
    click(&mut harness, text::t().close_file);
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
    click(&mut harness, text::t().open_sample);
    harness.get_by_label(text::t().file_menu).click();
    harness.run();
    click(&mut harness, text::t().new_file);
    assert!(harness.query_by_label(text::t().unsaved_title).is_some());
    click(&mut harness, text::t().cancel);
    assert!(projects(harness.state()).len() >= 3, "kept");

    assert!(!harness.state_mut().close_requested(), "asks first");
    harness.run();
    click(&mut harness, text::t().cancel);
    assert!(!harness.state().wants_to_quit());

    harness.get_by_label(text::t().file_menu).click();
    harness.run();
    click(&mut harness, text::t().new_file);
    click(&mut harness, text::t().discard);
    assert!(projects(harness.state()).is_empty());
    assert!(harness.state_mut().close_requested(), "nothing to lose");
}

#[test]
fn saving_without_a_place_asks_and_a_cancelled_dialog_changes_nothing() {
    let mut harness = harness(PlnwrkApp::default());
    click(&mut harness, text::t().open_sample);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run();
    assert!(
        harness.state().has_unsaved_changes(),
        "no dialogs: cancelled"
    );

    // Quit → save → the save dialog is cancelled → the app stays open.
    assert!(!harness.state_mut().close_requested());
    harness.run();
    click(&mut harness, text::t().save);
    assert!(!harness.state().wants_to_quit());
}

#[test]
fn cards_move_on_the_board_by_drag_and_drop() {
    let mut harness = with_project("Umzug");
    type_into(&mut harness, text::t().new_task_hint, "Kisten", true);
    type_into(&mut harness, text::t().new_task_hint, "Transporter", true);
    click(&mut harness, text::t().board);
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
    type_into(&mut harness, text::t().new_task_hint, "Kisten", true);
    click(&mut harness, text::t().board);
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
    assert!(harness.query_by_label(text::t().delete_task).is_some());
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
    click(&mut harness, text::t().open_sample);
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
        text::t().description,
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
    type_into(&mut harness, text::t().due, "24.12.2026", false);
    assert!(!harness.state_mut().close_requested(), "asks");
    harness.run();
    assert!(harness.query_by_label(text::t().unsaved_title).is_some());
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
    type_into(&mut harness, text::t().description, "Wichtig", false);
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
    type_into(&mut harness, text::t().due, "24.12.2026", false);
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
    type_into(
        &mut harness,
        text::t().description,
        "Notiz für Wache 11",
        false,
    );
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
    // Untitled and changed: quitting without asking writes a copy that opens again.
    let mut app = harness_with_sample();
    app.state_mut()
        .set_autosave(Some(dir.clone()), false, crate::autosave::INTERVAL);
    let copy = app.state_mut().emergency_save().unwrap();
    let reopened = crate::OpenDoc::open(&copy, None).unwrap();
    assert!(reopened.session.workspace().projects().count() >= 3);
    assert_eq!(crate::doc::recovery_copies(&dir), vec![copy.clone()]);
    drop(reopened);
    // The person chose to discard: nothing is written, and the copy goes.
    app.key_press_modifiers(Modifiers::COMMAND, Key::Q);
    app.run();
    click(&mut app, text::t().discard);
    assert!(app.state_mut().emergency_save().is_none());
    assert!(crate::doc::recovery_copies(&dir).is_empty());
    // Nothing changed: nothing is written.
    let mut clean = harness(PlnwrkApp::default());
    click(&mut clean, text::t().new_file);
    clean
        .state_mut()
        .set_autosave(Some(dir.clone()), false, crate::autosave::INTERVAL);
    assert!(clean.state_mut().emergency_save().is_none());
    assert!(crate::doc::recovery_copies(&dir).is_empty());
}

#[test]
fn autosave_writes_a_copy_and_saving_removes_it() {
    let dir = scratch("autosave-flow");
    let recovery = dir.join("Notkopien");
    let host = Scripted {
        save: vec![dir.join("abteilung.plnwrk")],
        open: Vec::new(),
    };
    let mut app = PlnwrkApp::with_host(Box::new(host));
    app.set_autosave(Some(recovery.clone()), false, 0.0);
    let mut harness = harness(app);
    click(&mut harness, text::t().new_file);
    assert!(
        crate::doc::recovery_copies(&recovery).is_empty(),
        "nothing to keep yet"
    );
    click(&mut harness, text::t().new_project);
    type_into(&mut harness, text::t().title, "Lage", false);
    click(&mut harness, text::t().create);
    harness.run();
    let copies = crate::doc::recovery_copies(&recovery);
    assert_eq!(copies.len(), 1, "a copy right after the change");
    let copy = copies.first().unwrap().clone();
    let reopened = crate::OpenDoc::open(&copy, None).unwrap();
    assert!(
        reopened
            .session
            .workspace()
            .projects()
            .any(|p| p.title == "Lage")
    );
    drop(reopened);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run();
    assert!(dir.join("abteilung.plnwrk").exists());
    assert!(!copy.exists(), "saved: the copy is gone");
    harness.run();
    assert!(
        crate::doc::recovery_copies(&recovery).is_empty(),
        "and not written again"
    );
}

#[test]
fn a_recovery_copy_opens_as_unsaved_work_and_goes_once_saved() {
    let dir = scratch("autosave-recover");
    let recovery = dir.join("Notkopien");
    // A copy left behind (e.g. by a crash).
    let mut crashed = PlnwrkApp::default();
    crashed.set_autosave(Some(recovery.clone()), false, crate::autosave::INTERVAL);
    let mut crashed = harness(crashed);
    click(&mut crashed, text::t().open_sample);
    let copy = crashed.state_mut().emergency_save().unwrap();
    drop(crashed);

    let host = Scripted {
        save: vec![dir.join("gerettet.plnwrk")],
        open: Vec::new(),
    };
    let mut app = PlnwrkApp::with_host(Box::new(host));
    app.set_autosave(Some(recovery.clone()), false, crate::autosave::INTERVAL);
    app.state.recovered = crate::doc::recovery_copies(&recovery);
    let mut harness = harness(app);
    assert!(harness.query_by_label(text::t().recovered).is_some());
    let name = copy.file_name().unwrap().to_string_lossy().into_owned();
    click(&mut harness, &name);
    let app = harness.state();
    assert_eq!(
        app.state.message.clone().unwrap().text,
        text::t().recovered_opened
    );
    assert!(app.has_unsaved_changes());
    assert!(
        app.doc().unwrap().path().is_none(),
        "saving asks for a place"
    );
    assert_eq!(app.doc().unwrap().name(), text::t().untitled);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run();
    assert!(dir.join("gerettet.plnwrk").exists());
    assert!(!copy.exists(), "recovered and saved: the copy is gone");
}

#[test]
fn every_dialog_focuses_its_first_field_each_time() {
    let mut harness = harness(PlnwrkApp::default());
    click(&mut harness, text::t().new_file);
    for _ in 0..2 {
        click(&mut harness, text::t().new_project);
        harness.run();
        assert!(harness.get_by_label(text::t().title).is_focused());
        click(&mut harness, text::t().cancel);
    }
}

#[test]
fn typing_survives_a_change_of_the_task_from_elsewhere() {
    let (mut harness, _) = saved_sample_with_task("rebase", "Einbau Wache 11");
    type_into(&mut harness, text::t().tags, "eilig, intern", false);
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
    type_into(&mut harness, text::t().new_task_hint, "Kisten", true);
    click(&mut harness, "Kisten");
    type_into(&mut harness, text::t().due, "01.12.2026", true);
    assert_eq!(
        harness.get_by_label(text::t().due).value().as_deref(),
        Some("01.12.2026")
    );
    click(&mut harness, &format!("⟲ {}", text::t().undo));
    let task = task_named(harness.state(), "Kisten").unwrap();
    assert_eq!(task.due, None);
    assert_eq!(
        harness.get_by_label(text::t().due).value().as_deref(),
        Some("")
    );
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
        click(&mut harness, text::t().open_sample);
        harness.state_mut().file(action.clone());
        harness.run();
        assert!(
            harness.query_by_label(text::t().unsaved_title).is_some(),
            "{action:?} asks"
        );
        click(&mut harness, text::t().cancel);
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
    assert!(harness.query_by_label(text::t().unsaved_title).is_some());
    click(&mut harness, text::t().discard);
    assert!(harness.state().wants_to_quit());
}

fn harness_with_sample() -> Harness<'static, PlnwrkApp> {
    let mut harness = harness(PlnwrkApp::default());
    click(&mut harness, text::t().open_sample);
    harness
}

#[test]
fn shortcuts_wait_while_a_dialog_is_open() {
    let mut harness = with_project("Umzug");
    type_into(&mut harness, text::t().new_task_hint, "Kisten", true);
    click(&mut harness, "Kisten");
    click(&mut harness, text::t().delete_task);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    assert!(
        task_named(harness.state(), "Kisten").is_some(),
        "no undo behind the dialog"
    );
    click(&mut harness, text::t().delete);
    assert!(task_named(harness.state(), "Kisten").is_none());
    assert!(harness.state().state.message.is_none());
}

#[test]
fn a_half_written_comment_never_lands_on_another_task() {
    let mut harness = with_project("Umzug");
    type_into(&mut harness, text::t().new_task_hint, "Kisten", true);
    type_into(&mut harness, text::t().new_task_hint, "Transporter", true);
    click(&mut harness, "Kisten");
    type_into(
        &mut harness,
        text::t().new_comment_hint,
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
        type_into(&mut harness, text::t().new_task_hint, title, true);
    }
    click(&mut harness, text::t().board);
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
    assert!(
        harness
            .query_by_label(text::t().passphrase_question)
            .is_some()
    );

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
        harness
            .query_by_label(text::t().passphrase_question)
            .is_some(),
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
    harness.get_by_label(text::t().file_menu).click();
    harness.run();
    click(&mut harness, text::t().save_as);
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
    assert_eq!(message.text, text::t().already_open);
}

#[test]
fn the_project_form_never_writes_back_outdated_values() {
    let mut harness = with_project("Umzug");
    let id = projects(harness.state()).first().unwrap().id;
    // "Bearbeiten" is also the menu; the button of the project head comes last.
    harness
        .get_all_by_label(text::t().edit)
        .last()
        .unwrap()
        .click();
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
    click(&mut harness, text::t().apply);
    let project = projects(harness.state()).into_iter().next().unwrap();
    assert_eq!(project.reference.as_deref(), Some("AZ 7"));
}

#[test]
fn opening_the_open_file_again_does_not_ask_or_drop_anything() {
    let (mut harness, path) = saved_sample_with_task("reopen-dirty", "Einbau Wache 11");
    type_into(&mut harness, text::t().new_task_hint, "Neu", true);
    assert!(harness.state().has_unsaved_changes());
    harness.state_mut().open_path(path);
    harness.run();
    assert!(harness.state().state.dialog.is_none(), "no question");
    assert_eq!(
        harness.state().state.message.clone().unwrap().text,
        text::t().already_open
    );
    assert!(task_named(harness.state(), "Neu").is_some());
}

#[test]
fn a_refused_typed_title_is_put_back_and_does_not_block_closing() {
    let (mut harness, _) = saved_sample_with_task("empty-title", "Einbau Wache 11");
    type_into(&mut harness, text::t().description, "x", false);
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
    click(&mut harness, text::t().new_file);
    click(&mut harness, text::t().new_project);
    type_into(&mut harness, text::t().title, "Übung", false);
    // Start and deadline each have a calendar button; the deadline's is the second.
    harness
        .get_all_by_label(text::t().pick_date)
        .nth(1)
        .unwrap()
        .click();
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
            .query_by(|n| n.placeholder() == Some(text::t().time_hint))
            .is_none()
    );
    harness.get_by_label(text::t().add_time).click();
    harness.run();
    type_into(&mut harness, text::t().time_hint, "14:30", false);
    click(&mut harness, text::t().create);
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
    type_into(&mut harness, text::t().new_task_hint, "Antwort", true);
    click(&mut harness, "Antwort");
    // A case: the time field is there right away.
    type_into(&mut harness, text::t().due, "20.11.2026", false);
    type_into(&mut harness, text::t().time_hint, "12:00", true);
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
    type_into(&mut harness, text::t().new_task_hint, "Übergabe", true);
    type_into(&mut harness, text::t().new_task_hint, "Packen", true);
    harness
        .get_all_by_label(text::t().add_subtask)
        .last()
        .unwrap()
        .click();
    harness.run();
    type_into(&mut harness, text::t().new_subtask_hint, "Kartons", true);
    click(&mut harness, "Übergabe");
    harness
        .get_by_role_and_label(egui::accesskit::Role::CheckBox, text::t().milestone)
        .click();
    harness.run();
    assert!(task_named(harness.state(), "Übergabe").unwrap().milestone);
    // The list marks it.
    assert!(harness.query_by_label("♦ Übergabe").is_some());
    // A subtask cannot be one: the box is off (not only refused by the engine).
    click(&mut harness, "Kartons");
    assert!(
        harness
            .query_by(|n| n.role() == egui::accesskit::Role::CheckBox
                && n.label().as_deref() == Some(text::t().milestone)
                && n.is_disabled())
            .is_some(),
        "disabled for subtasks"
    );
    harness
        .get_by_role_and_label(egui::accesskit::Role::CheckBox, text::t().milestone)
        .click();
    harness.run();
    assert!(!task_named(harness.state(), "Kartons").unwrap().milestone);
    // Nor a task with subtasks.
    click(&mut harness, "Packen");
    harness
        .get_by_role_and_label(egui::accesskit::Role::CheckBox, text::t().milestone)
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
    assert_eq!(app.state.message.clone().unwrap().text, text::t().upgraded);
    assert!(app.has_unsaved_changes(), "save to keep the new format");
    let ws = app.doc().unwrap().session.workspace();
    assert_eq!(ws.milestones().count(), 0);
    assert_eq!(ws.tasks().filter(|t| t.milestone).count(), 4);
}

#[test]
fn clearing_a_deadline_in_the_calendar_clears_its_time_too() {
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
    type_into(&mut harness, text::t().new_task_hint, "Antwort", true);
    click(&mut harness, "Antwort");
    type_into(&mut harness, text::t().due, "20.11.2026", false);
    type_into(&mut harness, text::t().time_hint, "12:00", true);
    assert!(
        task_named(harness.state(), "Antwort")
            .unwrap()
            .due_time
            .is_some()
    );
    // Start and deadline each have a calendar button; the deadline's is the second.
    harness
        .get_all_by_label(text::t().pick_date)
        .nth(1)
        .unwrap()
        .click();
    harness.run();
    click(&mut harness, text::t().clear);
    let task = task_named(harness.state(), "Antwort").unwrap();
    assert_eq!((task.due, task.due_time), (None, None));
    assert!(
        harness.state().state.message.is_none(),
        "no complaint about a lone time"
    );
}

#[test]
fn the_calendar_opens_at_the_month_of_its_value_again() {
    let mut harness = harness(PlnwrkApp::default());
    click(&mut harness, text::t().new_file);
    click(&mut harness, text::t().new_project);
    type_into(&mut harness, text::t().due, "15.03.2027", false);
    let open_due_calendar = |harness: &mut Harness<'static, PlnwrkApp>| {
        harness
            .get_all_by_label(text::t().pick_date)
            .nth(1)
            .unwrap()
            .click();
        harness.run();
    };
    open_due_calendar(&mut harness);
    assert!(harness.query_by_label("März 2027").is_some());
    click(&mut harness, text::t().next_month);
    assert!(harness.query_by_label("April 2027").is_some());
    harness.key_press(Key::Escape);
    harness.run();
    assert!(harness.query_by_label("April 2027").is_none(), "closed");
    open_due_calendar(&mut harness);
    assert!(
        harness.query_by_label("März 2027").is_some(),
        "back at the value's month"
    );
}

#[test]
fn templates_and_finished_projects_are_never_shown_as_overdue() {
    let mut harness = harness(PlnwrkApp::default());
    click(&mut harness, text::t().open_sample);
    click(&mut harness, text::t().templates);
    // The template's deadline (08.01.2026, 12:00) lies in the past.
    assert!(harness.query_all_by_label_contains("08.01.2026").count() >= 1);
    assert!(
        harness
            .query_all_by(|n| n.value().is_some_and(|v| v.contains("überfällig")))
            .next()
            .is_none()
    );
    // A live project past its deadline is.
    let id = projects(harness.state())
        .into_iter()
        .find(|p| p.title.starts_with("Digitalfunk"))
        .unwrap()
        .id;
    let run = |harness: &mut Harness<'static, PlnwrkApp>, params: serde_json::Value| {
        harness
            .state_mut()
            .doc
            .as_mut()
            .unwrap()
            .session
            .run("project.update", &params, plnwrk_engine::Origin::User)
            .unwrap();
        harness.run();
    };
    run(
        &mut harness,
        serde_json::json!({"project": id.to_string(), "due": "2026-10-02"}),
    );
    click(&mut harness, text::t().overview);
    let card_line = |h: &Harness<'static, PlnwrkApp>, overdue: bool| {
        // A text line and its text run both carry the value.
        h.query_all_by(|n| {
            n.value()
                .is_some_and(|v| v.contains("02.10.2026") && v.contains("überfällig") == overdue)
        })
        .next()
        .is_some()
    };
    assert!(card_line(&harness, true));
    run(
        &mut harness,
        serde_json::json!({"project": id.to_string(), "status": "done"}),
    );
    assert!(!card_line(&harness, true), "no longer overdue once done");
}

/// Two clicks in quick succession at the same place, a second after anything before (so
/// earlier clicks do not make it a triple click).
fn double_click(harness: &mut Harness<'_, PlnwrkApp>, label: &str) {
    harness.run_steps(60);
    let pos = harness.get_by_label(label).rect().center();
    harness.event(egui::Event::PointerMoved(pos));
    harness.step();
    for _ in 0..2 {
        for pressed in [true, false] {
            harness.event(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            });
            harness.step();
        }
    }
    harness.run();
}

#[test]
fn a_title_is_renamed_right_in_the_list() {
    let mut harness = with_project("Umzug");
    type_into(&mut harness, text::t().new_task_hint, "Kistn packen", true);
    // Double click: an input in place of the title.
    click(&mut harness, "Kistn packen");
    double_click(&mut harness, "Kistn packen");
    assert!(harness.state().state.forms.inline.is_some());
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.run();
    harness
        .get_by(|n| n.role() == egui::accesskit::Role::TextInput && n.is_focused())
        .type_text("Kisten packen");
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    assert!(task_named(harness.state(), "Kisten packen").is_some());
    assert!(harness.state().state.forms.inline.is_none());
    // Escape drops what was typed.
    double_click(&mut harness, "Kisten packen");
    harness
        .get_by(|n| n.role() == egui::accesskit::Role::TextInput && n.is_focused())
        .type_text(" sofort");
    harness.run();
    harness.key_press(Key::Escape);
    harness.run();
    assert!(task_named(harness.state(), "Kisten packen").is_some());
    assert!(task_named(harness.state(), "Kisten packen sofort").is_none());
    assert!(harness.state().state.forms.inline.is_none());
}

fn open_file_menu(harness: &mut Harness<'_, PlnwrkApp>) {
    harness.get_by_label(text::t().file_menu).click();
    harness.run();
}

fn type_password(harness: &mut Harness<'_, PlnwrkApp>, nth: usize, text: &str) {
    let field = harness
        .get_all_by_role(egui::accesskit::Role::PasswordInput)
        .nth(nth)
        .unwrap();
    field.focus();
    field.type_text(text);
    harness.run();
}

#[test]
fn a_file_is_protected_and_unprotected_from_the_menu() {
    let (mut harness, path) = saved_sample_with_task("protect", "Einbau Wache 11");
    open_file_menu(&mut harness);
    click(&mut harness, text::t().protect);
    // Too short, then different: refused, nothing written.
    type_password(&mut harness, 0, "kurz");
    type_password(&mut harness, 1, "kurz");
    click(&mut harness, text::t().protect_button);
    assert_eq!(
        harness.state().state.message.clone().unwrap().text,
        text::t().passphrase_too_short
    );
    assert!(
        !plnwrk_store::peek(&std::fs::read(&path).unwrap())
            .unwrap()
            .encrypted
    );
    harness.state_mut().state.close_dialog();
    open_file_menu(&mut harness);
    click(&mut harness, text::t().protect);
    type_password(&mut harness, 0, "richtig-lange-passphrase");
    type_password(&mut harness, 1, "richtig-lange-passphrasE");
    click(&mut harness, text::t().protect_button);
    assert_eq!(
        harness.state().state.message.clone().unwrap().text,
        text::t().passphrases_differ
    );
    harness.state_mut().state.close_dialog();
    // Right: saved encrypted at once.
    open_file_menu(&mut harness);
    click(&mut harness, text::t().protect);
    type_password(&mut harness, 0, "richtig-lange-passphrase");
    type_password(&mut harness, 1, "richtig-lange-passphrase");
    click(&mut harness, text::t().protect_button);
    let app = harness.state();
    assert_eq!(app.state.message.clone().unwrap().text, text::t().protected);
    assert!(app.doc().unwrap().encrypted() && !app.has_unsaved_changes());
    assert!(
        plnwrk_store::peek(&std::fs::read(&path).unwrap())
            .unwrap()
            .encrypted
    );
    // And off again.
    open_file_menu(&mut harness);
    click(&mut harness, text::t().unprotect);
    click(&mut harness, text::t().unprotect_button);
    assert_eq!(
        harness.state().state.message.clone().unwrap().text,
        text::t().unprotected
    );
    assert!(
        !plnwrk_store::peek(&std::fs::read(&path).unwrap())
            .unwrap()
            .encrypted
    );
}

#[test]
fn classified_files_keep_their_protection() {
    let (mut harness, path) = saved_sample_with_task("classified", "Einbau Wache 11");
    open_file_menu(&mut harness);
    click(&mut harness, text::t().protect);
    type_password(&mut harness, 0, "richtig-lange-passphrase");
    type_password(&mut harness, 1, "richtig-lange-passphrase");
    click(&mut harness, text::t().protect_button);
    let id = projects(harness.state()).first().unwrap().id;
    harness
        .state_mut()
        .doc
        .as_mut()
        .unwrap()
        .session
        .run(
            "project.update",
            &serde_json::json!({"project": id.to_string(), "classification": "vs_nfd"}),
            plnwrk_engine::Origin::User,
        )
        .unwrap();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run();
    open_file_menu(&mut harness);
    click(&mut harness, text::t().unprotect);
    click(&mut harness, text::t().unprotect_button);
    let message = harness.state().state.message.clone().unwrap();
    assert!(message.error, "{message:?}");
    assert!(harness.state().doc().unwrap().encrypted());
    assert!(
        plnwrk_store::peek(&std::fs::read(&path).unwrap())
            .unwrap()
            .encrypted
    );
}

#[test]
fn protecting_an_unsaved_workspace_needs_a_place_or_changes_nothing() {
    let mut harness = harness_with_sample();
    open_file_menu(&mut harness);
    click(&mut harness, text::t().protect);
    type_password(&mut harness, 0, "richtig-lange-passphrase");
    type_password(&mut harness, 1, "richtig-lange-passphrase");
    click(&mut harness, text::t().protect_button);
    // No dialogs in this test: the save dialog is cancelled.
    let app = harness.state();
    assert_eq!(
        app.state.message.clone().unwrap().text,
        text::t().protection_unchanged
    );
    assert!(!app.doc().unwrap().encrypted());
}

#[test]
fn symbol_buttons_have_spoken_names() {
    let mut harness = with_project("Umzug");
    type_into(&mut harness, text::t().new_task_hint, "Kisten", true);
    click(&mut harness, "Kisten");
    for name in [
        text::t().pick_date,
        text::t().add_subtask,
        text::t().close_details,
    ] {
        assert!(
            harness
                .query_all_by_role_and_label(egui::accesskit::Role::Button, name)
                .next()
                .is_some(),
            "{name}"
        );
    }
    // No button is announced by its symbol alone.
    for symbol in ["📅", "🕑", "🗑", "🗙", "+"] {
        assert!(harness.query_by_label(symbol).is_none(), "{symbol}");
    }
}

#[test]
fn a_recovered_copy_that_is_only_looked_at_is_kept() {
    let dir = scratch("autosave-keep");
    let recovery = dir.join("Notkopien");
    let mut crashed = PlnwrkApp::default();
    crashed.set_autosave(Some(recovery.clone()), false, crate::autosave::INTERVAL);
    let mut crashed = harness(crashed);
    click(&mut crashed, text::t().open_sample);
    let copy = crashed.state_mut().emergency_save().unwrap();
    drop(crashed);

    let mut app = PlnwrkApp::default();
    app.set_autosave(Some(recovery.clone()), false, crate::autosave::INTERVAL);
    app.state.recovered = crate::doc::recovery_copies(&recovery);
    let mut harness = harness(app);
    click(&mut harness, &copy.file_name().unwrap().to_string_lossy());
    // Close and discard: the only remaining version must not be deleted.
    open_file_menu(&mut harness);
    click(&mut harness, text::t().close_file);
    click(&mut harness, text::t().discard);
    assert!(harness.state().doc().is_none());
    assert!(copy.exists(), "kept for later");
    // … and offered again right away, where it can also be deleted (after a question).
    let name = copy.file_name().unwrap().to_string_lossy().into_owned();
    assert!(harness.query_by_label(&name).is_some());
    click(&mut harness, text::t().delete_copy);
    assert!(copy.exists(), "asks first");
    click(&mut harness, text::t().delete);
    assert!(!copy.exists());
    assert!(harness.query_by_label(&name).is_none());
}

#[test]
fn a_title_typed_in_the_list_is_saved_with_the_file() {
    let (mut harness, path) = saved_sample_with_task("inline-save", "Einbau Wache 11");
    double_click(&mut harness, "Einbau Wache 11");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.run();
    harness
        .get_by(|n| n.role() == egui::accesskit::Role::TextInput && n.is_focused())
        .type_text("Einbau Wache 12");
    harness.run();
    assert!(harness.state().has_unsaved_changes(), "typed counts");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run();
    assert!(!harness.state().has_unsaved_changes());
    drop(harness);
    let stored = reopen(&path);
    assert!(
        stored
            .session
            .workspace()
            .tasks()
            .any(|t| t.title == "Einbau Wache 12")
    );
}

#[test]
fn closing_or_replacing_a_workspace_removes_its_copy() {
    let dir = scratch("autosave-replace");
    let mut app = PlnwrkApp::default();
    app.set_autosave(Some(dir.clone()), false, 0.0);
    let mut harness = harness(app);
    click(&mut harness, text::t().open_sample);
    harness.run();
    assert_eq!(crate::doc::recovery_copies(&dir).len(), 1);
    // Another file instead (changes discarded): the first copy goes.
    open_file_menu(&mut harness);
    click(&mut harness, text::t().new_file);
    click(&mut harness, text::t().discard);
    assert!(crate::doc::recovery_copies(&dir).is_empty());
    click(&mut harness, text::t().new_project);
    type_into(&mut harness, text::t().title, "Neu", false);
    click(&mut harness, text::t().create);
    harness.run();
    assert_eq!(crate::doc::recovery_copies(&dir).len(), 1);
    open_file_menu(&mut harness);
    click(&mut harness, text::t().close_file);
    click(&mut harness, text::t().discard);
    assert!(crate::doc::recovery_copies(&dir).is_empty());
}

/// Every text the screen shows: names and values in the accessibility tree.
fn shown_texts(harness: &Harness<'_, PlnwrkApp>) -> Vec<String> {
    harness
        .query_all_by(|_| true)
        .flat_map(|n| {
            let node = egui_kittest::kittest::NodeT::accesskit_node(&n);
            [node.label(), node.value()]
        })
        .flatten()
        .collect()
}

/// Whether a text looks German: German letters or quotes, or a word only German uses here.
fn looks_german(text: &str) -> bool {
    const WORDS: &[&str] = &[
        "und",
        "Bitte",
        "nicht",
        "Neu",
        "Neue",
        "Neues",
        "Aufgabe",
        "Aufgaben",
        "Frist",
        "Datei",
        "Speichern",
        "Vorlage",
        "Vorlagen",
        "Projekt",
        "Projekte",
        "Vorgang",
        "Vorgänge",
        "Uhr",
        "Beginn",
        "Personen",
        "Bearbeiten",
        "Wiederholen",
        "Abbrechen",
        "Anlegen",
        "Offen",
        "Erledigt",
        "Kommentar",
        "Kommentare",
        "Verlauf",
        "Meilenstein",
        "Meilensteine",
        "Liste",
        "Heute",
        "Leeren",
        "Titel",
        "Art",
        "Zuständig",
        "Verantwortlich",
        "Schlagworte",
        "Beschreibung",
        "Ansicht",
        "Sprache",
        "Darstellung",
        "seit",
        "Tagen",
        "Tag",
        "heute",
        "morgen",
        "angelegt",
        "Version",
        "Unbenannt",
        "ungespeichert",
        "Kürzel",
        "Funktion",
        "Hoch",
        "Niedrig",
        "Aktiv",
        "Entwurf",
        "Fortschritt",
        "Priorität",
        "Wiederherstellen",
        "offen",
        "von",
        "erledigt",
        "JJJJ",
    ];
    // "Version" is English too; it is only listed to catch "Version" glued to German text.
    text.chars().any(|c| "äöüÄÖÜß„".contains(c))
        || text
            .split(|c: char| !c.is_alphanumeric())
            .any(|w| w != "Version" && WORDS.contains(&w))
}

fn assert_english(harness: &Harness<'_, PlnwrkApp>, place: &str) {
    let german: Vec<String> = shown_texts(harness)
        .into_iter()
        .filter(|t| looks_german(t))
        .collect();
    assert!(german.is_empty(), "German on {place}: {german:?}");
}

/// Opens a submenu (its button carries an arrow after the name).
fn submenu(harness: &mut Harness<'_, PlnwrkApp>, name: &str) {
    harness
        .get_by(|n| n.label().is_some_and(|l| l.starts_with(name)))
        .click();
    harness.run();
}

/// Switches the interface to English through the menu, like a person.
fn to_english(harness: &mut Harness<'_, PlnwrkApp>) {
    click(harness, text::DE.view_menu);
    submenu(harness, text::DE.language_menu);
    click(harness, text::Language::English.name());
    assert_eq!(harness.state().state.language, text::Language::English);
}

#[test]
fn the_german_detector_finds_german() {
    assert!(looks_german("Neue Aufgabe … (Enter)"));
    assert!(looks_german("seit 3 Tagen überfällig"));
    assert!(!looks_german("New task … (Enter)"));
    assert!(!looks_german("3 days overdue"));
}

#[test]
fn the_whole_interface_speaks_english() {
    let mut harness = harness(PlnwrkApp::default());
    to_english(&mut harness);
    let t = &text::EN;
    assert!(harness.query_by_label(t.open_sample).is_some());
    assert_english(&harness, "start page");

    click(&mut harness, t.new_file);
    assert_english(&harness, "overview");
    click(&mut harness, t.new_project);
    assert_english(&harness, "new project dialog");
    type_into(&mut harness, t.title, "Move", false);
    type_into(&mut harness, t.reference, "A-1", false);
    type_into(&mut harness, t.due, "2020-01-01", false);
    click(&mut harness, t.create);
    assert_english(&harness, "project list");

    type_into(&mut harness, t.new_task_hint, "Draft", true);
    click(&mut harness, "Draft");
    assert_english(&harness, "detail panel");
    click(&mut harness, t.history);
    assert!(harness.query_by_label(t.created).is_some());
    assert_english(&harness, "history");

    click(&mut harness, t.board);
    assert_english(&harness, "board");
    click(&mut harness, t.milestones);
    assert_english(&harness, "milestones");
    click(&mut harness, t.people);
    assert_english(&harness, "people");
    click(&mut harness, t.templates);
    assert_english(&harness, "templates");
    click(&mut harness, t.overview);
    assert!(
        harness.query_all_by_label_contains("days overdue").count() >= 1,
        "the deadline reads in English"
    );
    assert_english(&harness, "overview with an overdue case");

    click(&mut harness, t.file_menu);
    assert_english(&harness, "file menu");
    click(&mut harness, t.protect);
    assert_english(&harness, "protect dialog");
    click(&mut harness, t.cancel);
    click(&mut harness, t.view_menu);
    assert_english(&harness, "view menu");
    harness.key_press(Key::Escape);
    harness.run();

    harness.key_press_modifiers(Modifiers::COMMAND, Key::Q);
    harness.run();
    assert!(harness.query_by_label(t.unsaved_title).is_some());
    assert_english(&harness, "unsaved changes dialog");
}

#[test]
fn english_shows_iso_dates_and_still_reads_german_ones() {
    let mut harness = harness(PlnwrkApp::default());
    to_english(&mut harness);
    let t = &text::EN;
    click(&mut harness, t.new_file);
    click(&mut harness, t.new_project);
    type_into(&mut harness, t.title, "Move", false);
    type_into(&mut harness, t.due, "31.12.2026", false);
    click(&mut harness, t.create);
    let project = projects(harness.state()).pop().unwrap();
    assert_eq!(
        project.due.map(|d| d.to_string()).as_deref(),
        Some("2026-12-31")
    );
    click(&mut harness, t.overview);
    assert!(harness.query_all_by_label_contains("2026-12-31").count() >= 1);
    assert_eq!(harness.query_all_by_label_contains("31.12.2026").count(), 0);
}

#[test]
fn switching_the_language_keeps_typed_text() {
    let mut harness = with_project("Lage");
    type_into(&mut harness, text::t().new_task_hint, "Bericht", true);
    click(&mut harness, "Bericht");
    // A date typed in German, not taken yet, when the language changes.
    type_into(&mut harness, text::t().due, "24.12.2026", false);
    to_english(&mut harness);
    assert!(harness.state().has_unsaved_changes());
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Q);
    harness.run();
    click(&mut harness, text::EN.cancel);
    let task = task_named(harness.state(), "Bericht").unwrap();
    assert_eq!(
        task.due.map(|d| d.to_string()).as_deref(),
        Some("2026-12-24")
    );
}

/// Settings kept in memory, like eframe keeps them in a file.
#[derive(Default)]
struct MemoryStorage(std::collections::HashMap<String, String>);

impl eframe::Storage for MemoryStorage {
    fn get_string(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }

    fn set_string(&mut self, key: &str, value: String) {
        self.0.insert(key.to_owned(), value);
    }

    fn remove_string(&mut self, key: &str) {
        self.0.remove(key);
    }

    fn flush(&mut self) {}
}

#[test]
fn language_and_theme_are_remembered() {
    let mut first = harness(PlnwrkApp::default());
    to_english(&mut first);
    for (name, theme, dark) in [
        (text::EN.theme_light, egui::ThemePreference::Light, false),
        (text::EN.theme_dark, egui::ThemePreference::Dark, true),
        (text::EN.theme_light, egui::ThemePreference::Light, false),
    ] {
        click(&mut first, text::EN.view_menu);
        submenu(&mut first, text::EN.appearance);
        click(&mut first, name);
        assert_eq!(first.ctx.options(|o| o.theme_preference), theme);
        assert_eq!(first.ctx.global_style().visuals.dark_mode, dark, "{name}");
    }
    let mut storage = MemoryStorage::default();
    eframe::App::save(first.state_mut(), &mut storage);
    drop(first);

    // Next start, on a thread that still speaks German.
    text::set_language(text::Language::German);
    let mut app = PlnwrkApp::default();
    app.load_settings(&storage);
    assert_eq!(
        text::t().new_file,
        text::EN.new_file,
        "texts made before the first frame"
    );
    let restarted = harness(app);
    assert!(restarted.query_by_label(text::EN.new_file).is_some());
    assert_eq!(
        restarted.ctx.options(|o| o.theme_preference),
        egui::ThemePreference::Light
    );
    assert!(!restarted.ctx.global_style().visuals.dark_mode);
}

#[test]
fn each_window_speaks_its_own_language() {
    let mut english = harness(PlnwrkApp::default());
    to_english(&mut english);
    // Another app on the same thread, still German.
    let german = harness(PlnwrkApp::default());
    assert!(german.query_by_label(text::DE.new_file).is_some());
    assert!(german.query_by_label(text::EN.new_file).is_none());
    english.run();
    assert!(english.query_by_label(text::EN.new_file).is_some());
}

#[test]
fn a_failing_autosave_is_reported_in_the_language_of_the_window() {
    let dir = scratch("autosave-english");
    // The recovery folder cannot be made: a file is in its place.
    let blocked = dir.join("Notkopien");
    std::fs::write(&blocked, b"").unwrap();
    let mut app = PlnwrkApp::default();
    app.set_autosave(Some(blocked), true, 0.0);
    let mut harness = harness(app);
    to_english(&mut harness);
    click(&mut harness, text::EN.open_sample);
    let mut message = None;
    for _ in 0..200 {
        harness.run();
        message = harness.state().state.message.clone();
        if message.as_ref().is_some_and(|m| m.error) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let message = message.unwrap().text;
    assert!(message.starts_with(text::EN.recovery_failed), "{message}");
    assert!(message.contains("File access failed"), "{message}");
}

#[test]
fn unknown_stored_settings_are_ignored() {
    let mut storage = MemoryStorage::default();
    eframe::Storage::set_string(&mut storage, super::LANGUAGE_KEY, "tlh".to_owned());
    eframe::Storage::set_string(&mut storage, super::THEME_KEY, "neon".to_owned());
    let mut app = PlnwrkApp::default();
    app.load_settings(&storage);
    assert_eq!(app.state.language, text::Language::German);
    assert_eq!(app.state.theme, egui::ThemePreference::System);
}

/// Opens the palette with Cmd+K, types and presses Enter.
fn palette(harness: &mut Harness<'_, PlnwrkApp>, query: &str) {
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run();
    let field = text::t().palette.trim_end_matches('…').to_owned();
    type_into(harness, &field, query, true);
}

fn project_named(app: &PlnwrkApp, title: &str) -> plnwrk_core::ProjectId {
    projects(app)
        .into_iter()
        .find(|p| p.title == title)
        .unwrap()
        .id
}

#[test]
fn the_palette_goes_to_a_project_by_typing() {
    let mut harness = harness_with_sample();
    palette(&mut harness, "digi");
    let app = harness.state();
    assert!(app.state.dialog.is_none(), "chosen: the palette closes");
    assert_eq!(
        app.shown_project(),
        Some(project_named(app, "Digitalfunk Umrüstung Wachen"))
    );
}

#[test]
fn the_palette_goes_to_a_task_in_another_project() {
    let mut harness = harness_with_sample();
    palette(&mut harness, "sirene");
    palette(&mut harness, "leistungsverz");
    let app = harness.state();
    let task = task_named(app, "Leistungsverzeichnis").unwrap();
    assert_eq!(app.shown_project(), Some(task.project));
    assert_eq!(app.state.selected, Some(task.id));
    assert!(harness.query_by_label(text::t().close_details).is_some());
}

#[test]
fn the_palette_runs_what_the_menus_and_buttons_do() {
    let dir = scratch("palette-save");
    let host = Scripted {
        save: vec![dir.join("lage.plnwrk")],
        open: Vec::new(),
    };
    let mut harness = harness(PlnwrkApp::with_host(Box::new(host)));
    palette(&mut harness, "beispiel");
    assert!(harness.state().doc().is_some(), "file menu");
    palette(&mut harness, "digi");
    palette(&mut harness, "board");
    assert_eq!(harness.state().state.tab, Tab::Board, "tabs of the project");
    palette(&mut harness, "speich");
    assert!(
        dir.join("lage.plnwrk").exists(),
        "Speichern comes before Speichern unter"
    );
    assert!(!harness.state().has_unsaved_changes());

    palette(&mut harness, "neue aufgabe");
    assert_eq!(harness.state().state.tab, Tab::List);
    harness.run();
    // The cursor is in the field for a new task: typing goes there.
    harness.event(egui::Event::Text("a".to_owned()));
    harness.run();
    let typed = harness.state().state.forms.new_task.clone();
    assert_eq!(typed, "a", "the new task field has the cursor");

    palette(&mut harness, "projekt bearbeiten");
    assert!(
        harness.state().state.forms.project.is_some(),
        "edit form open"
    );
    assert!(harness.query_by_label(text::t().apply).is_some());

    palette(&mut harness, "english");
    assert_eq!(harness.state().state.language, text::Language::English);
    palette(&mut harness, "dark");
    assert_eq!(harness.state().state.theme, egui::ThemePreference::Dark);
}

#[test]
fn the_palette_sets_the_status_of_the_selected_task_and_asks_before_deleting() {
    let mut harness = harness_with_sample();
    palette(&mut harness, "vergabevermerk");
    let id = task_named(harness.state(), "Vergabevermerk").unwrap().id;
    palette(&mut harness, "status erledigt");
    let task = harness
        .state()
        .doc()
        .unwrap()
        .session
        .workspace()
        .task(id)
        .cloned();
    assert_eq!(task.unwrap().status, TaskStatus::Done);
    palette(&mut harness, "aufgabe löschen");
    assert!(
        matches!(
            harness.state().state.dialog,
            Some(crate::state::Dialog::DeleteTask(_))
        ),
        "deleting still asks"
    );
    click(&mut harness, text::t().cancel);
    assert!(task_named(harness.state(), "Vergabevermerk").is_some());
}

#[test]
fn arrows_choose_and_escape_closes_without_doing_anything() {
    let mut harness = harness_with_sample();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run();
    let field = text::t().palette.trim_end_matches('…').to_owned();
    type_into(&mut harness, &field, "vorlage", false);
    // "Vorlagen" first (the name starts with it), then "Aus Vorlage anlegen"; the choice
    // stops at both ends.
    let app = harness.state();
    let count =
        crate::palette::search(crate::palette::entries(app.doc(), &app.state), "vorlage").len();
    assert!(count >= 2);
    for _ in 0..count + 2 {
        harness.key_press(Key::ArrowDown);
        harness.run();
    }
    let selected = |h: &Harness<'_, PlnwrkApp>| match &h.state().state.dialog {
        Some(crate::state::Dialog::Palette { selected, .. }) => *selected,
        _ => usize::MAX,
    };
    assert_eq!(selected(&harness), count - 1, "stops at the last");
    for _ in 0..count + 2 {
        harness.key_press(Key::ArrowUp);
        harness.run();
    }
    assert_eq!(selected(&harness), 0, "stops at the first");
    harness.key_press(Key::ArrowDown);
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    assert!(matches!(
        harness.state().state.dialog,
        Some(crate::state::Dialog::FromTemplate(_))
    ));
    click(&mut harness, text::t().cancel);

    let before = harness.state().state.view;
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run();
    type_into(&mut harness, &field, "personen", false);
    harness.key_press(Key::Escape);
    harness.run();
    assert!(harness.state().state.dialog.is_none());
    assert_eq!(harness.state().state.view, before);
}

#[test]
fn nothing_found_keeps_the_palette_open() {
    let mut harness = harness_with_sample();
    palette(&mut harness, "xyzzy");
    assert!(harness.query_by_label(text::t().palette_empty).is_some());
    assert!(matches!(
        harness.state().state.dialog,
        Some(crate::state::Dialog::Palette { .. })
    ));
}

#[test]
fn text_typed_before_the_palette_is_kept() {
    let dir = scratch("palette-typed");
    let host = Scripted {
        save: vec![dir.join("lage.plnwrk")],
        open: Vec::new(),
    };
    let mut app = PlnwrkApp::with_host(Box::new(host));
    app.file(crate::state::FileAction::New);
    let mut harness = harness(app);
    click(&mut harness, text::t().new_project);
    type_into(&mut harness, text::t().title, "Lage", false);
    click(&mut harness, text::t().create);
    type_into(&mut harness, text::t().new_task_hint, "Bericht", true);
    click(&mut harness, "Bericht");
    // Typed in the detail panel, still in the field, then Cmd+K → Speichern.
    type_into(&mut harness, text::t().description, "Lage um 10 Uhr", false);
    palette(&mut harness, "speichern");
    let bytes = std::fs::read(dir.join("lage.plnwrk")).unwrap();
    let saved = crate::OpenDoc::from_bytes(&bytes, None).unwrap();
    let task = saved
        .session
        .workspace()
        .tasks()
        .find(|t| t.title == "Bericht")
        .cloned()
        .unwrap();
    assert_eq!(task.description, "Lage um 10 Uhr");
}

#[test]
fn every_command_can_be_reached_from_the_palette_or_has_its_place() {
    let mut harness = harness_with_sample();
    palette(&mut harness, "vergabevermerk");
    // Something to undo and to redo.
    palette(&mut harness, "status erledigt");
    palette(&mut harness, "status in arbeit");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    let app = harness.state();
    let reached: std::collections::BTreeSet<&str> = crate::palette::entries(app.doc(), &app.state)
        .iter()
        .flat_map(|e| e.commands.iter().copied())
        .collect();
    let elsewhere: std::collections::BTreeSet<&str> = crate::palette::ELSEWHERE
        .iter()
        .map(|(id, _)| *id)
        .collect();
    // What an entry claims is at least what it runs.
    for entry in crate::palette::entries(app.doc(), &app.state) {
        for intent in &entry.intents {
            if let crate::Intent::Run { command, .. } = intent {
                assert!(
                    entry.commands.contains(command),
                    "{} runs {command} without naming it",
                    entry.label
                );
            }
        }
    }
    for spec in plnwrk_engine::registry() {
        assert!(
            reached.contains(spec.id) || elsewhere.contains(spec.id),
            "{} is neither in the palette nor in palette::ELSEWHERE",
            spec.id
        );
        assert!(
            !(reached.contains(spec.id) && elsewhere.contains(spec.id)),
            "{} is in the palette: take it off palette::ELSEWHERE",
            spec.id
        );
    }
    for id in reached.iter().chain(&elsewhere) {
        assert!(
            plnwrk_engine::command::find(id).is_some(),
            "{id} is not a command"
        );
    }
}

#[test]
fn the_palette_speaks_english_too() {
    let mut harness = harness_with_sample();
    to_english(&mut harness);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run();
    let shown: Vec<String> = shown_texts(&harness)
        .into_iter()
        .filter(|t| looks_german(t))
        .collect();
    // Names of projects and tasks are data and stay as typed; everything else is English.
    let data: Vec<String> = tasks(harness.state())
        .iter()
        .map(|t| t.title.clone())
        .chain(projects(harness.state()).iter().map(|p| p.title.clone()))
        .chain(
            projects(harness.state())
                .iter()
                .filter_map(|p| p.kind.clone()),
        )
        .collect();
    let german: Vec<&String> = shown
        .iter()
        .filter(|t| !data.iter().any(|d| t.contains(d.as_str())))
        .collect();
    assert!(german.is_empty(), "German in the palette: {german:?}");
}

#[test]
fn undo_after_switching_the_language_does_not_bring_a_date_back() {
    let mut harness = with_project("Lage");
    type_into(&mut harness, text::t().new_task_hint, "Bericht", true);
    click(&mut harness, "Bericht");
    type_into(&mut harness, text::t().due, "01.12.2026", true);
    click(&mut harness, text::t().description);
    let due = |h: &Harness<'_, PlnwrkApp>| task_named(h.state(), "Bericht").unwrap().due;
    assert_eq!(
        due(&harness).map(|d| d.to_string()).as_deref(),
        Some("2026-12-01")
    );
    to_english(&mut harness);
    // The untouched field now reads in English.
    assert_eq!(
        harness.get_by_label(text::EN.due).value().as_deref(),
        Some("2026-12-01")
    );
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    assert_eq!(due(&harness), None, "undone");
    let draft_pending = harness
        .state()
        .state
        .forms
        .task
        .as_ref()
        .is_some_and(crate::state::TaskDraft::is_pending);
    assert!(!draft_pending, "nothing typed: the field follows the undo");
    click(&mut harness, text::EN.overview);
    assert_eq!(due(&harness), None, "and the old date is not written back");
}

#[test]
fn a_half_edited_project_form_is_not_lost() {
    let dir = scratch("project-form");
    let host = Scripted {
        save: vec![dir.join("lage.plnwrk")],
        open: Vec::new(),
    };
    let mut app = PlnwrkApp::with_host(Box::new(host));
    app.file(crate::state::FileAction::New);
    let mut harness = harness(app);
    click(&mut harness, text::t().new_project);
    type_into(&mut harness, text::t().title, "Umzug", false);
    click(&mut harness, text::t().create);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run();
    assert!(!harness.state().has_unsaved_changes(), "saved");
    open_project_form(&mut harness);
    type_into(&mut harness, text::t().title, "Umzug NEU", false);
    // The palette neither reopens the form nor archives behind it.
    palette(&mut harness, "projekt bearbeiten");
    let id = projects(harness.state()).first().unwrap().id;
    harness.state_mut().apply(crate::Intent::EditProject(id));
    assert_eq!(
        harness
            .state()
            .state
            .forms
            .project
            .as_ref()
            .unwrap()
            .1
            .title,
        "Umzug NEU"
    );
    let offered: Vec<String> =
        crate::palette::entries(harness.state().doc(), &harness.state().state)
            .into_iter()
            .map(|e| e.label)
            .collect();
    assert!(!offered.iter().any(|l| l == text::t().archive_project));
    assert!(!offered.iter().any(|l| l == text::t().edit_project));
    harness.key_press(Key::Escape);
    harness.run();
    // Typed in the form, not applied: that is unsaved work, and saving takes it.
    assert!(harness.state().has_unsaved_changes());
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run();
    let bytes = std::fs::read(dir.join("lage.plnwrk")).unwrap();
    let saved = crate::OpenDoc::from_bytes(&bytes, None).unwrap();
    assert!(
        saved
            .session
            .workspace()
            .projects()
            .any(|p| p.title == "Umzug NEU")
    );
    assert!(!harness.state().has_unsaved_changes());
}

#[test]
fn a_project_form_with_a_wrong_date_never_blocks_closing() {
    let mut harness = with_project("Umzug");
    open_project_form(&mut harness);
    type_into(&mut harness, text::t().start, "32.13.2026", false);
    assert!(harness.state().has_unsaved_changes());
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Q);
    harness.run();
    let message = harness.state().state.message.clone().unwrap();
    assert!(
        message.error && message.text.starts_with(text::t().not_taken),
        "{message:?}"
    );
    assert!(harness.state().state.forms.project.is_none(), "put back");
}

#[test]
fn an_unchanged_project_form_is_not_unsaved_work_in_either_language() {
    let mut harness = with_project("Umzug");
    type_into(&mut harness, text::t().new_task_hint, "x", true);
    open_project_form(&mut harness);
    type_into(&mut harness, text::t().due, "01.12.2026", false);
    click(&mut harness, text::t().apply);
    open_project_form(&mut harness);
    let dirty_before = harness.state().doc().unwrap().is_dirty();
    assert_eq!(harness.state().has_unsaved_changes(), dirty_before);
    assert!(harness.state().project_pending().is_none());
    to_english(&mut harness);
    assert!(
        harness.state().project_pending().is_none(),
        "01.12.2026 is 2026-12-01"
    );
}

/// Opens the form of the shown project ("Bearbeiten" is also the menu; the button comes last).
fn open_project_form(harness: &mut Harness<'_, PlnwrkApp>) {
    harness
        .get_all_by_label(text::t().edit)
        .last()
        .unwrap()
        .click();
    harness.run();
}

#[test]
fn the_palette_shows_a_limited_number_of_results_and_skips_archived_projects() {
    let mut harness = with_project("Lage");
    let project = projects(harness.state()).first().unwrap().id.to_string();
    for n in 0..120 {
        harness.state_mut().apply(crate::Intent::run(
            "task.create",
            serde_json::json!({"project": project, "title": format!("Punkt {n}")}),
        ));
    }
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run();
    let field = text::t().palette.trim_end_matches('…').to_owned();
    type_into(&mut harness, &field, "punkt", false);
    assert!(harness.query_by_label(text::t().more_results).is_some());
    harness.key_press(Key::Escape);
    harness.run();
    palette(&mut harness, "projekt archivieren");
    let offered: Vec<String> =
        crate::palette::entries(harness.state().doc(), &harness.state().state)
            .into_iter()
            .map(|e| e.label)
            .collect();
    assert!(
        !offered.iter().any(|l| l.starts_with("Punkt")),
        "archived: its tasks are not offered"
    );
}

#[test]
fn an_untouched_project_form_on_another_view_does_not_undo_an_undo() {
    let mut harness = with_project("Umzug");
    open_project_form(&mut harness);
    type_into(&mut harness, text::t().reference, "AZ 1", false);
    click(&mut harness, text::t().apply);
    open_project_form(&mut harness);
    click(&mut harness, text::t().overview);
    assert!(
        harness.state().state.forms.project.is_some(),
        "still open, out of sight"
    );
    let reference =
        |h: &Harness<'_, PlnwrkApp>| projects(h.state()).first().unwrap().reference.clone();
    // Undone without a frame in between (e.g. closing right after): nothing typed in the form,
    // so it is not unsaved work, and no flush writes it back.
    harness
        .state_mut()
        .apply(crate::Intent::run("undo", serde_json::json!({})));
    assert_eq!(reference(&harness), None, "undone");
    assert!(harness.state().project_pending().is_none());
    harness.state_mut().close_requested();
    assert_eq!(reference(&harness), None, "the undo stays");
    // And after frames, on another view.
    harness.run();
    click(&mut harness, text::t().templates);
    harness.state_mut().close_requested();
    assert_eq!(reference(&harness), None);
}

#[test]
fn a_refused_rename_is_named_even_when_more_is_taken_after_it() {
    let mut harness = with_project("Umzug");
    type_into(&mut harness, text::t().new_task_hint, "Bericht", true);
    let task = task_named(harness.state(), "Bericht").unwrap().id;
    open_project_form(&mut harness);
    type_into(&mut harness, text::t().title, "Umzug NEU", false);
    // A title the data rules refuse (control character), typed in the list, still in its field.
    harness.state_mut().state.forms.inline = Some((task, "Be\u{7}richt".to_owned()));
    harness.state_mut().close_requested();
    let message = harness.state().state.message.clone().unwrap();
    assert!(message.error, "{message:?}");
    assert!(message.text.starts_with(text::t().not_taken), "{message:?}");
    assert!(
        projects(harness.state())
            .iter()
            .any(|p| p.title == "Umzug NEU")
    );
}

#[test]
fn project_form_dates_follow_a_language_switch() {
    let mut harness = with_project("Umzug");
    open_project_form(&mut harness);
    type_into(&mut harness, text::t().due, "01.12.2026", false);
    click(&mut harness, text::t().apply);
    open_project_form(&mut harness);
    to_english(&mut harness);
    assert_eq!(
        harness.state().state.forms.project.as_ref().unwrap().1.due,
        "2026-12-01"
    );
}

/// An app in the browser, and the log of what its browser saw.
fn in_browser() -> (Harness<'static, PlnwrkApp>, crate::host::TestBrowser) {
    let browser = crate::host::TestBrowser::default();
    let app = PlnwrkApp::with_host(Box::new(browser.clone()));
    (harness(app), browser)
}

/// The bytes of a saved file with this workspace.
fn file_bytes(doc: &crate::OpenDoc) -> Vec<u8> {
    doc.to_bytes().unwrap()
}

/// The person chooses this file in the browser's dialog.
fn choose_upload(
    harness: &mut Harness<'_, PlnwrkApp>,
    browser: &crate::host::TestBrowser,
    name: &str,
    bytes: Vec<u8>,
) {
    browser.0.borrow_mut().upload = Some(Ok(crate::Upload {
        name: name.to_owned(),
        bytes,
    }));
    harness.run();
}

#[test]
fn the_browser_opens_an_uploaded_file_and_saves_by_download() {
    let (mut harness, browser) = in_browser();
    click(&mut harness, text::t().open_file);
    assert_eq!(
        browser.0.borrow().upload_requests,
        1,
        "the browser asks for a file"
    );
    let sample = crate::OpenDoc::sample().unwrap();
    choose_upload(
        &mut harness,
        &browser,
        "abteilung.plnwrk",
        file_bytes(&sample),
    );
    let app = harness.state();
    assert_eq!(app.doc().unwrap().name(), "abteilung");
    assert!(!app.has_unsaved_changes(), "as uploaded");
    assert!(app.state.recent.is_empty(), "no paths in the browser");
    assert_eq!(
        app.doc().unwrap().session.workspace(),
        sample.session.workspace()
    );

    palette(&mut harness, "digi");
    type_into(&mut harness, text::t().new_task_hint, "Funkprobe", true);
    assert!(harness.state().has_unsaved_changes());
    assert!(browser.0.borrow().leave_warning, "leaving the page asks");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run();
    let download = browser.0.borrow().downloads.last().cloned().unwrap();
    assert_eq!(download.name, "abteilung.plnwrk");
    let saved = crate::OpenDoc::from_bytes(&download.bytes, None).unwrap();
    assert!(
        saved
            .session
            .workspace()
            .tasks()
            .any(|t| t.title == "Funkprobe")
    );
    assert!(
        !harness.state().has_unsaved_changes(),
        "downloaded counts as saved"
    );
    assert!(!browser.0.borrow().leave_warning);
}

#[test]
fn a_failed_download_is_not_saved() {
    let (mut harness, browser) = in_browser();
    click(&mut harness, text::t().open_sample);
    browser.0.borrow_mut().download_fails = Some("blockiert".to_owned());
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run();
    assert!(harness.state().has_unsaved_changes());
    let message = harness.state().state.message.clone().unwrap();
    assert!(message.error && message.text.starts_with(text::t().download_failed));
    assert!(browser.0.borrow().downloads.is_empty());
}

#[test]
fn uploading_over_unsaved_work_asks_first() {
    let (mut harness, browser) = in_browser();
    click(&mut harness, text::t().open_sample);
    let mut other = crate::OpenDoc::new();
    other
        .session
        .run(
            "project.create",
            &serde_json::json!({"title": "Neu"}),
            plnwrk_engine::Origin::User,
        )
        .unwrap();
    click(&mut harness, text::t().file_menu);
    click(&mut harness, text::t().open_file);
    choose_upload(&mut harness, &browser, "neu.plnwrk", file_bytes(&other));
    assert!(harness.query_by_label(text::t().unsaved_title).is_some());
    assert!(projects(harness.state()).len() > 1, "nothing replaced yet");
    click(&mut harness, text::t().discard);
    assert_eq!(harness.state().doc().unwrap().name(), "neu");
    assert_eq!(projects(harness.state()).len(), 1);
}

#[test]
fn an_encrypted_upload_asks_for_its_passphrase() {
    let (mut harness, browser) = in_browser();
    let sample = crate::OpenDoc::sample().unwrap();
    let (document, _) = sample.snapshot();
    let secret = plnwrk_store::SecretString::from("richtig-lang-genug".to_owned());
    let bytes = plnwrk_store::save_with(
        &document,
        &plnwrk_store::Protection::Passphrase(secret),
        plnwrk_store::SaveOptions {
            work_factor: Some(10),
        },
    )
    .unwrap();
    choose_upload(&mut harness, &browser, "geheim.plnwrk", bytes);
    assert!(
        harness.query_by_label("geheim.plnwrk").is_some(),
        "the dialog names the file"
    );
    let field = harness.get_by(|n| n.role() == egui::accesskit::Role::PasswordInput);
    field.type_text("falsch-und-lang");
    harness.run();
    click(&mut harness, text::t().open);
    let message = harness.state().state.message.clone().unwrap();
    assert!(message.error, "wrong passphrase: said, and asked again");
    let field = harness.get_by(|n| n.role() == egui::accesskit::Role::PasswordInput);
    field.type_text("richtig-lang-genug");
    harness.run();
    click(&mut harness, text::t().open);
    let app = harness.state();
    assert_eq!(app.doc().unwrap().name(), "geheim");
    assert!(app.doc().unwrap().encrypted());
}

#[test]
fn the_browser_has_no_quit() {
    let (mut harness, _browser) = in_browser();
    click(&mut harness, text::t().open_sample);
    let offered: Vec<String> =
        crate::palette::entries(harness.state().doc(), &harness.state().state)
            .into_iter()
            .map(|e| e.label)
            .collect();
    assert!(!offered.iter().any(|l| l == text::t().quit));
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Q);
    harness.run();
    assert!(!harness.state().wants_to_quit());
    assert!(harness.query_by_label(text::t().unsaved_title).is_none());
}

/// An encrypted VS-NfD file (cheap encryption for tests).
fn classified_file() -> Vec<u8> {
    let mut doc = crate::OpenDoc::new();
    doc.session
        .run(
            "project.create",
            &serde_json::json!({"title": "Geheim", "classification": "vs_nfd"}),
            plnwrk_engine::Origin::User,
        )
        .unwrap();
    let (document, _) = doc.snapshot();
    let secret = plnwrk_store::SecretString::from("richtig-lang-genug".to_owned());
    plnwrk_store::save_with(
        &document,
        &plnwrk_store::Protection::Passphrase(secret),
        plnwrk_store::SaveOptions {
            work_factor: Some(10),
        },
    )
    .unwrap()
}

#[test]
fn the_browser_refuses_vs_nfd_before_asking_for_a_passphrase() {
    let (mut harness, browser) = in_browser();
    assert!(
        harness.query_all_by_label(text::t().browser_not_vs).count() >= 1,
        "the start page says so"
    );
    choose_upload(&mut harness, &browser, "geheim.plnwrk", classified_file());
    let app = harness.state();
    assert!(app.doc().is_none());
    assert!(app.state.dialog.is_none(), "no passphrase asked");
    let message = app.state.message.clone().unwrap();
    assert!(message.error);
    assert_eq!(message.text, text::t().vs_blocked);
}

#[test]
fn the_browser_refuses_vs_nfd_before_it_reaches_the_history() {
    let (mut harness, browser) = in_browser();
    click(&mut harness, text::t().new_file);
    click(&mut harness, text::t().new_project);
    type_into(&mut harness, text::t().title, "Lage", false);
    click(&mut harness, text::t().create);
    let id = projects(harness.state()).first().unwrap().id.to_string();
    let log = |h: &Harness<'_, PlnwrkApp>| h.state().doc().unwrap().log_len();
    let before = log(&harness);
    for (command, params) in [
        (
            "project.update",
            serde_json::json!({"project": id, "classification": "vs_nfd"}),
        ),
        (
            "project.create",
            serde_json::json!({"title": "STRENG", "classification": "vs_nfd"}),
        ),
    ] {
        harness
            .state_mut()
            .apply(crate::Intent::run(command, params));
        assert_eq!(
            harness.state().state.message.clone().unwrap().text,
            text::t().vs_blocked
        );
        assert_eq!(log(&harness), before, "{command}: not even in the history");
    }
    // Nothing tainted: saving and the recovery copy still work.
    harness.run();
    assert!(!stored_copies(&browser).is_empty());
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run();
    let download = browser.0.borrow().downloads.last().cloned().unwrap();
    let saved = plnwrk_store::peek(&download.bytes).unwrap();
    assert_eq!(saved.classification, plnwrk_core::Classification::Open);
}

#[test]
fn a_file_that_ever_held_vs_nfd_never_leaves_the_browser() {
    let (mut harness, browser) = in_browser();
    click(&mut harness, text::t().new_file);
    // A way the parameter check does not foresee: straight into the session.
    {
        let app = harness.state_mut();
        let doc = app.doc.as_mut().unwrap();
        doc.session
            .run(
                "project.create",
                &serde_json::json!({"title": "STRENG", "classification": "vs_nfd"}),
                plnwrk_engine::Origin::User,
            )
            .unwrap();
        doc.session
            .run("undo", &serde_json::json!({}), plnwrk_engine::Origin::User)
            .unwrap();
    }
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run();
    assert!(
        browser.0.borrow().downloads.is_empty(),
        "not even encrypted"
    );
    assert_eq!(
        harness.state().state.message.clone().unwrap().text,
        text::t().vs_blocked
    );
}

#[test]
fn the_desktop_keeps_vs_nfd_and_shows_no_browser_hint() {
    let mut harness = harness(PlnwrkApp::default());
    assert_eq!(
        harness.query_all_by_label(text::t().browser_not_vs).count(),
        0
    );
    click(&mut harness, text::t().new_file);
    click(&mut harness, text::t().new_project);
    type_into(&mut harness, text::t().title, "Lage", false);
    click(&mut harness, text::t().create);
    let id = projects(harness.state()).first().unwrap().id.to_string();
    harness.state_mut().apply(crate::Intent::run(
        "project.update",
        serde_json::json!({"project": id, "classification": "vs_nfd"}),
    ));
    assert_eq!(
        harness
            .state()
            .doc()
            .unwrap()
            .session
            .workspace()
            .highest_classification(),
        plnwrk_core::Classification::VsNfd
    );
}

/// The recovery copies in the test browser's storage.
fn stored_copies(browser: &crate::host::TestBrowser) -> Vec<crate::browser_copy::StoredCopy> {
    browser
        .0
        .borrow()
        .storage
        .iter()
        .filter(|(k, _)| k.starts_with(crate::browser_copy::PREFIX))
        .filter_map(|(k, v)| crate::browser_copy::decode(k, v))
        .collect()
}

/// Clicks the start page entry of a recovery copy (its line also shows the time).
fn open_copy(harness: &mut Harness<'_, PlnwrkApp>, name: &str) {
    harness.get_by_label_contains(name).click();
    harness.run();
}

/// The page is loaded again: a new app on the same browser storage.
fn reload(browser: &crate::host::TestBrowser) -> Harness<'static, PlnwrkApp> {
    harness(PlnwrkApp::with_host(Box::new(browser.clone())))
}

#[test]
fn work_in_the_browser_survives_a_reload_until_it_is_downloaded() {
    let (mut harness, browser) = in_browser();
    click(&mut harness, text::t().new_file);
    assert!(stored_copies(&browser).is_empty(), "nothing to keep yet");
    click(&mut harness, text::t().new_project);
    type_into(&mut harness, text::t().title, "Lage", false);
    click(&mut harness, text::t().create);
    let copies = stored_copies(&browser);
    let [copy] = copies.as_slice() else {
        panic!("{copies:?}")
    };
    let copy = copy.file.clone();
    let kept = crate::OpenDoc::from_bytes(&copy.bytes, None).unwrap();
    assert!(
        kept.session
            .workspace()
            .projects()
            .any(|p| p.title == "Lage")
    );
    drop(harness);

    // Reloaded (or the tab crashed): the copy is offered and opens as unsaved work.
    let mut harness = reload(&browser);
    assert!(harness.query_by_label(text::t().recovered).is_some());
    open_copy(&mut harness, &copy.name);
    let app = harness.state();
    assert!(app.has_unsaved_changes(), "recovered work is unsaved");
    assert!(projects(app).iter().any(|p| p.title == "Lage"));
    assert_eq!(
        app.state.message.clone().unwrap().text,
        text::t().recovered_opened
    );
    // More work goes into the same copy, not a second one.
    palette(&mut harness, "lage");
    type_into(&mut harness, text::t().new_task_hint, "Bericht", true);
    assert_eq!(stored_copies(&browser).len(), 1);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run();
    assert_eq!(browser.0.borrow().downloads.len(), 1);
    // The browser cannot tell whether the file arrived (its dialog may have been cancelled):
    // the copy stays, marked, and is offered again after a reload.
    let copies = stored_copies(&browser);
    assert_eq!(copies.len(), 1);
    assert!(copies[0].downloaded);
    assert!(!harness.state().has_unsaved_changes());
    click(&mut harness, text::t().file_menu);
    click(&mut harness, text::t().close_file);
    assert_eq!(
        stored_copies(&browser).len(),
        1,
        "closed after a download: kept"
    );
    let harness = reload(&browser);
    assert!(
        harness
            .query_all_by_label_contains(text::t().copy_downloaded)
            .count()
            >= 1
    );
}
#[test]
fn discarding_drops_the_copy_but_keeps_a_recovered_one() {
    let (mut harness, browser) = in_browser();
    click(&mut harness, text::t().open_sample);
    assert_eq!(stored_copies(&browser).len(), 1);
    // Closed on purpose: its copy goes.
    click(&mut harness, text::t().file_menu);
    click(&mut harness, text::t().close_file);
    click(&mut harness, text::t().discard);
    assert!(stored_copies(&browser).is_empty());

    // A recovered copy that is only looked at and dropped stays on offer.
    click(&mut harness, text::t().open_sample);
    drop(harness);
    let mut harness = reload(&browser);
    let name = stored_copies(&browser)[0].file.name.clone();
    open_copy(&mut harness, &name);
    click(&mut harness, text::t().file_menu);
    click(&mut harness, text::t().close_file);
    click(&mut harness, text::t().discard);
    assert_eq!(stored_copies(&browser).len(), 1, "kept");
    assert_eq!(
        harness.state().state.stored_copies.len(),
        1,
        "and offered again"
    );

    // Deleted on purpose (asked first).
    click(&mut harness, text::t().delete_copy);
    assert!(
        harness
            .query_by_label(text::t().delete_copy_question)
            .is_some()
    );
    click(&mut harness, text::t().delete);
    assert!(stored_copies(&browser).is_empty());
    assert!(harness.state().state.stored_copies.is_empty());
}

#[test]
fn encrypted_work_is_not_kept_in_the_browser() {
    let (mut harness, browser) = in_browser();
    let sample = crate::OpenDoc::sample().unwrap();
    let (document, _) = sample.snapshot();
    let secret = plnwrk_store::SecretString::from("richtig-lang-genug".to_owned());
    let bytes = plnwrk_store::save_with(
        &document,
        &plnwrk_store::Protection::Passphrase(secret),
        plnwrk_store::SaveOptions {
            work_factor: Some(10),
        },
    )
    .unwrap();
    choose_upload(&mut harness, &browser, "geheim.plnwrk", bytes);
    let field = harness.get_by(|n| n.role() == egui::accesskit::Role::PasswordInput);
    field.type_text("richtig-lang-genug");
    harness.run();
    click(&mut harness, text::t().open);
    palette(&mut harness, "digi");
    type_into(&mut harness, text::t().new_task_hint, "Funkprobe", true);
    assert!(harness.state().has_unsaved_changes());
    assert!(
        stored_copies(&browser).is_empty(),
        "no plain copy of protected work"
    );
    assert_eq!(
        harness.state().state.recovery,
        Some(crate::autosave::Written::Failed(
            text::t().no_copy_encrypted.to_owned()
        ))
    );
}

#[test]
fn a_full_storage_is_reported_and_tried_again_later() {
    let (mut harness, browser) = in_browser();
    browser.0.borrow_mut().store_fails = Some("Speicher voll".to_owned());
    click(&mut harness, text::t().open_sample);
    let message = harness.state().state.message.clone().unwrap();
    assert!(
        message.error && message.text.contains("Speicher voll"),
        "{message:?}"
    );
    // Not every frame: the next try waits.
    browser.0.borrow_mut().store_fails = None;
    let now = harness.ctx.input(|i| i.time);
    let app = harness.state_mut();
    let wait = app.autosave_at(now + 1.0);
    assert!(wait.is_some_and(|w| w > 1.0), "{wait:?}");
    assert!(stored_copies(&browser).is_empty());
    // Written once it works again.
    assert!(app.autosave_at(1.0e6).is_none());
    assert_eq!(
        stored_copies(&browser).len(),
        1,
        "written once it works again"
    );
}

#[test]
fn rubbish_in_the_browser_storage_is_ignored() {
    let browser = crate::host::TestBrowser::default();
    browser.0.borrow_mut().storage.insert(
        format!("{}kaputt", crate::browser_copy::PREFIX),
        "{nicht json".to_owned(),
    );
    browser.0.borrow_mut().storage.insert(
        format!("{}keine-datei", crate::browser_copy::PREFIX),
        crate::browser_copy::encode("müll", b"PK not a plnwrk file", "", false),
    );
    browser
        .0
        .borrow_mut()
        .storage
        .insert("egui_memory_ron".to_owned(), "(…)".to_owned());
    let mut harness = reload(&browser);
    let offered = harness.state().state.stored_copies.len();
    assert_eq!(offered, 1, "only what decodes is offered");
    open_copy(&mut harness, "müll");
    assert!(harness.state().doc().is_none());
    assert!(harness.state().state.message.clone().unwrap().error);
}

/// A copy of the sample, written by another tab.
fn copy_of_another_tab(browser: &crate::host::TestBrowser, tab: &str, downloaded: bool) -> String {
    let sample = crate::OpenDoc::sample().unwrap();
    let key = format!("{}andere {tab}", crate::browser_copy::PREFIX);
    let value = crate::browser_copy::encode("andere", &file_bytes(&sample), tab, downloaded);
    browser.0.borrow_mut().storage.insert(key.clone(), value);
    key
}

#[test]
fn downloaded_copies_go_after_a_week() {
    let browser = crate::host::TestBrowser::default();
    let key = copy_of_another_tab(&browser, "alt", true);
    // Written eight days ago.
    let old = jiff::Timestamp::now()
        .checked_sub(jiff::SignedDuration::from_hours(8 * 24))
        .unwrap();
    {
        let mut log = browser.0.borrow_mut();
        let value = log.storage.get(&key).cloned().unwrap();
        let mut json: serde_json::Value = serde_json::from_str(&value).unwrap();
        json["time"] = serde_json::Value::String(old.to_string());
        log.storage.insert(key.clone(), json.to_string());
    }
    let harness = reload(&browser);
    assert!(harness.state().state.stored_copies.is_empty());
    assert!(!browser.0.borrow().storage.contains_key(&key), "removed");
    // Unsaved work is never removed by age.
    copy_of_another_tab(&browser, "alt", false);
    let harness = reload(&browser);
    assert_eq!(harness.state().state.stored_copies.len(), 1);
}

#[test]
fn a_file_that_cannot_be_read_is_said() {
    let (mut harness, browser) = in_browser();
    browser.0.borrow_mut().upload = Some(Err("NotReadableError".to_owned()));
    harness.run();
    let message = harness.state().state.message.clone().unwrap();
    assert!(message.error && message.text.starts_with(text::t().upload_failed));
}

#[test]
fn a_vs_nfd_upload_over_unsaved_work_is_refused_without_asking() {
    let (mut harness, browser) = in_browser();
    click(&mut harness, text::t().open_sample);
    choose_upload(&mut harness, &browser, "geheim.plnwrk", classified_file());
    assert!(
        harness.state().state.dialog.is_none(),
        "no question about unsaved work"
    );
    assert_eq!(
        harness.state().state.message.clone().unwrap().text,
        text::t().vs_blocked
    );
    assert!(
        harness.state().has_unsaved_changes(),
        "the work is still there"
    );
}

#[test]
fn protecting_in_the_browser_leaves_no_plain_copy() {
    let (mut harness, browser) = in_browser();
    click(&mut harness, text::t().open_sample);
    assert_eq!(
        stored_copies(&browser).len(),
        1,
        "a plain copy of unprotected work"
    );
    open_file_menu(&mut harness);
    click(&mut harness, text::t().protect);
    type_password(&mut harness, 0, "richtig-lange-passphrase");
    type_password(&mut harness, 1, "richtig-lange-passphrase");
    click(&mut harness, text::t().protect_button);
    let download = browser.0.borrow().downloads.last().cloned().unwrap();
    assert!(plnwrk_store::peek(&download.bytes).unwrap().encrypted);
    assert!(stored_copies(&browser).is_empty(), "the plain copy is gone");
}

#[test]
fn a_failed_rewrite_at_download_keeps_the_older_copy() {
    let (mut harness, browser) = in_browser();
    click(&mut harness, text::t().open_sample);
    assert_eq!(stored_copies(&browser).len(), 1);
    browser.0.borrow_mut().store_fails = Some("Speicher voll".to_owned());
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run();
    harness.run();
    assert_eq!(
        stored_copies(&browser).len(),
        1,
        "the download may not have arrived"
    );
}

#[test]
fn all_offered_copies_can_be_deleted_at_once() {
    let browser = crate::host::TestBrowser::default();
    copy_of_another_tab(&browser, "alt-1", false);
    copy_of_another_tab(&browser, "alt-2", true);
    let mut harness = reload(&browser);
    assert_eq!(harness.state().state.stored_copies.len(), 2);
    click(&mut harness, text::t().delete_all_copies);
    assert!(
        harness
            .query_by_label(text::t().delete_all_copies_question)
            .is_some()
    );
    click(&mut harness, text::t().delete);
    assert!(stored_copies(&browser).is_empty());
    assert!(harness.state().state.stored_copies.is_empty());
}

#[test]
fn copies_of_open_tabs_are_not_offered_to_another() {
    let browser = crate::host::TestBrowser::default();
    browser.0.borrow_mut().tab = "mein-tab".to_owned();
    copy_of_another_tab(&browser, "tab-b", false);
    copy_of_another_tab(&browser, "mein-tab", false);
    // The browser has not told yet which tabs are open: only the own copy.
    browser.0.borrow_mut().live_unknown = true;
    let mut harness = reload(&browser);
    let offered = |h: &Harness<'_, PlnwrkApp>| {
        h.state()
            .state
            .stored_copies
            .iter()
            .map(|c| c.tab.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(offered(&harness), ["mein-tab"]);
    // Tab B is open.
    browser.0.borrow_mut().live_unknown = false;
    browser.0.borrow_mut().live = vec!["mein-tab".to_owned(), "tab-b".to_owned()];
    browser.0.borrow_mut().live_changed = true;
    harness.run();
    assert_eq!(offered(&harness), ["mein-tab"]);
    // Tab B ended (closed, crashed, discarded): its copy is offered at once.
    browser.0.borrow_mut().live = vec!["mein-tab".to_owned()];
    browser.0.borrow_mut().live_changed = true;
    harness.run();
    let mut tabs = offered(&harness);
    tabs.sort();
    assert_eq!(tabs, ["mein-tab", "tab-b"]);
}

#[test]
fn signs_of_life_of_an_older_version_are_removed() {
    let browser = crate::host::TestBrowser::default();
    browser
        .0
        .borrow_mut()
        .storage
        .insert("plnwrk.live.alt".to_owned(), "1791997538 hidden".to_owned());
    let _harness = reload(&browser);
    assert!(!browser.0.borrow().storage.contains_key("plnwrk.live.alt"));
}

/// A project with two dated tasks, shown in its list: (project, first, second).
fn two_dated_tasks() -> (Harness<'static, PlnwrkApp>, TaskId, TaskId) {
    let mut harness = with_project("Funk");
    let project = projects(harness.state()).first().unwrap().id.to_string();
    for (title, start, due) in [
        ("Ausschreibung", "2026-10-12", "2026-10-14"),
        ("Vergabe", "2026-10-15", "2026-10-16"),
    ] {
        harness.state_mut().apply(crate::Intent::run(
            "task.create",
            serde_json::json!({"project": project, "title": title, "start": start, "due": due}),
        ));
    }
    harness.run();
    let first = task_named(harness.state(), "Ausschreibung").unwrap().id;
    let second = task_named(harness.state(), "Vergabe").unwrap().id;
    (harness, first, second)
}

#[test]
fn a_later_deadline_moves_what_waits_in_one_step() {
    let (mut harness, first, second) = two_dated_tasks();
    // "Vergabe" waits for "Ausschreibung", chosen in the detail panel.
    click(&mut harness, "Vergabe");
    harness
        .get_by(|n| n.value().as_deref() == Some(text::t().add_predecessor))
        .click();
    harness.run();
    harness
        .get_all_by_label("Ausschreibung")
        .last()
        .unwrap()
        .click();
    harness.run();
    let ws = |h: &Harness<'_, PlnwrkApp>| h.state().doc().unwrap().session.workspace().clone();
    assert!(
        ws(&harness)
            .dependencies()
            .any(|d| d.from == first && d.to == second)
    );
    // The first now ends on Friday: the second starts Monday, still two days long.
    // The list comes first (the detail panel names it too, under "Wartet auf").
    harness
        .get_all_by_label("Ausschreibung")
        .next()
        .unwrap()
        .click();
    harness.run();
    type_into(&mut harness, text::t().due, "16.10.2026", true);
    let day = |h: &Harness<'_, PlnwrkApp>, id: TaskId| {
        let t = ws(h).task(id).cloned().unwrap();
        (
            t.start.map(|d| d.to_string()).unwrap_or_default(),
            t.due.map(|d| d.to_string()).unwrap_or_default(),
        )
    };
    assert_eq!(
        day(&harness, first),
        ("2026-10-12".into(), "2026-10-16".into())
    );
    assert_eq!(
        day(&harness, second),
        ("2026-10-19".into(), "2026-10-20".into())
    );
    assert_eq!(
        harness.state().state.message.clone().unwrap().text,
        text::moved_along(1)
    );
    // One undo takes both back.
    harness.key_press(Key::Escape);
    harness.run();
    harness
        .state_mut()
        .apply(crate::Intent::run("undo", serde_json::json!({})));
    assert_eq!(
        day(&harness, first),
        ("2026-10-12".into(), "2026-10-14".into())
    );
    assert_eq!(
        day(&harness, second),
        ("2026-10-15".into(), "2026-10-16".into())
    );
}

#[test]
fn a_deadline_typed_and_left_by_saving_moves_what_waits_too() {
    let (mut harness, first, second) = two_dated_tasks();
    let link = serde_json::json!({"from": first.to_string(), "to": second.to_string()});
    harness
        .state_mut()
        .apply(crate::Intent::run("dependency.add", link));
    click(&mut harness, "Ausschreibung");
    // Typed, the field still has the focus; then saved, closed or another task chosen.
    type_into(&mut harness, text::t().due, "16.10.2026", false);
    assert!(harness.state_mut().flush_draft().is_none());
    let ws = harness.state().doc().unwrap().session.workspace().clone();
    let dates = |id: TaskId| {
        let t = ws.task(id).unwrap();
        (t.start.unwrap().to_string(), t.due.unwrap().to_string())
    };
    assert_eq!(dates(first), ("2026-10-12".into(), "2026-10-16".into()));
    assert_eq!(dates(second), ("2026-10-19".into(), "2026-10-20".into()));
}

#[test]
fn waits_for_does_not_offer_what_would_make_a_circle() {
    let (mut harness, first, second) = two_dated_tasks();
    let project = projects(harness.state()).first().unwrap().id.to_string();
    harness.state_mut().apply(crate::Intent::run(
        "task.create",
        serde_json::json!({"project": project, "title": "Abnahme"}),
    ));
    let third = task_named(harness.state(), "Abnahme").unwrap().id;
    for (from, to) in [(first, second), (second, third)] {
        let link = serde_json::json!({"from": from.to_string(), "to": to.to_string()});
        harness
            .state_mut()
            .apply(crate::Intent::run("dependency.add", link));
    }
    // Both others wait for the first (one through the other): nothing left to offer.
    click(&mut harness, "Ausschreibung");
    assert!(
        harness
            .query_by(|n| n.value().as_deref() == Some(text::t().add_predecessor))
            .is_none()
    );
    // The last may still wait for the first directly.
    click(&mut harness, "Abnahme");
    assert!(
        harness
            .query_by(|n| n.value().as_deref() == Some(text::t().add_predecessor))
            .is_some()
    );
}

#[test]
fn a_dependency_is_removed_in_the_detail_panel() {
    let (mut harness, first, second) = two_dated_tasks();
    let link = serde_json::json!({"from": first.to_string(), "to": second.to_string()});
    harness
        .state_mut()
        .apply(crate::Intent::run("dependency.add", link));
    click(&mut harness, "Vergabe");
    assert!(harness.query_by_label(text::t().waits_for).is_some());
    click(&mut harness, text::t().remove_dependency);
    assert_eq!(
        harness
            .state()
            .doc()
            .unwrap()
            .session
            .workspace()
            .dependencies()
            .count(),
        0
    );
}

/// The bar of a task in the schedule, by its title.
fn bar(harness: &Harness<'_, PlnwrkApp>, title: &str) -> egui::Rect {
    harness
        .get_by(|n| {
            n.label()
                .is_some_and(|l| l.starts_with(&format!("{title}: ")))
        })
        .rect()
}

#[test]
fn the_schedule_follows_every_change_and_its_titles_choose_tasks() {
    let (mut harness, _, second) = two_dated_tasks();
    click(&mut harness, text::t().schedule_tab);
    let before = bar(&harness, "Vergabe").left();
    // Worked out once and kept; a change (also an undo) must show at once.
    harness.state_mut().apply(crate::Intent::run(
        "task.update",
        serde_json::json!({"task": second.to_string(), "start": "2026-10-19", "due": "2026-10-20"}),
    ));
    harness.run();
    assert!(bar(&harness, "Vergabe").left() > before + 3.0 * 22.0);
    harness
        .state_mut()
        .apply(crate::Intent::run("undo", serde_json::json!({})));
    harness.run();
    assert!((bar(&harness, "Vergabe").left() - before).abs() < 1.0);
    // The title in the left column chooses the task.
    harness.get_all_by_label("Vergabe").next().unwrap().click();
    harness.run();
    assert_eq!(harness.state().state.selected, Some(second));
}

#[test]
fn a_push_too_big_for_one_step_is_refused_plainly() {
    let mut harness = with_project("Lang");
    let project = projects(harness.state()).first().unwrap().id.to_string();
    let count = plnwrk_engine::params::MOVES_MAX + 1;
    let mut ids = Vec::new();
    for n in 0..count {
        let outcome = harness
            .state_mut()
            .doc
            .as_mut()
            .unwrap()
            .session
            .run(
                "task.create",
                &serde_json::json!({"project": project, "title": format!("T{n}"), "due": "2026-10-12"}),
                plnwrk_engine::Origin::User,
            )
            .unwrap();
        let Some(plnwrk_core::EntityKey::Task(id)) = outcome.created.first().copied() else {
            panic!("no task");
        };
        ids.push(id);
    }
    // One fan: all wait for the first.
    for id in &ids[1..] {
        harness
            .state_mut()
            .doc
            .as_mut()
            .unwrap()
            .session
            .run(
                "dependency.add",
                &serde_json::json!({"from": ids[0].to_string(), "to": id.to_string()}),
                plnwrk_engine::Origin::User,
            )
            .unwrap();
    }
    harness.state_mut().apply(crate::Intent::Reschedule {
        task: ids[0],
        start: None,
        due: Some("2026-10-13".parse().unwrap()),
        due_time: None,
    });
    let message = harness.state().state.message.clone().unwrap();
    assert!(message.error);
    assert_eq!(message.text, text::too_many_moves(count - 1));
    let ws = harness.state().doc().unwrap().session.workspace().clone();
    assert_eq!(
        ws.task(ids[0]).unwrap().due.unwrap().to_string(),
        "2026-10-12",
        "nothing moved"
    );
}

#[test]
fn the_schedule_shows_bars_and_dragging_moves_what_waits() {
    let (mut harness, first, second) = two_dated_tasks();
    let link = serde_json::json!({"from": first.to_string(), "to": second.to_string()});
    harness
        .state_mut()
        .apply(crate::Intent::run("dependency.add", link));
    click(&mut harness, text::t().schedule_tab);
    assert!(
        harness
            .query_by_label("Ausschreibung: 12.10.2026 – 14.10.2026")
            .is_some()
    );
    assert!(harness.query_by_label(text::t().critical_hint).is_some());
    // Two days later, by dragging the bar.
    let from = bar(&harness, "Ausschreibung").center();
    let to = from + egui::vec2(2.0 * 22.0, 0.0);
    // A fixed number of frames: the bar's tooltip keeps asking for more while hovered.
    harness.drag_at(from);
    harness.run_steps(2);
    harness.hover_at(from + egui::vec2(10.0, 0.0));
    harness.run_steps(2);
    harness.hover_at(to);
    harness.run_steps(2);
    harness.drop_at(to);
    harness.run_steps(2);
    let ws = harness.state().doc().unwrap().session.workspace().clone();
    let day = |id: TaskId| {
        let t = ws.task(id).cloned().unwrap();
        (t.start.unwrap().to_string(), t.due.unwrap().to_string())
    };
    assert_eq!(day(first), ("2026-10-14".into(), "2026-10-16".into()));
    assert_eq!(day(second), ("2026-10-19".into(), "2026-10-20".into()));
    harness
        .state_mut()
        .apply(crate::Intent::run("undo", serde_json::json!({})));
    let ws = harness.state().doc().unwrap().session.workspace().clone();
    assert_eq!(
        ws.task(second).unwrap().start.unwrap().to_string(),
        "2026-10-15",
        "one undo takes the move and what moved along back"
    );
}

#[test]
fn the_schedule_warns_about_a_milestone_that_cannot_hold() {
    let (mut harness, first, _) = two_dated_tasks();
    let project = projects(harness.state()).first().unwrap().id.to_string();
    harness.state_mut().apply(crate::Intent::run(
        "task.create",
        serde_json::json!({"project": project, "title": "Abgabe", "due": "2026-10-13", "milestone": true}),
    ));
    let milestone = task_named(harness.state(), "Abgabe").unwrap().id;
    let link = serde_json::json!({"from": first.to_string(), "to": milestone.to_string()});
    harness
        .state_mut()
        .apply(crate::Intent::run("dependency.add", link));
    click(&mut harness, text::t().schedule_tab);
    let warning = format!("⚠ {}", text::milestone_unreachable("Abgabe", "14.10.2026"));
    assert!(harness.query_by_label(&warning).is_some(), "{warning}");
    click(&mut harness, &warning);
    assert_eq!(harness.state().state.selected, Some(milestone));
}

#[test]
fn an_empty_schedule_says_what_to_do() {
    let mut harness = with_project("Lage");
    type_into(&mut harness, text::t().new_task_hint, "Ohne Datum", true);
    palette(&mut harness, "zeitplan");
    assert_eq!(harness.state().state.tab, crate::state::Tab::Schedule);
    assert!(harness.query_by_label(text::t().no_scheduled).is_some());
    assert!(harness.query_by_label("Ohne Datum").is_some());
}

#[test]
fn the_schedule_speaks_english() {
    let (mut harness, first, second) = two_dated_tasks();
    let link = serde_json::json!({"from": first.to_string(), "to": second.to_string()});
    harness
        .state_mut()
        .apply(crate::Intent::run("dependency.add", link));
    to_english(&mut harness);
    click(&mut harness, text::EN.schedule_tab);
    let data = ["Ausschreibung", "Vergabe", "Funk"];
    let german: Vec<String> = shown_texts(&harness)
        .into_iter()
        .filter(|t| looks_german(t) && !data.iter().any(|d| t.contains(d)))
        .collect();
    assert!(german.is_empty(), "German in the schedule: {german:?}");
}
