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

/// Clicks what carries this label, scrolled into view first (like a person would).
fn click(harness: &mut Harness<'_, PlnwrkApp>, label: &str) {
    harness.get_by_label(label).scroll_to_me();
    harness.run();
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
    assert!(
        harness
            .query_by_label(text::t().today_and_overdue)
            .is_some()
    );
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
    // Left: the mistake is said at the field, and "Anlegen" waits (one message, one place).
    type_into(&mut harness, text::t().title, "Umzug", false);
    let said = crate::dates::parse("31.02.2026").unwrap_err();
    assert!(harness.query_by_label(&said).is_some(), "{said}");
    harness.get_by_label(text::t().create).click();
    harness.run();
    let app = harness.state();
    assert!(projects(app).is_empty());
    assert!(app.state.dialog.is_some(), "input stays");
    assert!(app.state.message.is_none(), "{:?}", app.state.message);
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

    add_subtask_to(&mut harness, "Lage vorbereiten");
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

    click(&mut harness, text::t().undo);
    assert!(task_named(harness.state(), "Kisten").is_none());
    click(&mut harness, text::t().redo);
    assert!(task_named(harness.state(), "Kisten").is_some());

    // Undo everything: the project disappears and the view falls back to the overview.
    while harness.state().doc().is_some_and(|d| d.session.can_undo()) {
        click(&mut harness, text::t().undo);
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
        ..Scripted::default()
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

/// A project with "Kisten" and its two subtasks "Bücher" and "Geschirr", on the board.
fn board_with_subtasks() -> Harness<'static, PlnwrkApp> {
    let mut harness = with_project("Umzug");
    type_into(&mut harness, text::t().new_task_hint, "Kisten", true);
    let parent = task_named(harness.state(), "Kisten").unwrap().id;
    let project = parent_project(&harness, parent);
    for title in ["Bücher", "Geschirr"] {
        harness.state_mut().apply(crate::Intent::run(
            "task.create",
            serde_json::json!({"project": project, "title": title, "parent": parent.to_string()}),
        ));
    }
    harness.run();
    click(&mut harness, text::t().board);
    harness
}

fn parent_project(harness: &Harness<'_, PlnwrkApp>, task: TaskId) -> String {
    harness
        .state()
        .doc()
        .unwrap()
        .session
        .workspace()
        .task(task)
        .unwrap()
        .project
        .to_string()
}

/// Drags the card with this title into the column with this heading.
fn drag_card(harness: &mut Harness<'_, PlnwrkApp>, title: &str, column: &str) {
    let card = harness.get_by_label(title).rect().center();
    let target = harness.get_by_label(column).rect().center() + egui::vec2(0.0, 60.0);
    harness.drag_at(card);
    harness.run();
    harness.hover_at(card + egui::vec2(10.0, 10.0));
    harness.run();
    harness.hover_at(target);
    harness.run();
    harness.drop_at(target);
    harness.run();
}

#[test]
fn subtasks_are_in_their_parents_card_and_ticked_off_there() {
    let mut harness = board_with_subtasks();
    // Not cards of their own: the "Offen" column holds one card.
    assert!(harness.query_by_label("Offen (1)").is_some());
    harness
        .get_by(|n| {
            n.role() == egui::accesskit::Role::CheckBox && n.label().as_deref() == Some("Bücher")
        })
        .click();
    harness.run();
    assert_eq!(
        task_named(harness.state(), "Bücher").unwrap().status,
        TaskStatus::Done
    );
    assert_eq!(
        task_named(harness.state(), "Kisten").unwrap().status,
        TaskStatus::Todo,
        "the parent keeps its own status"
    );
}

#[test]
fn a_card_with_open_subtasks_asks_before_it_is_done() {
    let mut harness = board_with_subtasks();
    let status =
        |h: &Harness<'_, PlnwrkApp>, title: &str| task_named(h.state(), title).unwrap().status;
    // Moving within the open columns: the subtasks keep their status, no question.
    drag_card(&mut harness, "Kisten", "In Arbeit (0)");
    assert_eq!(status(&harness, "Kisten"), TaskStatus::InProgress);
    assert_eq!(status(&harness, "Bücher"), TaskStatus::Todo);
    // To done: asked; "only this task" leaves the subtasks open.
    drag_card(&mut harness, "Kisten", "Erledigt (0)");
    assert!(
        harness
            .query_by_label(text::t().close_subtasks_title)
            .is_some()
    );
    assert!(
        harness
            .query_by_label(&text::subtasks_still_open(2))
            .is_some()
    );
    assert_eq!(
        status(&harness, "Kisten"),
        TaskStatus::InProgress,
        "not before the answer"
    );
    click(&mut harness, text::t().only_this_task);
    assert_eq!(status(&harness, "Kisten"), TaskStatus::Done);
    assert_eq!(status(&harness, "Bücher"), TaskStatus::Todo);
    // Back, then "close them all": one step, one undo.
    harness
        .state_mut()
        .apply(crate::Intent::run("undo", serde_json::json!({})));
    harness.run();
    drag_card(&mut harness, "Kisten", "Erledigt (0)");
    click(&mut harness, text::t().close_all_subtasks);
    for title in ["Kisten", "Bücher", "Geschirr"] {
        assert_eq!(status(&harness, title), TaskStatus::Done, "{title}");
    }
    harness
        .state_mut()
        .apply(crate::Intent::run("undo", serde_json::json!({})));
    assert_eq!(status(&harness, "Geschirr"), TaskStatus::Todo);
    assert_eq!(status(&harness, "Kisten"), TaskStatus::InProgress);
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
        ..Scripted::default()
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
        ..Scripted::default()
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
        ..Scripted::default()
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
    click(&mut harness, text::t().undo);
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
            ..Scripted::default()
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
        ..Scripted::default()
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
    // A case has the time field open at once (the one rule, `is_case`).
    assert!(
        harness
            .query_by(|n| n.placeholder() == Some(text::t().time_hint))
            .is_some()
    );
    // A project: the time field opens on request.
    click(&mut harness, text::t().kinds[0]);
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
    add_subtask_to(&mut harness, "Packen");
    type_into(&mut harness, text::t().new_subtask_hint, "Kartons", true);
    click(&mut harness, "Übergabe");
    harness
        .get_by_role_and_label(egui::accesskit::Role::CheckBox, text::t().milestone)
        .click();
    harness.run();
    // Making it a milestone is a change of milestones: confirmed first.
    assert!(!task_named(harness.state(), "Übergabe").unwrap().milestone);
    click(&mut harness, text::t().change_button);
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
        ..Scripted::default()
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

/// Switches the interface to English through the ribbon, like a person.
fn to_english(harness: &mut Harness<'_, PlnwrkApp>) {
    click(harness, text::DE.view_menu);
    click(
        harness,
        &format!(
            "{}: {}",
            text::DE.language_menu,
            text::Language::English.name()
        ),
    );
    assert_eq!(harness.state().state.language, text::Language::English);
    // Back to the tab Home, so names of the tab View are not there twice.
    click(harness, text::EN.ribbon.start);
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
        click(&mut first, &format!("{}: {name}", text::EN.appearance));
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
        ..Scripted::default()
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
        ..Scripted::default()
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
        ..Scripted::default()
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
    // The tab Datei is still open from closing: back to Start, as a person would.
    click(&mut harness, text::t().ribbon.start);
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
    // The first now ends on Friday: the second starts Saturday (every day counts), still two days long.
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
        ("2026-10-17".into(), "2026-10-18".into())
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
    assert_eq!(dates(second), ("2026-10-17".into(), "2026-10-18".into()));
}

#[test]
fn waits_for_greys_what_would_make_a_circle_and_says_why() {
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
    let greyed = |h: &Harness<'_, PlnwrkApp>, title: &str| {
        h.query_all_by_label(title)
            .any(|n| egui_kittest::kittest::NodeT::accesskit_node(&n).is_disabled())
    };
    let open_choice = |h: &mut Harness<'_, PlnwrkApp>| {
        h.get_by(|n| n.value().as_deref() == Some(text::t().add_predecessor))
            .click();
        h.run();
    };
    // Both others wait for the first (one through the other): offered, but greyed (Phase 9f:
    // not missing without a word).
    click(&mut harness, "Ausschreibung");
    open_choice(&mut harness);
    assert!(greyed(&harness, "Vergabe") && greyed(&harness, "Abnahme"));
    harness.key_press(Key::Escape);
    harness.run();
    // The last may still wait for the first directly; what it waits for already is greyed.
    click(&mut harness, "Abnahme");
    open_choice(&mut harness);
    assert!(!greyed(&harness, "Ausschreibung"));
    assert!(greyed(&harness, "Vergabe"));
}

/// The reasons of the circle rule (one rule for the details and the schedule).
#[test]
fn the_circle_rule_says_why() {
    let (harness, first, second) = two_dated_tasks();
    let ws = harness.state().doc().unwrap().session.workspace().clone();
    let mut ws = ws;
    ws.put(plnwrk_core::Entity::Dependency(plnwrk_core::Dependency {
        id: plnwrk_core::DependencyId::from_ulid(ulid::Ulid(99)),
        from: first,
        to: second,
        kind: plnwrk_core::DependencyKind::FinishToStart,
        lag_days: 0,
    }));
    let of_second = crate::views::detail::precede_refusal(&ws, second);
    assert_eq!(of_second(second), Some(text::t().not_itself));
    assert_eq!(of_second(first), Some(text::t().waits_already));
    let of_first = crate::views::detail::precede_refusal(&ws, first);
    assert_eq!(of_first(second), Some(text::t().would_circle));
}

#[test]
fn changes_by_an_ai_are_marked_in_the_history() {
    let (mut harness, first, _) = two_dated_tasks();
    harness
        .state_mut()
        .doc
        .as_mut()
        .unwrap()
        .session
        .run(
            "task.update",
            &serde_json::json!({"task": first.to_string(), "title": "Ausschreibung (KI)"}),
            plnwrk_engine::Origin::Assistant,
        )
        .unwrap();
    // Tall enough that the history is in view.
    harness.set_size(egui::vec2(1200.0, 1600.0));
    harness.run();
    click(&mut harness, "Ausschreibung (KI)");
    click(&mut harness, text::t().history);
    let marked = harness.query_all_by_label_contains(text::t().by_ai).count();
    assert_eq!(marked, 1, "only the change by the AI");
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
fn the_drag_preview_names_the_working_days_it_will_land_on() {
    let (mut harness, first, _) = two_dated_tasks();
    click(&mut harness, text::t().schedule_tab);
    // Mon 12 – Wed 14, dragged four days on: Friday 16th, so it lands Fri 16 – Sun 18 (every day counts).
    let from = bar(&harness, "Ausschreibung").center();
    let to = from + egui::vec2(4.0 * 22.0, 0.0);
    harness.drag_at(from);
    harness.run_steps(2);
    harness.hover_at(from + egui::vec2(10.0, 0.0));
    harness.run_steps(2);
    harness.hover_at(to);
    harness.run_steps(2);
    assert!(
        harness
            .query_by_label_contains("Ausschreibung: 16.10.2026 – 18.10.2026")
            .is_some()
    );
    harness.drop_at(to);
    harness.run_steps(2);
    let ws = harness.state().doc().unwrap().session.workspace().clone();
    let t = ws.task(first).unwrap();
    assert_eq!(
        (t.start.unwrap().to_string(), t.due.unwrap().to_string()),
        ("2026-10-16".into(), "2026-10-18".into()),
        "lands where the preview showed"
    );
}

#[test]
fn bars_hidden_under_the_titles_cannot_be_taken() {
    let mut harness = with_project("Alt und neu");
    let project = projects(harness.state()).first().unwrap().id.to_string();
    let today = harness.state().doc().unwrap().session.clock().today();
    let day = |days: i64| (today + jiff::Span::new().days(days)).to_string();
    for (title, start, due) in [("Alt", day(-60), day(-58)), ("Jetzt", day(0), day(2))] {
        harness.state_mut().apply(crate::Intent::run(
            "task.create",
            serde_json::json!({"project": project, "title": title, "start": start, "due": due}),
        ));
    }
    harness.run();
    click(&mut harness, text::t().schedule_tab);
    harness.run();
    // Opened a week before today: the old bar lies under the column with the titles.
    let has_bar = |h: &Harness<'_, PlnwrkApp>, title: &str| {
        h.query_by(|n| {
            n.label()
                .is_some_and(|l| l.starts_with(&format!("{title}: ")))
        })
        .is_some()
    };
    assert!(has_bar(&harness, "Jetzt"));
    assert!(!has_bar(&harness, "Alt"));
}

#[test]
fn saving_a_date_and_a_text_together_keeps_the_moved_along_message() {
    let (mut harness, first, second) = two_dated_tasks();
    let link = serde_json::json!({"from": first.to_string(), "to": second.to_string()});
    harness
        .state_mut()
        .apply(crate::Intent::run("dependency.add", link));
    click(&mut harness, "Ausschreibung");
    type_into(&mut harness, text::t().due, "16.10.2026", false);
    harness
        .state_mut()
        .state
        .forms
        .task
        .as_mut()
        .unwrap()
        .description = "Angebote".to_owned();
    assert!(harness.state_mut().flush_draft().is_none());
    let ws = harness.state().doc().unwrap().session.workspace().clone();
    assert_eq!(ws.task(first).unwrap().description, "Angebote");
    assert_eq!(
        harness.state().state.message.clone().unwrap().text,
        text::moved_along(1)
    );
}

#[test]
fn a_refused_typed_date_is_put_back_and_named() {
    let (mut harness, first, _) = two_dated_tasks();
    click(&mut harness, "Ausschreibung");
    // A deadline before the start.
    type_into(&mut harness, text::t().due, "01.10.2026", false);
    let warning = harness.state_mut().flush_draft().unwrap();
    assert!(warning.starts_with(text::t().not_taken), "{warning}");
    let ws = harness.state().doc().unwrap().session.workspace().clone();
    assert_eq!(
        ws.task(first).unwrap().due.unwrap().to_string(),
        "2026-10-14"
    );
    assert_eq!(
        harness.state().state.forms.task.as_ref().unwrap().due,
        "14.10.2026"
    );
}

#[test]
fn a_date_changed_elsewhere_is_not_written_back_by_saving() {
    let (mut harness, first, _) = two_dated_tasks();
    click(&mut harness, "Ausschreibung");
    // A deadline typed; meanwhile (no frame in between) the start changes elsewhere.
    type_into(&mut harness, text::t().due, "16.10.2026", false);
    harness
        .state_mut()
        .doc
        .as_mut()
        .unwrap()
        .session
        .run(
            "task.update",
            &serde_json::json!({"task": first.to_string(), "start": "2026-10-13"}),
            plnwrk_engine::Origin::User,
        )
        .unwrap();
    assert!(harness.state_mut().flush_draft().is_none());
    let ws = harness.state().doc().unwrap().session.workspace().clone();
    let t = ws.task(first).unwrap();
    assert_eq!(t.start.unwrap().to_string(), "2026-10-13", "not put back");
    assert_eq!(t.due.unwrap().to_string(), "2026-10-16");
}

#[test]
fn the_schedule_zooms_by_named_levels() {
    let (mut harness, _, _) = two_dated_tasks();
    click(&mut harness, text::t().schedule_tab);
    let width = |h: &Harness<'_, PlnwrkApp>| bar(h, "Ausschreibung").width();
    // Three days (12th to 14th): 3 days wide less a pixel at each end.
    let [quarter, month, week, day] = text::t().zoom_levels;
    for (name, per_day) in [(day, 48.0), (month, 8.0), (quarter, 3.0), (week, 22.0)] {
        // The whole project in the middle first: the zoom keeps the middle, so the bar stays
        // in view at any level.
        click(&mut harness, text::t().whole_project);
        click(&mut harness, name);
        let expected = (3.0 * per_day - 2.0_f32).max(4.0);
        assert!(
            (width(&harness) - expected).abs() < 1.0,
            "{name}: {} instead of {expected}",
            width(&harness)
        );
    }
    // Ctrl/⌘ and the mouse wheel (a pinch) zoom in too, anywhere in the schedule: from week
    // to day.
    let over = egui::pos2(1000.0, bar(&harness, "Ausschreibung").center().y);
    harness.hover_at(over);
    harness.run();
    harness.event(egui::Event::Zoom(1.5));
    harness.run();
    assert!((width(&harness) - (3.0 * 48.0 - 2.0)).abs() < 1.0);
}

/// Opens the sample's project "Digitalfunk Umrüstung Wachen" in the schedule tab.
fn sample_schedule(harness: &mut Harness<'_, PlnwrkApp>) {
    click(harness, text::t().open_sample);
    harness
        .get_all_by_label("Digitalfunk Umrüstung Wachen")
        .next()
        .unwrap()
        .click();
    harness.run();
    click(harness, text::t().schedule_tab);
}

#[test]
fn the_schedule_is_exported_as_a_table_for_excel() {
    let dir = scratch("export");
    let path = dir.join("zeitplan.csv");
    let host = Scripted {
        export: vec![path.clone()],
        ..Scripted::default()
    };
    let mut harness = harness(PlnwrkApp::with_host(Box::new(host)));
    sample_schedule(&mut harness);
    click(&mut harness, text::t().export_schedule);
    let csv = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = csv.lines().collect();
    assert!(
        lines[0].starts_with("\u{FEFF}Aufgabe;Ebene;Oberaufgabe;Status"),
        "{}",
        lines[0]
    );
    let row = |title: &str| {
        lines
            .iter()
            .find(|l| l.starts_with(&format!("{title};")))
            .unwrap_or_else(|| panic!("{title}: {csv}"))
            .split(';')
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    let install = row("Einbau Wache 11");
    assert_eq!(install[1], "1");
    assert_eq!(install[5], "02.11.2026", "German dates for German Excel");
    assert_eq!(install[6], "20.11.2026");
    assert_eq!(install[8], "19", "every day counts: 2 Nov to 20 Nov");
    assert_eq!(
        install[12], "Ausschreibung vorbereiten",
        "what it waits for"
    );
    let sub = row("Vergabevermerk");
    assert_eq!(sub[1], "2");
    assert_eq!(sub[2], "Ausschreibung vorbereiten");
    assert_eq!(row("Inbetriebnahme")[11], "ja");
    assert!(
        harness
            .state()
            .state
            .message
            .as_ref()
            .is_some_and(|m| !m.error && m.text.contains("zeitplan.csv"))
    );
    // Cancelled dialog: nothing written, no message about it.
    let before = harness.state().state.message.clone();
    click(&mut harness, text::t().export_schedule);
    assert_eq!(harness.state().state.message, before);
}

#[test]
fn a_classified_schedule_carries_the_marking_and_never_leaves_the_browser() {
    let dir = scratch("export-vs");
    let path = dir.join("zeitplan.csv");
    let host = Scripted {
        export: vec![path.clone()],
        ..Scripted::default()
    };
    let mut harness = harness(PlnwrkApp::with_host(Box::new(host)));
    sample_schedule(&mut harness);
    let project = projects(harness.state())
        .into_iter()
        .find(|p| p.title == "Digitalfunk Umrüstung Wachen")
        .unwrap()
        .id;
    harness.state_mut().apply(crate::Intent::run(
        "project.update",
        serde_json::json!({"project": project.to_string(), "classification": "vs_nfd"}),
    ));
    harness.run();
    click(&mut harness, text::t().export_schedule);
    let csv = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        csv.lines().next(),
        Some(format!("\u{FEFF}{}", plnwrk_store::export::VS_NFD_MARK).as_str())
    );
    // In the browser: a download, never of anything classified (it cannot get there anyway).
    let (mut harness, browser) = in_browser();
    sample_schedule(&mut harness);
    click(&mut harness, text::t().export_schedule);
    let downloads = &browser.0.borrow().downloads;
    assert_eq!(downloads.len(), 1);
    assert!(downloads[0].name.ends_with(".csv"), "{}", downloads[0].name);
    assert!(String::from_utf8_lossy(&downloads[0].bytes).contains("Einbau Wache 11"));
}

#[test]
fn one_notch_of_the_wheel_is_one_zoom_level() {
    let (mut harness, _, _) = two_dated_tasks();
    click(&mut harness, text::t().schedule_tab);
    let width = |h: &Harness<'_, PlnwrkApp>| bar(h, "Ausschreibung").width();
    let start = width(&harness);
    harness.hover_at(bar(&harness, "Ausschreibung").center());
    harness.run();
    // egui spreads one notch (or a pinch) over several frames.
    for _ in 0..5 {
        harness.event(egui::Event::Zoom(1.05));
        harness.step();
    }
    harness.run();
    let one = width(&harness);
    // From week (22 px a day) to day (48 px), not further.
    assert!(
        one > start * 1.9 && one < start * 2.4,
        "{start} → {one}: one level only"
    );
}

#[test]
fn zooming_keeps_the_dates_in_view() {
    let mut harness = with_project("Weit");
    let project = projects(harness.state()).first().unwrap().id.to_string();
    let today = harness.state().doc().unwrap().session.clock().today();
    let day = |days: i64| (today + jiff::Span::new().days(days)).to_string();
    for (title, start, due) in [
        ("Früher", day(-60), day(-58)),
        ("Jetzt", day(0), day(2)),
        ("Später", day(60), day(62)),
    ] {
        harness.state_mut().apply(crate::Intent::run(
            "task.create",
            serde_json::json!({"project": project, "title": title, "start": start, "due": due}),
        ));
    }
    harness.run();
    click(&mut harness, text::t().schedule_tab);
    harness.run();
    // From the middle of the visible chart to the bar: the day in the middle stays in the
    // middle (plan §7.1), so the distance grows with the zoom.
    let gap = |h: &Harness<'_, PlnwrkApp>| {
        let view = chart_view(h);
        bar(h, "Jetzt").center().x - view.center().x
    };
    let before = gap(&harness);
    click(&mut harness, text::t().zoom_levels[3]);
    let after = gap(&harness);
    assert!(
        (after - before * 48.0 / 22.0).abs() < 30.0,
        "{before} → {after}: the day in the middle stays there"
    );
}

#[test]
fn subtasks_of_subtasks_are_in_the_card_too() {
    let mut harness = board_with_subtasks();
    let books = task_named(harness.state(), "Bücher").unwrap().id;
    let project = parent_project(&harness, books);
    harness.state_mut().apply(crate::Intent::run(
        "task.create",
        serde_json::json!({"project": project, "title": "Krimis", "parent": books.to_string()}),
    ));
    harness.run();
    harness
        .get_by(|n| {
            n.role() == egui::accesskit::Role::CheckBox && n.label().as_deref() == Some("Krimis")
        })
        .click();
    harness.run();
    assert_eq!(
        task_named(harness.state(), "Krimis").unwrap().status,
        TaskStatus::Done
    );
}

#[test]
fn an_export_never_replaces_the_open_file_and_always_ends_in_csv() {
    let dir = scratch("export-guard");
    let path = dir.join("abteilung.plnwrk");
    let plain = dir.join("tabelle");
    let host = Scripted {
        save: vec![path.clone()],
        // The person picks the open file itself, then a name without an ending.
        export: vec![path.clone(), plain.clone()],
        ..Scripted::default()
    };
    let mut harness = harness(PlnwrkApp::with_host(Box::new(host)));
    sample_schedule(&mut harness);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run();
    let before = std::fs::read(&path).unwrap();
    click(&mut harness, text::t().export_schedule);
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "the workspace file is untouched"
    );
    assert!(dir.join("abteilung.plnwrk.csv").exists());
    click(&mut harness, text::t().export_schedule);
    assert!(
        std::fs::read_to_string(dir.join("tabelle.csv"))
            .unwrap()
            .contains("Einbau Wache 11")
    );
}

#[test]
fn a_project_without_dates_can_be_exported_too() {
    let mut harness = with_project("Ohne Termine");
    type_into(&mut harness, text::t().new_task_hint, "Kisten", true);
    click(&mut harness, text::t().schedule_tab);
    assert!(harness.query_by_label(text::t().schedule_empty).is_some());
    assert!(harness.query_by_label(text::t().export_schedule).is_some());
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
            .query_by_label_contains("Ausschreibung: 12.10.2026 – 14.10.2026")
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
    assert_eq!(day(second), ("2026-10-17".into(), "2026-10-18".into()));
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
    assert!(harness.query_by_label(text::t().schedule_empty).is_some());
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

/// Moves the pointer and presses in one frame, releases in the next – what a touch screen or a
/// quick click delivers, unlike [`egui_kittest::kittest::NodeT::click`] (one frame per event).
fn tap(harness: &mut Harness<'_, PlnwrkApp>, at: egui::Pos2) {
    let button = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    let events = &mut harness.input_mut().events;
    events.push(egui::Event::PointerMoved(at));
    events.push(button(true));
    harness.step();
    harness.input_mut().events.push(button(false));
    harness.run();
}

/// The next click after a dialog closes counts (Phase 9a, real test 2026-10-17): egui blocks
/// input one frame longer than a modal is shown, and the app sleeps until the next input – if
/// that input is the click itself, it was lost.
#[test]
fn the_first_click_after_a_dialog_counts() {
    let mut harness = harness_with_sample();
    let closing: [fn(&mut Harness<'_, PlnwrkApp>); 3] = [
        // The palette, closed with Escape (only the press: a release would spend the frame).
        |h| {
            h.key_press_modifiers(Modifiers::COMMAND, Key::K);
            h.run();
            h.event(egui::Event::Key {
                key: Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            });
        },
        // "From template", closed with its button.
        |h| {
            click(h, text::t().from_template);
            h.get_by_label(text::t().cancel).click();
        },
        // Deleting a task, cancelled.
        |h| {
            let task = task_named(h.state(), "Vergabevermerk").unwrap().id;
            h.state_mut()
                .state
                .open_dialog(crate::state::Dialog::DeleteTask(task));
            h.run();
            h.get_by_label(text::t().cancel).click();
        },
    ];
    harness
        .get_all_by_label("Digitalfunk Umrüstung Wachen")
        .next()
        .unwrap()
        .click();
    harness.run();
    for (n, close) in closing.iter().enumerate() {
        click(&mut harness, text::t().list);
        // Where the tab is (it does not move; without the fix the closed palette would still
        // be drawn in the next frame, with an entry of the same name).
        let at = harness.get_by_label(text::t().schedule_tab).rect().center();
        close(&mut harness);
        // The closing input and nothing more: the next press comes in the very next frame.
        harness.step();
        assert!(harness.state().state.dialog.is_none(), "dialog {n} closed");
        // Like a tap or a quick click: the pointer arrives and presses in the first frame
        // after the dialog is gone.
        tap(&mut harness, at);
        assert_eq!(
            harness.state().state.tab,
            Tab::Schedule,
            "click after dialog {n}"
        );
    }
    // "Discard" in the question about unsaved work: the sample opens anew, on the overview.
    let sidebar_project = harness
        .get_all_by_label("Digitalfunk Umrüstung Wachen")
        .next()
        .unwrap()
        .rect()
        .center();
    harness
        .state_mut()
        .state
        .open_dialog(crate::state::Dialog::Unsaved(
            crate::state::FileAction::Sample,
        ));
    harness.run();
    harness.get_by_label(text::t().discard).click();
    harness.step();
    assert!(harness.state().state.dialog.is_none(), "discarded");
    tap(&mut harness, sidebar_project);
    assert!(
        matches!(harness.state().state.view, View::Project(_)),
        "click after discarding"
    );
}

/// A project whose dates reach far beyond the window: from 60 days ago to 60 days ahead, with
/// `extra` more tasks today. Shown in the schedule tab.
fn wide_schedule(extra: usize) -> Harness<'static, PlnwrkApp> {
    let mut harness = with_project("Weit");
    let project = projects(harness.state()).first().unwrap().id.to_string();
    let today = harness.state().doc().unwrap().session.clock().today();
    let day = |days: i64| (today + jiff::Span::new().days(days)).to_string();
    let mut all = vec![
        ("Früher".to_owned(), day(-60), day(-58)),
        ("Jetzt".to_owned(), day(0), day(2)),
        ("Später".to_owned(), day(60), day(62)),
    ];
    all.extend((0..extra).map(|n| (format!("Zeile {n}"), day(0), day(1))));
    for (title, start, due) in all {
        harness.state_mut().apply(crate::Intent::run(
            "task.create",
            serde_json::json!({"project": project, "title": title, "start": start, "due": due}),
        ));
    }
    harness.run();
    click(&mut harness, text::t().schedule_tab);
    harness.run();
    harness
}

/// Turns the mouse wheel one notch down (towards the person) at this point of the chart.
fn wheel(harness: &mut Harness<'_, PlnwrkApp>, at: egui::Pos2, modifiers: Modifiers) {
    harness.hover_at(at);
    harness.run();
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: egui::vec2(0.0, -3.0),
        phase: egui::TouchPhase::Move,
        modifiers,
    });
    // egui scrolls smoothly, over many frames.
    harness.run_steps(60);
}

#[test]
fn the_wheel_moves_through_time_when_all_rows_fit() {
    let mut harness = wide_schedule(0);
    let before = bar(&harness, "Jetzt");
    wheel(
        &mut harness,
        before.right_center() + egui::vec2(20.0, 0.0),
        Modifiers::NONE,
    );
    let after = bar(&harness, "Jetzt");
    assert!(
        after.left() < before.left() - 20.0,
        "{before:?} → {after:?}"
    );
    assert!((after.top() - before.top()).abs() < 1.0);
}

#[test]
fn the_wheel_moves_the_rows_when_they_do_not_fit_and_shift_moves_through_time() {
    let mut harness = wide_schedule(40);
    let before = bar(&harness, "Jetzt");
    let at = before.right_center() + egui::vec2(20.0, 0.0);
    wheel(&mut harness, at, Modifiers::NONE);
    let rows = bar(&harness, "Jetzt");
    assert!(rows.top() < before.top() - 20.0, "{before:?} → {rows:?}");
    assert!((rows.left() - before.left()).abs() < 1.0);
    wheel(&mut harness, at, Modifiers::SHIFT);
    let time = bar(&harness, "Jetzt");
    assert!(time.left() < rows.left() - 20.0, "{rows:?} → {time:?}");
    assert!((time.top() - rows.top()).abs() < 1.0);
}

#[test]
fn ctrl_and_the_wheel_zoom_around_the_pointer() {
    let mut harness = wide_schedule(0);
    let before = bar(&harness, "Jetzt");
    // The pointer stands right of the bar; the day under it stays put.
    let pointer = before.center().x + 40.0;
    // Up, away from the person: zoom in.
    harness.hover_at(egui::pos2(pointer, before.center().y));
    harness.run();
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: egui::vec2(0.0, 3.0),
        phase: egui::TouchPhase::Move,
        modifiers: Modifiers::COMMAND,
    });
    harness.run_steps(60);
    let after = bar(&harness, "Jetzt");
    assert!(
        after.width() > before.width() * 1.3,
        "zoomed in: {before:?} → {after:?}"
    );
    let scale = after.width() / before.width();
    let expected = pointer - (pointer - before.left()) * scale;
    assert!(
        (after.left() - expected).abs() < 6.0,
        "the day under the pointer stays: left {} expected {expected}",
        after.left()
    );
}

/// A pinch on the trackpad (or Ctrl/⌘ and the wheel) zooms anywhere in the schedule, also
/// below the last row and over the title column – not only over the bars (Tobi, 2026-10-17).
#[test]
fn a_pinch_zooms_anywhere_in_the_schedule() {
    for place in ["below the rows", "over the titles"] {
        let mut harness = wide_schedule(0);
        let before = bar(&harness, "Jetzt");
        let at = match place {
            "below the rows" => before.center() + egui::vec2(0.0, 300.0),
            _ => egui::pos2(
                harness
                    .get_all_by_label("Jetzt")
                    .map(|n| n.rect())
                    .find(|r| r.width() > 100.0)
                    .unwrap()
                    .center()
                    .x,
                before.center().y,
            ),
        };
        harness.hover_at(at);
        harness.run();
        harness.event(egui::Event::Zoom(1.5));
        harness.run();
        let after = bar(&harness, "Jetzt");
        assert!(
            after.width() > before.width() * 1.3,
            "{place}: {before:?} → {after:?}"
        );
    }
}

/// A dialog over the schedule takes the pinch: the schedule behind it does not zoom.
#[test]
fn a_pinch_over_a_dialog_does_not_zoom_the_schedule() {
    let mut harness = wide_schedule(0);
    let before = bar(&harness, "Jetzt");
    let task = task_named(harness.state(), "Jetzt").unwrap().id;
    harness
        .state_mut()
        .state
        .open_dialog(crate::state::Dialog::DeleteTask(task));
    harness.run();
    // Below the rows, where the schedule would zoom without the dialog.
    harness.hover_at(before.center() + egui::vec2(0.0, 300.0));
    harness.run();
    harness.event(egui::Event::Zoom(1.5));
    harness.run();
    harness.state_mut().state.close_dialog();
    harness.run();
    let after = bar(&harness, "Jetzt");
    assert!(
        (after.width() - before.width()).abs() < 0.5,
        "{before:?} → {after:?}"
    );
}

// ---- Ribbon (Phase 9b) ----

/// Whether the body of the ribbon is shown (its button "Befehl suchen" in the tab Start).
fn ribbon_open(harness: &Harness<'_, PlnwrkApp>) -> bool {
    harness
        .query_by_label(text::t().ribbon.find_action)
        .is_some()
}

#[test]
fn the_ribbon_collapses_and_opens_for_a_moment() {
    let mut harness = harness_with_sample();
    assert!(
        ribbon_open(&harness),
        "open by default on a large desktop window"
    );
    // A double click on a tab collapses it.
    double_click(&mut harness, text::t().ribbon.start);
    assert!(!ribbon_open(&harness), "collapsed by a double click");
    assert_eq!(harness.state().state.ribbon_collapsed, Some(true));
    // A click on a tab opens it for a moment; a click elsewhere closes it again.
    harness.run_steps(30);
    click(&mut harness, text::t().ribbon.start);
    assert!(ribbon_open(&harness), "peek");
    let sidebar = harness
        .get_all_by_label("Digitalfunk Umrüstung Wachen")
        .next()
        .unwrap()
        .rect()
        .center();
    tap(&mut harness, sidebar);
    assert!(!ribbon_open(&harness), "a click elsewhere closes the peek");
    assert!(
        matches!(harness.state().state.view, View::Project(_)),
        "and still does what it was for"
    );
    // Ctrl/Cmd+F1 expands it for good.
    harness.key_press_modifiers(Modifiers::COMMAND, Key::F1);
    harness.run();
    assert!(ribbon_open(&harness));
    assert_eq!(harness.state().state.ribbon_collapsed, Some(false));
    // In the Mac app also Cmd+Option+R, as in Office (elsewhere it would be AltGr+R).
    harness.key_press_modifiers(Modifiers::COMMAND | Modifiers::ALT, Key::R);
    harness.run();
    assert_eq!(!ribbon_open(&harness), cfg!(target_os = "macos"));
}

#[test]
fn contextual_tabs_come_with_their_context() {
    let mut harness = harness_with_sample();
    let task_tab = text::t().ribbon.task;
    assert!(harness.query_by_label(task_tab).is_none());
    harness
        .get_all_by_label("Digitalfunk Umrüstung Wachen")
        .next()
        .unwrap()
        .click();
    harness.run();
    click(&mut harness, "Einbau Wache 11");
    assert!(
        harness.query_by_label(task_tab).is_some(),
        "a task is chosen"
    );
    click(&mut harness, task_tab);
    let id = task_named(harness.state(), "Einbau Wache 11").unwrap().id;
    // The tab offers the status; "In Arbeit" sets it.
    click(
        &mut harness,
        &format!(
            "{}: {}",
            text::t().status,
            text::task_status(TaskStatus::InProgress)
        ),
    );
    assert_eq!(
        harness
            .state()
            .doc()
            .unwrap()
            .session
            .workspace()
            .task(id)
            .unwrap()
            .status,
        TaskStatus::InProgress
    );
    // Without a task chosen the tab goes, and the ribbon falls back to Start.
    harness.state_mut().state.select(None);
    harness.run();
    assert!(harness.query_by_label(task_tab).is_none());
    assert_eq!(
        harness.state().state.ribbon_tab,
        crate::actions::RibbonTab::Start
    );
}

#[test]
fn the_palette_names_where_an_action_lives() {
    let harness = harness_with_sample();
    let app = harness.state();
    let entries = crate::palette::entries(app.doc(), &app.state);
    let new_case = entries
        .iter()
        .find(|e| e.label == text::t().ribbon.new_case_long)
        .unwrap();
    assert_eq!(
        new_case.detail,
        format!("{} › {}", text::t().ribbon.start, text::t().ribbon.new)
    );
}

#[test]
fn every_action_can_be_found_in_its_tab() {
    let mut harness = harness_with_sample();
    harness
        .get_all_by_label("Digitalfunk Umrüstung Wachen")
        .next()
        .unwrap()
        .click();
    harness.run();
    click(&mut harness, "Einbau Wache 11");
    let tabs = {
        let app = harness.state();
        crate::actions::tabs(app.doc(), &app.state)
    };
    for tab in tabs {
        harness.state_mut().state.ribbon_tab = tab;
        harness.run();
        let app = harness.state();
        let names: Vec<String> = crate::actions::all(app.doc(), &app.state)
            .iter()
            .filter(|a| a.group.tab() == tab)
            .map(|a| a.full_name().to_owned())
            .collect();
        for name in names {
            assert!(
                harness.query_all_by_label(&name).next().is_some(),
                "{tab:?}: {name}"
            );
        }
    }
}

/// The engine's table of places (which the AI reads over MCP) names the same tab and group as
/// the ribbon, in both languages, and lists every command that has a button there.
#[test]
fn the_engine_knows_where_each_command_lives() {
    let mut harness = harness_with_sample();
    harness
        .get_all_by_label("Digitalfunk Umrüstung Wachen")
        .next()
        .unwrap()
        .click();
    harness.run();
    click(&mut harness, "Einbau Wache 11");
    let mut seen = std::collections::BTreeSet::new();
    for language in text::Language::ALL {
        text::set_language(language);
        let app = harness.state();
        for action in crate::actions::all(app.doc(), &app.state) {
            for command in action.commands {
                let place = plnwrk_engine::places::place(command)
                    .unwrap_or_else(|| panic!("{command} has no place in the engine"));
                let expected = match language {
                    text::Language::German => place.de,
                    text::Language::English => place.en,
                };
                assert_eq!(action.group.place(), expected, "{command} ({language:?})");
                seen.insert(*command);
            }
        }
    }
    text::set_language(text::Language::German);
    for (command, _) in plnwrk_engine::places::PLACES {
        assert!(
            seen.contains(command),
            "{command} has no button in the ribbon"
        );
    }
}

#[test]
fn the_ribbon_starts_open_except_in_very_low_windows_and_never_jumps() {
    let (browser, _) = in_browser();
    assert!(
        ribbon_open(&browser),
        "open in the browser too: newcomers see the commands"
    );
    let mut low = Harness::builder()
        .with_size(egui::vec2(1400.0, 600.0))
        .with_step_dt(1.0 / 60.0)
        .build_ui_state(|ui, app: &mut PlnwrkApp| app.show(ui), PlnwrkApp::default());
    low.run();
    assert!(!ribbon_open(&low), "very low window");
    // Decided once: a larger window later does not make it jump open.
    low.set_size(egui::vec2(1400.0, 1000.0));
    low.run();
    assert!(!ribbon_open(&low), "stays as it started");
    // The person's choice wins and is remembered.
    low.state_mut().state.ribbon_collapsed = Some(false);
    low.run();
    assert!(ribbon_open(&low));
    let mut storage = MemoryStorage::default();
    eframe::App::save(low.state_mut(), &mut storage);
    let mut app = PlnwrkApp::default();
    app.load_settings(&storage);
    assert_eq!(app.state.ribbon_collapsed, Some(false));
}

/// A collapsed ribbon with the sample open.
fn collapsed_ribbon() -> Harness<'static, PlnwrkApp> {
    let mut harness = harness_with_sample();
    harness.state_mut().state.ribbon_collapsed = Some(true);
    harness.run();
    assert!(!ribbon_open(&harness));
    harness
}

#[test]
fn a_peek_lies_over_the_content_and_closes_with_an_action_or_escape() {
    let mut harness = collapsed_ribbon();
    let heading = harness.get_by_label(text::t().today_and_overdue).rect();
    click(&mut harness, text::t().ribbon.start);
    assert!(ribbon_open(&harness), "peek");
    assert_eq!(
        harness.get_by_label(text::t().today_and_overdue).rect(),
        heading,
        "the content does not move"
    );
    // An action runs and closes the peek.
    click(&mut harness, text::t().ribbon.new_project_long);
    assert!(matches!(
        harness.state().state.dialog,
        Some(crate::state::Dialog::NewProject(_))
    ));
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!ribbon_open(&harness), "closed by the action");
    // Escape closes it too.
    click(&mut harness, text::t().ribbon.start);
    assert!(ribbon_open(&harness));
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!ribbon_open(&harness), "closed by Escape");
    // The focus goes back to the tab, not nowhere.
    let focused = harness.query_by(|n| n.is_focused()).map(|n| n.rect());
    assert_eq!(
        focused,
        Some(harness.get_by_label(text::t().ribbon.start).rect())
    );
}

#[test]
fn the_tools_of_the_schedule_choose_the_zoom_level() {
    let mut harness = wide_schedule(0);
    let before = bar(&harness, "Jetzt").width();
    click(&mut harness, text::t().ribbon.schedule_tools);
    assert_eq!(
        harness.state().state.ribbon_tab,
        crate::actions::RibbonTab::Schedule
    );
    // The button of the ribbon (the schedule's own segment is called only "Tag").
    click(
        &mut harness,
        &format!("{}: {}", text::t().ribbon.zoom, text::t().zoom_levels[3]),
    );
    let after = bar(&harness, "Jetzt").width();
    // From week to day: 22 to 48 px a day.
    let ratio = after / before;
    assert!((ratio - 48.0 / 22.0).abs() < 0.08, "{before} → {after}");
    // The ribbon shows the level chosen.
    let app = harness.state();
    let chosen: Vec<String> = crate::actions::all(app.doc(), &app.state)
        .into_iter()
        .filter(|a| a.group == crate::actions::Group::Zoom && a.chosen)
        .map(|a| a.label)
        .collect();
    assert_eq!(chosen, [text::t().zoom_levels[3]]);
}

#[test]
fn the_palette_leaves_out_what_is_already_chosen() {
    let harness = harness_with_sample();
    let app = harness.state();
    let labels: Vec<String> = crate::palette::entries(app.doc(), &app.state)
        .into_iter()
        .map(|e| e.label)
        .collect();
    let current = format!("{}: {}", text::t().appearance, text::t().theme_system);
    let other = format!("{}: {}", text::t().appearance, text::t().theme_dark);
    assert!(!labels.contains(&current), "{labels:?}");
    assert!(labels.contains(&other));
}

#[test]
fn the_keyboard_skips_greyed_ribbon_buttons() {
    use egui_kittest::kittest::NodeT as _;
    let mut harness = harness_with_sample();
    // Without a project open, "Neue Aufgabe" is greyed; "Neues Projekt" is not.
    let mut visited = Vec::new();
    for _ in 0..40 {
        harness.key_press(Key::Tab);
        harness.run();
        if let Some(label) = harness
            .query_by(|n| n.is_focused())
            .and_then(|n| n.accesskit_node().label())
        {
            visited.push(label);
        }
    }
    assert!(
        visited
            .iter()
            .any(|l| l == text::t().ribbon.new_project_long),
        "{visited:?}"
    );
    assert!(
        !visited.iter().any(|l| l == text::t().new_task),
        "{visited:?}"
    );
}

#[test]
fn the_palette_names_the_task_an_action_acts_on() {
    let mut harness = harness_with_sample();
    harness
        .get_all_by_label("Digitalfunk Umrüstung Wachen")
        .next()
        .unwrap()
        .click();
    harness.run();
    click(&mut harness, "Einbau Wache 11");
    let app = harness.state();
    let entries = crate::palette::entries(app.doc(), &app.state);
    let delete = entries
        .iter()
        .find(|e| e.label == text::t().delete_task)
        .unwrap();
    assert!(
        delete.detail.ends_with("Einbau Wache 11"),
        "{}",
        delete.detail
    );
}

#[test]
fn a_dialog_or_the_keyboard_leaving_ends_a_peek() {
    // A dialog opened by its shortcut, without a field to take the focus (Cmd+Q with unsaved
    // work asks first).
    let mut harness = collapsed_ribbon();
    click(&mut harness, text::t().ribbon.start);
    assert!(ribbon_open(&harness));
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Q);
    harness.run();
    assert!(matches!(
        harness.state().state.dialog,
        Some(crate::state::Dialog::Unsaved(_))
    ));
    assert!(
        !harness.state().state.ribbon_peek,
        "the dialog ended the peek"
    );
    click(&mut harness, text::t().cancel);
    assert!(!ribbon_open(&harness));
    // Tab out of the peek: it closes rather than hide the focus.
    click(&mut harness, text::t().ribbon.start);
    assert!(ribbon_open(&harness));
    for _ in 0..30 {
        harness.key_press(Key::Tab);
        harness.run();
        if !ribbon_open(&harness) {
            break;
        }
    }
    assert!(!ribbon_open(&harness), "focus left the peek");
}

/// The visible part of the chart: right of the title column, inside the window.
fn chart_view(harness: &Harness<'_, PlnwrkApp>) -> egui::Rect {
    let titles = harness
        .get_all_by_label("Jetzt")
        .map(|n| n.rect())
        .find(|r| r.width() > 100.0)
        .unwrap();
    // The detail panel, when open, covers the right of the window.
    let right = harness
        .query_by_label(text::t().delete_task)
        .map_or(1400.0, |n| n.rect().left() - 8.0);
    // The row of the table (titles and columns) ends where the chart begins.
    egui::Rect::from_min_max(
        egui::pos2(titles.right(), titles.top() - 200.0),
        egui::pos2(right, titles.bottom() + 800.0),
    )
}

/// Whether the bar of a task is fully in view (bars out of view are not drawn at all).
fn in_view(harness: &Harness<'_, PlnwrkApp>, title: &str) -> bool {
    harness
        .query_by(|n| {
            n.label()
                .is_some_and(|l| l.starts_with(&format!("{title}: ")))
        })
        .is_some_and(|n| chart_view(harness).contains_rect(n.rect()))
}

#[test]
fn today_and_the_whole_project_are_one_click_away() {
    let mut harness = wide_schedule(0);
    assert!(in_view(&harness, "Jetzt"), "opens at today");
    assert!(!in_view(&harness, "Später"));
    // Somewhere else, then back to today.
    harness.state_mut().state.schedule_request = None;
    let at = bar(&harness, "Jetzt").right_center() + egui::vec2(30.0, 0.0);
    for _ in 0..6 {
        wheel(&mut harness, at, Modifiers::SHIFT);
    }
    assert!(!in_view(&harness, "Jetzt"), "scrolled away");
    click(&mut harness, text::t().today);
    assert!(in_view(&harness, "Jetzt"), "back at today");
    // The whole project: first and last task at once.
    click(&mut harness, text::t().whole_project);
    assert!(in_view(&harness, "Früher") && in_view(&harness, "Später"));
    assert_eq!(
        harness
            .state()
            .state
            .zoom_levels
            .values()
            .copied()
            .collect::<Vec<_>>(),
        [0],
        "the finest level that fits: four months need 984 px at month, 940 are there"
    );
}

#[test]
fn dragging_the_empty_chart_moves_the_view() {
    let mut harness = wide_schedule(0);
    let before = bar(&harness, "Jetzt").left();
    // In the row of "Jetzt", right of its bar where the chart is empty: drag to the left.
    let from = bar(&harness, "Jetzt").center() + egui::vec2(200.0, 0.0);
    // The pointer comes there first, then presses (as a hand on the mouse does).
    harness.hover_at(from);
    harness.run();
    harness.drag_at(from);
    harness.run();
    for step in 1..=10 {
        harness.hover_at(from - egui::vec2(20.0 * step as f32, 0.0));
        harness.run();
    }
    harness.drop_at(from - egui::vec2(200.0, 0.0));
    // The view glides on a little after letting go.
    harness.run_steps(120);
    let after = bar(&harness, "Jetzt").left();
    assert!(after < before - 150.0, "{before} → {after}");
}

#[test]
fn choosing_a_task_elsewhere_brings_its_bar_into_view() {
    let mut harness = wide_schedule(0);
    assert!(!in_view(&harness, "Später"));
    let later = task_named(harness.state(), "Später").unwrap().id;
    // As the detail panel, the palette or the list would choose it.
    harness
        .state_mut()
        .apply(crate::Intent::Select(Some(later)));
    harness.run_steps(30);
    assert!(in_view(&harness, "Später"));
}

#[test]
fn the_whole_project_fits_even_beside_a_very_long_title() {
    let mut harness = with_project(&"Sehr langer Projekttitel ".repeat(7));
    let project = projects(harness.state()).first().unwrap().id.to_string();
    let today = harness.state().doc().unwrap().session.clock().today();
    let day = |days: i64| (today + jiff::Span::new().days(days)).to_string();
    for (title, start, due) in [
        ("Früher", day(-60), day(-58)),
        ("Jetzt", day(0), day(2)),
        ("Später", day(60), day(62)),
    ] {
        harness.state_mut().apply(crate::Intent::run(
            "task.create",
            serde_json::json!({"project": project, "title": title, "start": start, "due": due}),
        ));
    }
    harness.run();
    click(&mut harness, text::t().schedule_tab);
    click(&mut harness, text::t().whole_project);
    assert!(in_view(&harness, "Früher") && in_view(&harness, "Später"));
}

#[test]
fn the_schedule_fits_a_narrow_window() {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(800.0, 900.0))
        .with_step_dt(1.0 / 60.0)
        .build_ui_state(|ui, app: &mut PlnwrkApp| app.show(ui), PlnwrkApp::default());
    harness.run();
    click(&mut harness, text::t().open_sample);
    harness
        .get_all_by_label("Digitalfunk Umrüstung Wachen")
        .next()
        .unwrap()
        .click();
    harness.run();
    click(&mut harness, text::t().schedule_tab);
    for name in [
        text::t().today,
        text::t().whole_project,
        text::t().export_schedule,
    ] {
        let rect = harness.get_by_label(name).rect();
        assert!(
            rect.right() <= 800.0,
            "{name} reaches past the window: {rect:?}"
        );
    }
    // The table leaves the chart at least half of the room.
    let row = table_row(&harness, "Einbau Wache 11");
    let room = 800.0 - row.left();
    assert!(row.width() <= room / 2.0 + 1.0, "{row:?} of {room}");
}

/// The row of a task in the schedule's table (wider than a title alone).
fn table_row(harness: &Harness<'_, PlnwrkApp>, title: &str) -> egui::Rect {
    harness
        .get_all_by_label(title)
        .map(|n| n.rect())
        .find(|r| r.width() > 100.0)
        .unwrap()
}

#[test]
fn the_schedule_table_shows_and_speaks_dates_and_who() {
    let mut harness = harness_with_sample();
    sample_schedule_open(&mut harness);
    let row = harness
        .get_all_by_label("Ausschreibung vorbereiten")
        .find(|n| n.rect().width() > 100.0)
        .unwrap();
    let value = row.value().unwrap_or_default();
    assert!(value.contains(text::t().start), "{value}");
    assert!(value.contains("Anna Berg"), "{value}");
    assert!(
        value.contains(&crate::dates::format(
            task_named(harness.state(), "Ausschreibung vorbereiten")
                .unwrap()
                .due
                .unwrap()
        ))
    );
    // Rows are 32 high, bars 16.
    let bar = bar(&harness, "Einbau Wache 11");
    assert!((bar.height() - 16.0).abs() < 0.5, "{bar:?}");
}

/// Opens the sample's project "Digitalfunk Umrüstung Wachen" in the schedule tab, the sample
/// being open already.
fn sample_schedule_open(harness: &mut Harness<'_, PlnwrkApp>) {
    harness
        .get_all_by_label("Digitalfunk Umrüstung Wachen")
        .next()
        .unwrap()
        .click();
    harness.run();
    click(harness, text::t().schedule_tab);
}

#[test]
fn columns_are_switched_off_in_the_ribbon_and_the_title_column_is_dragged_wider() {
    let mut harness = wide_schedule(0);
    let full = table_row(&harness, "Jetzt").width();
    click(&mut harness, text::t().ribbon.schedule_tools);
    click(
        &mut harness,
        &format!("{}: {}", text::t().show, text::t().assignee),
    );
    let without = table_row(&harness, "Jetzt").width();
    assert!((full - without - 112.0).abs() < 1.0, "{full} → {without}");
    // Dragging the edge of the title column.
    let edge = harness.get_by_label(text::t().title_width).rect().center();
    harness.hover_at(edge);
    harness.run();
    harness.drag_at(edge);
    harness.run();
    for step in 1..=5 {
        harness.hover_at(edge + egui::vec2(16.0 * step as f32, 0.0));
        harness.run();
    }
    harness.drop_at(edge + egui::vec2(80.0, 0.0));
    harness.run();
    let wider = table_row(&harness, "Jetzt").width();
    assert!((wider - without - 80.0).abs() < 2.0, "{without} → {wider}");
}

#[test]
fn subtasks_fold_away_in_the_schedule() {
    let mut harness = harness_with_sample();
    sample_schedule_open(&mut harness);
    let in_table = |h: &Harness<'_, PlnwrkApp>, title: &str| {
        h.query_all_by_label(title)
            .any(|n| n.rect().width() > 100.0)
    };
    assert!(in_table(&harness, "Leistungsverzeichnis"));
    click(&mut harness, text::t().fold_subtasks);
    assert!(!in_table(&harness, "Leistungsverzeichnis"), "folded away");
    assert!(
        in_table(&harness, "Ausschreibung vorbereiten"),
        "the parent stays"
    );
    click(&mut harness, text::t().unfold_subtasks);
    assert!(in_table(&harness, "Leistungsverzeichnis"));
}

/// Drags from `from` by `by`, like a hand: there first, press, a few steps, let go.
fn drag(harness: &mut Harness<'_, PlnwrkApp>, from: egui::Pos2, by: egui::Vec2) {
    harness.hover_at(from);
    harness.run_steps(2);
    harness.drag_at(from);
    harness.run_steps(2);
    for step in 1..=4 {
        harness.hover_at(from + by * (step as f32 / 4.0));
        harness.run_steps(2);
    }
    harness.drop_at(from + by);
    harness.run_steps(2);
}

fn dates_of(harness: &Harness<'_, PlnwrkApp>, id: TaskId) -> (String, String) {
    let task = harness
        .state()
        .doc()
        .unwrap()
        .session
        .workspace()
        .task(id)
        .cloned()
        .unwrap();
    (
        task.start.unwrap().to_string(),
        task.due.unwrap().to_string(),
    )
}

#[test]
fn the_ends_of_a_bar_change_start_and_deadline() {
    let (mut harness, first, _) = two_dated_tasks();
    click(&mut harness, text::t().schedule_tab);
    // The deadline two days later (a week level: 22 px a day).
    let end = harness
        .get_by_label(&format!("{} · Ausschreibung", text::t().due))
        .rect()
        .center();
    drag(&mut harness, end, egui::vec2(44.0, 0.0));
    assert_eq!(
        dates_of(&harness, first),
        ("2026-10-12".into(), "2026-10-16".into())
    );
    // The start a day earlier.
    let start = harness
        .get_by_label(&format!("{} · Ausschreibung", text::t().start))
        .rect()
        .center();
    drag(&mut harness, start, egui::vec2(-22.0, 0.0));
    assert_eq!(
        dates_of(&harness, first),
        ("2026-10-11".into(), "2026-10-16".into())
    );
}

#[test]
fn the_preview_says_how_many_tasks_move_along() {
    let (mut harness, first, second) = two_dated_tasks();
    let link = serde_json::json!({"from": first.to_string(), "to": second.to_string()});
    harness
        .state_mut()
        .apply(crate::Intent::run("dependency.add", link));
    click(&mut harness, text::t().schedule_tab);
    let from = bar(&harness, "Ausschreibung").center();
    harness.hover_at(from);
    harness.run_steps(2);
    harness.drag_at(from);
    harness.run_steps(2);
    harness.hover_at(from + egui::vec2(66.0, 0.0));
    harness.run_steps(2);
    assert!(
        harness
            .query_by_label_contains(&text::moves_along(1))
            .is_some(),
        "named while dragged"
    );
    harness.drop_at(from + egui::vec2(66.0, 0.0));
    harness.run_steps(2);
    // What was announced happens: three days later, and Vergabe moved along.
    assert_eq!(
        dates_of(&harness, first),
        ("2026-10-15".into(), "2026-10-17".into())
    );
    assert_eq!(dates_of(&harness, second).0, "2026-10-18");
}

#[test]
fn bars_say_what_their_colours_show() {
    use egui_kittest::kittest::NodeT as _;
    let mut harness = wide_schedule(0);
    let named = |h: &Harness<'_, PlnwrkApp>, title: &str| {
        h.query_by(|n| {
            n.label()
                .is_some_and(|l| l.starts_with(&format!("{title}: ")))
        })
        .and_then(|n| n.accesskit_node().label())
        .unwrap_or_default()
    };
    let overdue = text::t().overdue_short;
    assert!(
        named(&harness, "Früher").contains(overdue) || {
            // Out of view at first: bring it in.
            click(&mut harness, text::t().whole_project);
            named(&harness, "Früher").contains(overdue)
        }
    );
    assert!(!named(&harness, "Jetzt").contains(overdue));
    // Progress.
    let now = task_named(harness.state(), "Jetzt").unwrap().id;
    harness.state_mut().apply(crate::Intent::run(
        "task.set_progress",
        serde_json::json!({"task": now.to_string(), "progress": 60}),
    ));
    harness.run();
    assert!(named(&harness, "Jetzt").contains("60 %"));
    // The critical path, switched off in the ribbon.
    let critical = text::t().critical_short;
    assert!(
        named(&harness, "Später").contains(critical),
        "{}",
        named(&harness, "Später")
    );
    click(&mut harness, text::t().ribbon.schedule_tools);
    click(
        &mut harness,
        &format!("{}: {}", text::t().show, text::t().critical_path),
    );
    assert!(!named(&harness, "Später").contains(critical));
}

fn waits_for(harness: &Harness<'_, PlnwrkApp>, from: TaskId, to: TaskId) -> bool {
    harness
        .state()
        .doc()
        .unwrap()
        .session
        .workspace()
        .dependencies()
        .any(|d| d.from == from && d.to == to)
}

#[test]
fn a_dependency_is_drawn_from_the_handle_onto_another_bar() {
    let (mut harness, first, second) = two_dated_tasks();
    click(&mut harness, text::t().schedule_tab);
    let handle = harness
        .get_by_label(&format!("{} · Ausschreibung", text::t().link_handle))
        .rect()
        .center();
    let target = bar(&harness, "Vergabe").center();
    drag(&mut harness, handle, target - handle);
    assert!(
        waits_for(&harness, first, second),
        "Vergabe waits for Ausschreibung"
    );
    // Back the other way would be a circle: nothing happens.
    let handle = harness
        .get_by_label(&format!("{} · Vergabe", text::t().link_handle))
        .rect()
        .center();
    let target = bar(&harness, "Ausschreibung").center();
    drag(&mut harness, handle, target - handle);
    assert!(!waits_for(&harness, second, first));
    assert!(
        harness
            .state()
            .state
            .message
            .as_ref()
            .is_none_or(|m| !m.error)
    );
}

#[test]
fn a_right_click_on_a_bar_adds_a_predecessor() {
    let (mut harness, first, second) = two_dated_tasks();
    click(&mut harness, text::t().schedule_tab);
    let at = bar(&harness, "Vergabe").center();
    harness.hover_at(at);
    harness.run_steps(2);
    harness
        .get_by(|n| n.label().is_some_and(|l| l.starts_with("Vergabe: ")))
        .click_secondary();
    harness.run_steps(4);
    // A submenu carries an arrow after its name.
    let submenu = harness
        .get_by(|n| {
            n.label()
                .is_some_and(|l| l.starts_with(text::t().add_predecessor_menu))
        })
        .rect()
        .center();
    tap(&mut harness, submenu);
    harness.run_steps(4);
    // The menu offers Ausschreibung (not Vergabe itself).
    let offered = harness
        .query_all_by_label("Ausschreibung")
        .map(|n| n.rect())
        .find(|r| r.width() < 400.0 && r.left() > at.x - 200.0);
    let offered = offered.expect("offered in the menu");
    tap(&mut harness, offered.center());
    harness.run_steps(4);
    assert!(waits_for(&harness, first, second));
}

#[test]
fn the_legend_explains_only_the_red_that_is_shown() {
    let mut harness = wide_schedule(0);
    // "Früher" is overdue; tasks without links: the last one is critical.
    assert!(harness.query_by_label(text::t().overdue_hint).is_some());
    assert!(harness.query_by_label(text::t().critical_hint).is_some());
    assert!(
        harness.query_by_label(text::t().due_line_hint).is_none(),
        "no project deadline"
    );
    harness.state_mut().apply(crate::Intent::ToggleCritical);
    harness.run();
    assert!(harness.query_by_label(text::t().critical_hint).is_none());
    assert_eq!(text::float(39), "Spielraum: 39 Tage");
}

#[test]
fn an_empty_schedule_says_what_to_do_and_leads_there() {
    let mut harness = with_project("Leer");
    type_into(&mut harness, text::t().new_task_hint, "Ohne Termin", true);
    click(&mut harness, text::t().schedule_tab);
    assert!(harness.query_by_label(text::t().schedule_empty).is_some());
    click(&mut harness, text::t().give_dates);
    let task = task_named(harness.state(), "Ohne Termin").unwrap().id;
    assert_eq!(
        harness.state().state.selected,
        Some(task),
        "its details are open"
    );
}

// ---- Review 9c: what must not happen ----

/// A project with one task holding only these dates, shown in the schedule at day level.
fn one_task_schedule(
    start: Option<i64>,
    due: Option<i64>,
) -> (Harness<'static, PlnwrkApp>, TaskId) {
    let mut harness = with_project("Eins");
    let project = projects(harness.state()).first().unwrap().id.to_string();
    let today = harness.state().doc().unwrap().session.clock().today();
    let day = |days: i64| (today + jiff::Span::new().days(days)).to_string();
    let mut params = serde_json::json!({"project": project, "title": "Eins"});
    if let Some(start) = start {
        params["start"] = serde_json::json!(day(start));
    }
    if let Some(due) = due {
        params["due"] = serde_json::json!(day(due));
    }
    harness
        .state_mut()
        .apply(crate::Intent::run("task.create", params));
    harness.run();
    click(&mut harness, text::t().schedule_tab);
    click(&mut harness, text::t().zoom_levels[3]);
    click(&mut harness, text::t().today);
    let id = task_named(harness.state(), "Eins").unwrap().id;
    (harness, id)
}

fn task_dates(
    harness: &Harness<'_, PlnwrkApp>,
    id: TaskId,
) -> (Option<jiff::civil::Date>, Option<jiff::civil::Date>) {
    let task = harness
        .state()
        .doc()
        .unwrap()
        .session
        .workspace()
        .task(id)
        .cloned()
        .unwrap();
    (task.start, task.due)
}

#[test]
fn edges_never_invent_a_deadline_and_never_cross() {
    let today = |h: &Harness<'_, PlnwrkApp>| h.state().doc().unwrap().session.clock().today();
    let day = |h: &Harness<'_, PlnwrkApp>, n: i64| today(h) + jiff::Span::new().days(n);
    // Only a start: its start moves, no deadline appears.
    let (mut harness, id) = one_task_schedule(Some(2), None);
    let at = harness
        .get_by_label(&format!("{} · Eins", text::t().start))
        .rect()
        .center();
    drag(&mut harness, at, egui::vec2(96.0, 0.0));
    assert_eq!(task_dates(&harness, id), (Some(day(&harness, 4)), None));
    // Only a deadline: it moves earlier.
    let (mut harness, id) = one_task_schedule(None, Some(6));
    let at = harness
        .get_by_label(&format!("{} · Eins", text::t().due))
        .rect()
        .center();
    drag(&mut harness, at, egui::vec2(-96.0, 0.0));
    assert_eq!(task_dates(&harness, id), (None, Some(day(&harness, 4))));
    // Start past the deadline stops at the deadline; deadline before the start stops there.
    let (mut harness, id) = one_task_schedule(Some(2), Some(4));
    let at = harness
        .get_by_label(&format!("{} · Eins", text::t().start))
        .rect()
        .center();
    drag(&mut harness, at, egui::vec2(48.0 * 5.0, 0.0));
    assert_eq!(
        task_dates(&harness, id),
        (Some(day(&harness, 4)), Some(day(&harness, 4)))
    );
    let (mut harness, id) = one_task_schedule(Some(2), Some(4));
    let at = harness
        .get_by_label(&format!("{} · Eins", text::t().due))
        .rect()
        .center();
    drag(&mut harness, at, egui::vec2(-48.0 * 5.0, 0.0));
    assert_eq!(
        task_dates(&harness, id),
        (Some(day(&harness, 2)), Some(day(&harness, 2)))
    );
}

#[test]
fn without_a_deadline_nothing_is_overdue() {
    use egui_kittest::kittest::NodeT as _;
    let (harness, _) = one_task_schedule(Some(-3), None);
    let name = harness
        .query_by(|n| n.label().is_some_and(|l| l.starts_with("Eins: ")))
        .and_then(|n| n.accesskit_node().label())
        .expect("the bar is there");
    assert!(!name.contains(text::t().overdue_short), "{name}");
    assert!(harness.query_by_label(text::t().overdue_hint).is_none());
}

#[test]
fn clicking_a_bar_in_view_does_not_move_the_chart() {
    let mut harness = wide_schedule(0);
    // The detail panel open already (it narrows the chart when it opens).
    let early = task_named(harness.state(), "Früher").unwrap().id;
    harness.state_mut().state.select(Some(early));
    harness.run_steps(30);
    click(&mut harness, text::t().today);
    let before = bar(&harness, "Jetzt");
    tap(&mut harness, before.center());
    harness.run_steps(30);
    assert_eq!(
        harness.state().state.selected,
        task_named(harness.state(), "Jetzt").map(|t| t.id)
    );
    let after = bar(&harness, "Jetzt");
    assert!(
        (after.center() - before.center()).length() < 1.0,
        "{before:?} → {after:?}"
    );
}

#[test]
fn even_a_wide_title_column_leaves_the_chart_half() {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(800.0, 900.0))
        .with_step_dt(1.0 / 60.0)
        .build_ui_state(|ui, app: &mut PlnwrkApp| app.show(ui), PlnwrkApp::default());
    harness.run();
    harness.state_mut().state.title_width = Some(480.0);
    click(&mut harness, text::t().open_sample);
    sample_schedule_open(&mut harness);
    let row = table_row(&harness, "Einbau Wache 11");
    let room = 800.0 - row.left();
    assert!(row.width() <= room / 2.0 + 1.0, "{row:?} of {room}");
}

#[test]
fn the_keyboard_does_not_stop_on_drag_handles() {
    use egui_kittest::kittest::NodeT as _;
    let (mut harness, _, _) = two_dated_tasks();
    click(&mut harness, text::t().schedule_tab);
    let handles = [text::t().start, text::t().due, text::t().link_handle];
    for _ in 0..40 {
        harness.key_press(Key::Tab);
        harness.run();
        if let Some(label) = harness
            .query_by(|n| n.is_focused())
            .and_then(|n| n.accesskit_node().label())
        {
            assert!(
                !handles
                    .iter()
                    .any(|h| label.starts_with(&format!("{h} · "))),
                "focus on {label}"
            );
        }
    }
}

#[test]
fn a_subtask_chosen_elsewhere_unfolds_its_parent() {
    let mut harness = harness_with_sample();
    sample_schedule_open(&mut harness);
    click(&mut harness, text::t().fold_subtasks);
    let sub = task_named(harness.state(), "Leistungsverzeichnis")
        .unwrap()
        .id;
    harness.state_mut().apply(crate::Intent::Select(Some(sub)));
    harness.run();
    assert!(
        harness
            .query_all_by_label("Leistungsverzeichnis")
            .any(|n| n.rect().width() > 100.0),
        "the parent unfolds"
    );
}

#[test]
fn the_circle_rule_holds_both_ways() {
    // A → B → C: C may not precede A, A may not follow C, and nothing twice or itself.
    let mut harness = with_project("Kette");
    let project = projects(harness.state()).first().unwrap().id.to_string();
    for title in ["A", "B", "C"] {
        harness.state_mut().apply(crate::Intent::run(
            "task.create",
            serde_json::json!({"project": project, "title": title}),
        ));
    }
    let id = |h: &Harness<'_, PlnwrkApp>, t: &str| task_named(h.state(), t).unwrap().id;
    let (a, b, c) = (id(&harness, "A"), id(&harness, "B"), id(&harness, "C"));
    for (from, to) in [(a, b), (b, c)] {
        harness.state_mut().apply(crate::Intent::run(
            "dependency.add",
            serde_json::json!({"from": from.to_string(), "to": to.to_string()}),
        ));
    }
    let ws = harness.state().doc().unwrap().session.workspace().clone();
    let a_may_wait_for = crate::views::detail::may_precede(&ws, a);
    assert!(!a_may_wait_for(c) && !a_may_wait_for(b) && !a_may_wait_for(a));
    let c_may_wait_for = crate::views::detail::may_precede(&ws, c);
    assert!(c_may_wait_for(a), "a shortcut is no circle");
    assert!(!c_may_wait_for(b), "not twice");
    let may_wait_for_c = crate::views::detail::may_follow(&ws, c);
    assert!(!may_wait_for_c(a) && !may_wait_for_c(b) && !may_wait_for_c(c));
    let may_wait_for_a = crate::views::detail::may_follow(&ws, a);
    assert!(
        may_wait_for_a(c),
        "C waits for A through B; directly too is a shortcut, no circle"
    );
    assert!(!may_wait_for_a(b), "not twice");
}

#[test]
fn an_edge_held_against_the_other_date_makes_up_nothing() {
    let today = |h: &Harness<'_, PlnwrkApp>| h.state().doc().unwrap().session.clock().today();
    let day = |h: &Harness<'_, PlnwrkApp>, n: i64| today(h) + jiff::Span::new().days(n);
    // Only a start: the deadline end pulled left onto the start stays without a deadline.
    let (mut harness, id) = one_task_schedule(Some(-3), None);
    let at = harness
        .get_by_label(&format!("{} · Eins", text::t().due))
        .rect()
        .center();
    drag(&mut harness, at, egui::vec2(-96.0, 0.0));
    assert_eq!(task_dates(&harness, id), (Some(day(&harness, -3)), None));
    // Only a deadline: the start end pushed right onto it stays without a start.
    let (mut harness, id) = one_task_schedule(None, Some(3));
    let at = harness
        .get_by_label(&format!("{} · Eins", text::t().start))
        .rect()
        .center();
    drag(&mut harness, at, egui::vec2(96.0, 0.0));
    assert_eq!(task_dates(&harness, id), (None, Some(day(&harness, 3))));
}

#[test]
fn clicking_a_long_bar_does_not_move_the_chart() {
    let (mut harness, id) = one_task_schedule(Some(-40), Some(80));
    let at = egui::pos2(1000.0, bar(&harness, "Eins").center().y);
    // Measured from the chart's left edge (the table can narrow when the details open), at the
    // bar's right end: its left end is cut at the table and would stay put anyway.
    let from_chart =
        |h: &Harness<'_, PlnwrkApp>| bar(h, "Eins").right() - table_row(h, "Eins").right();
    let before = from_chart(&harness);
    tap(&mut harness, at);
    harness.run_steps(30);
    assert_eq!(harness.state().state.selected, Some(id));
    let after = from_chart(&harness);
    assert!((after - before).abs() < 1.0, "{before} → {after}");
}

#[test]
fn the_schedule_calls_overdue_what_the_list_calls_overdue() {
    use egui_kittest::kittest::NodeT as _;
    // Due today at midnight: past already, as the list and the overview say.
    let mut harness = with_project("Heute fällig");
    let project = projects(harness.state()).first().unwrap().id.to_string();
    let today = harness.state().doc().unwrap().session.clock().today();
    harness.state_mut().apply(crate::Intent::run(
        "task.create",
        serde_json::json!({"project": project, "title": "Mitternacht", "start": today.to_string(),
            "due": format!("{today}T00:00")}),
    ));
    harness.run();
    click(&mut harness, text::t().schedule_tab);
    let name = harness
        .query_by(|n| n.label().is_some_and(|l| l.starts_with("Mitternacht: ")))
        .and_then(|n| n.accesskit_node().label())
        .expect("the bar is there");
    assert!(name.contains(text::t().overdue_short), "{name}");
}

/// "Overdue" is decided in one place (`plnwrk_engine::query::task_overdue`): no view compares
/// dates with today on its own (the schedule did, and disagreed with the list).
#[test]
fn views_ask_the_engine_what_is_overdue() {
    let own_rule = |line: &str| {
        let line = line.replace(' ', "");
        ["<today", "<=today", "today>", "today>="]
            .iter()
            .any(|p| line.contains(p))
    };
    assert!(
        own_rule("due < today") && own_rule("today >= due") && !own_rule("days_from(first, today)")
    );
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/views");
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        for (n, line) in text.lines().enumerate() {
            if own_rule(line) {
                found.push(format!("{}:{}: {}", path.display(), n + 1, line.trim()));
            }
        }
    }
    assert!(found.is_empty(), "{found:#?}");
}

// ---- Phase 9d: every change of a milestone is confirmed ----

fn asking(harness: &Harness<'_, PlnwrkApp>) -> bool {
    matches!(
        harness.state().state.dialog,
        Some(crate::state::Dialog::ConfirmMilestone { .. })
    )
}

fn milestone(harness: &Harness<'_, PlnwrkApp>) -> Task {
    task_named(harness.state(), "Inbetriebnahme").unwrap()
}

/// The sample's project with its milestone "Inbetriebnahme" (15.12.2026) chosen.
fn milestone_chosen() -> Harness<'static, PlnwrkApp> {
    let mut harness = harness_with_sample();
    harness
        .get_all_by_label("Digitalfunk Umrüstung Wachen")
        .next()
        .unwrap()
        .click();
    harness.run();
    let id = milestone(&harness).id;
    harness.state_mut().apply(crate::Intent::Select(Some(id)));
    harness.run();
    harness
}

#[test]
fn every_way_to_change_a_milestone_asks_first() {
    // The palette: status done.
    let mut harness = milestone_chosen();
    let before = milestone(&harness);
    palette(&mut harness, "status erledigt");
    assert!(asking(&harness), "palette");
    assert_eq!(milestone(&harness), before, "nothing changed yet");
    click(&mut harness, text::t().cancel);
    assert!(!asking(&harness));
    assert_eq!(milestone(&harness), before);

    // The detail panel: a typed date, taken when the field is left.
    type_into(&mut harness, text::t().due, "20.12.2026", true);
    assert!(asking(&harness), "detail panel");
    assert_eq!(milestone(&harness), before);
    click(&mut harness, text::t().change_button);
    assert_eq!(milestone(&harness).due.unwrap().to_string(), "2026-12-20");

    // The schedule: dragging its diamond a week later.
    click(&mut harness, text::t().schedule_tab);
    click(&mut harness, text::t().whole_project);
    let before = milestone(&harness);
    let diamond = bar(&harness, "Inbetriebnahme").center();
    drag(&mut harness, diamond, egui::vec2(60.0, 0.0));
    assert!(asking(&harness), "schedule");
    assert_eq!(milestone(&harness), before);
    click(&mut harness, text::t().cancel);
    assert_eq!(milestone(&harness), before);

    // The board: its card to "Erledigt" (the column's heading carries its count; the
    // details closed, so all columns are in view).
    harness.state_mut().state.select(None);
    click(&mut harness, text::t().board);
    // One task of the sample is done already.
    let column = format!("{} (1)", text::task_status(TaskStatus::Done));
    drag_card(&mut harness, "♦ Inbetriebnahme", &column);
    assert!(asking(&harness), "board");
    assert_eq!(milestone(&harness), before);
    click(&mut harness, text::t().cancel);

    // Deleting: the question about deleting is the confirmation – one question, not two.
    let id = milestone(&harness).id;
    harness.state_mut().apply(crate::Intent::Select(Some(id)));
    harness.run();
    click(&mut harness, text::t().delete_task);
    assert!(matches!(
        harness.state().state.dialog,
        Some(crate::state::Dialog::DeleteTask(_))
    ));
    click(&mut harness, text::t().delete);
    assert!(!asking(&harness));
    assert!(task_named(harness.state(), "Inbetriebnahme").is_none());
}

#[test]
fn the_question_names_old_new_and_what_follows() {
    let mut harness = milestone_chosen();
    type_into(&mut harness, text::t().due, "20.01.2027", true);
    assert!(asking(&harness));
    let line = shown_texts(&harness)
        .into_iter()
        .find(|t| t.contains(" → "))
        .expect("a line old → new");
    assert!(line.starts_with(text::t().milestone_date), "{line}");
    assert!(
        line.contains("15.12.2026") && line.contains("20.01.2027"),
        "{line}"
    );
    // After the project's deadline (15.12.2026).
    assert!(
        harness
            .query_by_label(text::t().after_project_due_then)
            .is_some()
    );
}

#[test]
fn cancelling_puts_the_typed_value_back_and_loses_nothing_else() {
    let mut harness = milestone_chosen();
    // Half typed elsewhere: a new milestone.
    harness.state_mut().state.forms.milestone_title = "Halb getippt".to_owned();
    type_into(&mut harness, text::t().due, "20.12.2026", true);
    assert!(asking(&harness));
    click(&mut harness, text::t().cancel);
    // The field shows the stored date again; nothing typed waits to be saved.
    let shown = harness
        .get_by_label(text::t().due)
        .value()
        .unwrap_or_default();
    assert_eq!(shown, "15.12.2026");
    assert_eq!(milestone(&harness).due.unwrap().to_string(), "2026-12-15");
    assert_eq!(harness.state().state.forms.milestone_title, "Halb getippt");
}

#[test]
fn undo_redo_and_new_milestones_ask_nothing() {
    let mut harness = milestone_chosen();
    type_into(&mut harness, text::t().due, "20.12.2026", true);
    click(&mut harness, text::t().change_button);
    harness
        .state_mut()
        .apply(crate::Intent::run("undo", serde_json::json!({})));
    harness.run();
    assert!(!asking(&harness), "undo");
    assert_eq!(milestone(&harness).due.unwrap().to_string(), "2026-12-15");
    harness
        .state_mut()
        .apply(crate::Intent::run("redo", serde_json::json!({})));
    harness.run();
    assert!(!asking(&harness), "redo");
    // A new milestone in the tab Meilensteine.
    click(&mut harness, text::t().milestones);
    harness.state_mut().state.forms.milestone_title = "Abnahme".to_owned();
    harness.state_mut().state.forms.milestone_date = "30.11.2026".to_owned();
    click(&mut harness, text::t().new_milestone);
    assert!(!asking(&harness), "creating");
    assert!(task_named(harness.state(), "Abnahme").is_some_and(|t| t.milestone));
}

/// The day `days` from the clock's today, as a command reads it.
fn day_from_today(harness: &Harness<'_, PlnwrkApp>, days: i64) -> String {
    let today = harness.state().doc().unwrap().session.clock().today();
    (today + jiff::Span::new().days(days)).to_string()
}

/// A project due in 30 days with a milestone in every light (Phase 9d), shown in the tab
/// Meilensteine:
/// - "A" (in 10 days) waits for "Vorarbeit", 2 days overdue: at risk;
/// - "B" (in 20 days) waits for "Bau", which ends the day before: 1 day of slack, at risk;
/// - "C" (in 24 days) waits for "Bau 2", ending 3 days before: on plan (just past the line);
/// - "D" 3 days ago, not reached: overdue;
/// - "E" reached;
/// - "F" (in 22 days) waits only for "Fertig", done, ending the day before: on plan – finished
///   work cannot be late (review 9d);
/// - "G" (in 26 days) waits for "Erledigt" (done, ending the day before) and "Offen" (open,
///   ending 10 days before): on plan – the slack comes from the open work (follow-up review).
fn milestones_in_every_light() -> Harness<'static, PlnwrkApp> {
    let mut harness = with_project("Lichter");
    let project = projects(harness.state()).first().unwrap().id.to_string();
    let due = day_from_today(&harness, 30);
    harness.state_mut().apply(crate::Intent::run(
        "project.update",
        serde_json::json!({"project": project, "due": due}),
    ));
    let tasks = [
        ("Vorarbeit", -2, false, "todo"),
        ("Bau", 19, false, "todo"),
        ("Bau 2", 21, false, "todo"),
        ("A", 10, true, "todo"),
        ("B", 20, true, "todo"),
        ("C", 24, true, "todo"),
        ("D", -3, true, "todo"),
        ("E", 5, true, "done"),
        ("Fertig", 21, false, "done"),
        ("F", 22, true, "todo"),
        ("Erledigt", 25, false, "done"),
        ("Offen", 16, false, "todo"),
        ("G", 26, true, "todo"),
    ];
    for (title, days, milestone, status) in tasks {
        let due = day_from_today(&harness, days);
        harness.state_mut().apply(crate::Intent::run(
            "task.create",
            serde_json::json!({"project": project, "title": title, "due": due,
                               "milestone": milestone, "status": status}),
        ));
    }
    for (from, to) in [
        ("Vorarbeit", "A"),
        ("Bau", "B"),
        ("Bau 2", "C"),
        ("Fertig", "F"),
        ("Erledigt", "G"),
        ("Offen", "G"),
    ] {
        let from = task_named(harness.state(), from).unwrap().id.to_string();
        let to = task_named(harness.state(), to).unwrap().id.to_string();
        harness.state_mut().apply(crate::Intent::run(
            "dependency.add",
            serde_json::json!({"from": from, "to": to}),
        ));
    }
    harness.run();
    click(&mut harness, text::t().milestones);
    harness
}

/// The diamond of a milestone on the timeline (its spoken name: "♦ title: date, light").
fn diamond<'a>(harness: &'a Harness<'_, PlnwrkApp>, title: &str) -> egui_kittest::Node<'a> {
    let start = format!("♦ {title}: ");
    harness.get_by(move |n| n.label().is_some_and(|l| l.starts_with(&start)))
}

#[test]
fn every_milestone_shows_its_light_derived_from_the_plan() {
    let harness = milestones_in_every_light();
    let t = text::t();
    for (title, light) in [
        ("A", t.light_at_risk),
        ("B", t.light_at_risk),
        ("C", t.light_on_plan),
        ("D", t.light_overdue),
        ("E", t.light_reached),
        ("F", t.light_on_plan),
        ("G", t.light_on_plan),
    ] {
        let node = diamond(&harness, title);
        let spoken = egui_kittest::kittest::NodeT::accesskit_node(&node)
            .label()
            .unwrap_or_default();
        assert!(spoken.ends_with(&format!(", {light}")), "{title}: {spoken}");
    }
    // Why: said in words on the card, never by colour alone.
    assert!(harness.query_by_label(t.predecessor_late).is_some());
    assert!(harness.query_by_label(&text::little_slack(1)).is_some());
    assert!(harness.query_by_label(&text::little_slack(3)).is_none());
}

#[test]
fn every_milestone_card_says_what_is_left_and_what_it_waits_for() {
    let harness = milestones_in_every_light();
    for left in ["in 10 Tagen", "seit 3 Tagen überfällig"] {
        assert!(harness.query_by_label(left).is_some(), "{left}");
    }
    let today = harness.state().doc().unwrap().session.clock().today();
    let reached = text::reached_on(Some(&crate::dates::format(today)));
    assert!(harness.query_by_label(&reached).is_some(), "{reached}");
    // A, B, C and G wait for one open task each; F only for finished work; D and E for
    // nothing, and say nothing about it.
    assert_eq!(
        harness
            .query_all_by_label(&text::open_predecessors(1))
            .count(),
        4
    );
    assert_eq!(
        harness
            .query_all_by_label(&text::open_predecessors(0))
            .count(),
        1
    );
    // Nobody in charge is said too.
    let nobody = format!("{}: {}", text::t().assignee, text::t().no_assignee);
    assert_eq!(harness.query_all_by_label(&nobody).count(), 7);
}

/// One milestone "Abnahme" in 10 days, in the tab Meilensteine.
fn one_milestone() -> Harness<'static, PlnwrkApp> {
    let mut harness = with_project("Feier");
    let project = projects(harness.state()).first().unwrap().id.to_string();
    let due = day_from_today(&harness, 10);
    harness.state_mut().apply(crate::Intent::run(
        "task.create",
        serde_json::json!({"project": project, "title": "Abnahme", "due": due,
                           "milestone": true}),
    ));
    harness.run();
    click(&mut harness, text::t().milestones);
    harness
}

#[test]
fn every_action_on_a_milestone_card_asks_first() {
    let mut harness = one_milestone();
    let abnahme = |h: &Harness<'_, PlnwrkApp>| task_named(h.state(), "Abnahme").unwrap();
    let before = abnahme(&harness);
    let t = text::t();

    // "Erreicht": asked; "Abbrechen" leaves it open.
    click(&mut harness, t.reach_button);
    assert!(asking(&harness), "reached");
    click(&mut harness, t.cancel);
    assert_eq!(abnahme(&harness), before);
    // Asked again and confirmed: reached today; the card says so, and offers to reopen it.
    click(&mut harness, t.reach_button);
    click(&mut harness, t.change_button);
    assert_eq!(abnahme(&harness).status, TaskStatus::Done);
    let today = harness.state().doc().unwrap().session.clock().today();
    let reached = text::reached_on(Some(&crate::dates::format(today)));
    assert!(harness.query_by_label(&reached).is_some());
    click(&mut harness, t.reopen_button);
    assert!(asking(&harness), "reopened");
    click(&mut harness, t.change_button);
    assert_eq!(abnahme(&harness).status, TaskStatus::Todo);

    // "Termin ändern": the typed date is asked about; "Abbrechen" closes the form and keeps
    // the stored date.
    let before = abnahme(&harness);
    click(&mut harness, t.change_date);
    let later = day_from_today(&harness, 14)
        .parse::<jiff::civil::Date>()
        .unwrap();
    if let Some((_, day, _)) = harness.state_mut().state.forms.milestone_edit.as_mut() {
        *day = crate::dates::format(later);
    }
    click(&mut harness, t.apply);
    assert!(asking(&harness), "date");
    click(&mut harness, t.cancel);
    assert_eq!(abnahme(&harness), before);
    assert!(harness.state().state.forms.milestone_edit.is_none());
    click(&mut harness, t.change_date);
    if let Some((_, day, _)) = harness.state_mut().state.forms.milestone_edit.as_mut() {
        *day = crate::dates::format(later);
    }
    click(&mut harness, t.apply);
    click(&mut harness, t.change_button);
    assert_eq!(abnahme(&harness).due, Some(later));

    // Dragged on the timeline: asked, nothing changed before.
    let before = abnahme(&harness);
    let at = diamond(&harness, "Abnahme").rect().center();
    drag(&mut harness, at, egui::vec2(-80.0, 0.0));
    assert!(asking(&harness), "timeline");
    assert_eq!(abnahme(&harness), before);
    click(&mut harness, t.change_button);
    assert!(abnahme(&harness).due < before.due, "moved earlier");

    // "Löschen": the question about deleting is the confirmation – one question.
    click(&mut harness, t.delete);
    assert!(matches!(
        harness.state().state.dialog,
        Some(crate::state::Dialog::DeleteTask(_))
    ));
    harness.get_all_by_label(t.delete).last().unwrap().click();
    harness.run();
    assert!(!asking(&harness));
    assert!(task_named(harness.state(), "Abnahme").is_none());
}

#[test]
fn a_click_on_a_diamond_chooses_the_milestone() {
    let mut harness = one_milestone();
    let id = task_named(harness.state(), "Abnahme").unwrap().id;
    diamond(&harness, "Abnahme").click();
    harness.run();
    assert_eq!(harness.state().state.selected, Some(id));
}

/// The milestones in their own row at the top of the schedule (plan §7.3): under the time axis,
/// above the first row, and still in view when the rows scroll; a click chooses one.
#[test]
fn the_milestones_stay_in_view_in_their_own_row() {
    const ROW_HEIGHT: f32 = 32.0;
    let mut harness = wide_schedule(40);
    let project = projects(harness.state()).first().unwrap().id.to_string();
    let due = day_from_today(&harness, 1);
    harness.state_mut().apply(crate::Intent::run(
        "task.create",
        serde_json::json!({"project": project, "title": "Ziel", "due": due, "milestone": true}),
    ));
    harness.run();
    let pinned = |h: &Harness<'_, PlnwrkApp>| {
        h.get_by(|n| n.label().is_some_and(|l| l.starts_with("♦ Ziel: ")))
            .rect()
    };
    // "Jetzt" is the second row (the first, "Früher", lies out of view in time).
    let first = bar(&harness, "Jetzt").top() - ROW_HEIGHT;
    let row = pinned(&harness);
    assert!(row.bottom() <= first, "{row:?} above the rows ({first})");
    // The rows scroll; the milestone row stays where it is, over them.
    let at = bar(&harness, "Jetzt").center() + egui::vec2(120.0, 0.0);
    for _ in 0..3 {
        wheel(&mut harness, at, Modifiers::NONE);
    }
    let moved = bar(&harness, "Jetzt").top();
    assert!(
        moved < first - 100.0,
        "the rows scrolled: {first} → {moved}"
    );
    // At most the time axis' height higher: once that scrolled away, the row takes its place
    // at the top of the view.
    let after = pinned(&harness);
    let up = row.top() - after.top();
    assert!(
        (0.0..=crate::views::schedule::HEADER + 1.0).contains(&up),
        "{row:?} → {after:?}"
    );
    // A click on it chooses the milestone.
    let id = task_named(harness.state(), "Ziel").unwrap().id;
    harness
        .get_by(|n| n.label().is_some_and(|l| l.starts_with("♦ Ziel: ")))
        .click();
    harness.run();
    assert_eq!(harness.state().state.selected, Some(id));
}

/// Review 9d (B1): a milestone date typed and the window closed – the question comes first,
/// the window stays open, and what was typed is still there; nothing is lost silently.
#[test]
fn closing_the_window_asks_about_a_typed_milestone_date_first() {
    let mut harness = milestone_chosen();
    type_into(&mut harness, text::t().due, "20.12.2026", false);
    let may_close = harness.state_mut().close_requested();
    assert!(!may_close);
    // At once, before the next frame: the question, not the one about saving.
    assert!(asking(&harness), "the question, not the one about saving");
    harness.run();
    assert!(asking(&harness));
    let shown = harness
        .get_by_label(text::t().due)
        .value()
        .unwrap_or_default();
    assert_eq!(shown, "20.12.2026");
    // Nothing refused: no "Nicht übernommen" (K1).
    let message = harness.state().state.message.clone();
    assert!(
        message
            .as_ref()
            .is_none_or(|m| !m.text.contains(text::t().not_taken)),
        "{message:?}"
    );
    click(&mut harness, text::t().change_button);
    assert_eq!(milestone(&harness).due.unwrap().to_string(), "2026-12-20");
}

/// Review 9d (K1): choosing another task with a typed milestone date asks; no error.
#[test]
fn choosing_elsewhere_with_a_typed_milestone_date_asks_without_an_error() {
    let mut harness = milestone_chosen();
    type_into(&mut harness, text::t().due, "20.12.2026", false);
    harness.state_mut().apply(crate::Intent::Select(None));
    harness.run();
    assert!(asking(&harness));
    let message = harness.state().state.message.clone();
    assert!(
        message
            .as_ref()
            .is_none_or(|m| !m.text.contains(text::t().not_taken)),
        "{message:?}"
    );
}

/// Review 9d (K2): Escape is "Abbrechen": the stored date comes back.
#[test]
fn escape_on_the_milestone_question_is_cancel() {
    let mut harness = milestone_chosen();
    type_into(&mut harness, text::t().due, "20.12.2026", true);
    assert!(asking(&harness));
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!asking(&harness));
    let shown = harness
        .get_by_label(text::t().due)
        .value()
        .unwrap_or_default();
    assert_eq!(shown, "15.12.2026");
    assert!(
        harness
            .state()
            .state
            .forms
            .task
            .as_ref()
            .is_none_or(|d| d.due == "15.12.2026")
    );
}

/// The wide schedule with a milestone "Ziel" in 10 days, its rows scrolled down: the pinned row
/// lies over some of them. Returns the pinned row's rect and a row hidden under it.
fn rows_under_the_milestone_row() -> (Harness<'static, PlnwrkApp>, egui::Rect, String, f32) {
    let mut harness = wide_schedule(40);
    let project = projects(harness.state()).first().unwrap().id.to_string();
    let due = day_from_today(&harness, 10);
    harness.state_mut().apply(crate::Intent::run(
        "task.create",
        serde_json::json!({"project": project, "title": "Ziel", "due": due, "milestone": true}),
    ));
    harness.run();
    let at = bar(&harness, "Jetzt").center() + egui::vec2(120.0, 0.0);
    for _ in 0..3 {
        wheel(&mut harness, at, Modifiers::NONE);
    }
    // The whole milestone row (32 px high) around its diamond.
    let diamond = harness
        .get_by(|n| n.label().is_some_and(|l| l.starts_with("♦ Ziel: ")))
        .rect();
    let band = egui::Rect::from_center_size(diamond.center(), egui::vec2(diamond.width(), 32.0));
    // A bar wholly under it (bars are 16 px high), and the height of its middle.
    let (hidden, y) = (0..40)
        .map(|n| format!("Zeile {n}"))
        .find_map(|title| {
            let start = format!("{title}: ");
            let y = harness
                .query_all_by(|n| n.label().is_some_and(|l| l.starts_with(&start)))
                .map(|n| n.rect().center().y)
                .find(|y| *y - 8.0 >= band.top() && *y + 8.0 <= band.bottom())?;
            Some((title, y))
        })
        .expect("a row under the milestone row");
    (harness, band, hidden, y)
}

/// Review 9d (F1): a bar hidden under the pinned row cannot be dragged through it.
#[test]
fn a_bar_under_the_milestone_row_cannot_be_dragged_through_it() {
    let (mut harness, band, hidden, y) = rows_under_the_milestone_row();
    let id = task_named(harness.state(), &hidden).unwrap().id;
    let before = dates_of(&harness, id);
    // On the pinned row, over the hidden bar, away from the diamond.
    let start = format!("{hidden}: ");
    let x = harness
        .get_by(|n| n.label().is_some_and(|l| l.starts_with(&start)))
        .rect()
        .center()
        .x;
    let ziel = band.center().x;
    assert!(
        (x - ziel).abs() > 30.0,
        "the hidden bar lies apart from the diamond"
    );
    drag(&mut harness, egui::pos2(x, y), egui::vec2(66.0, 0.0));
    assert_eq!(dates_of(&harness, id), before);
}

/// Review 9d (K3): a task chosen elsewhere whose row lies under the pinned row comes into view.
#[test]
fn a_chosen_task_under_the_milestone_row_comes_into_view() {
    let (mut harness, band, hidden, _) = rows_under_the_milestone_row();
    let id = task_named(harness.state(), &hidden).unwrap().id;
    harness.state_mut().apply(crate::Intent::Select(Some(id)));
    harness.run_steps(30);
    let start = format!("{hidden}: ");
    let bar = harness
        .get_by(|n| n.label().is_some_and(|l| l.starts_with(&start)))
        .rect();
    assert!(bar.top() >= band.bottom() - 1.0, "{bar:?} under {band:?}");
}

/// Review 9d (K5): the question says what moves along, and so does the answer.
#[test]
fn the_milestone_question_names_what_moves_along() {
    let mut harness = one_milestone();
    let project = projects(harness.state()).first().unwrap().id.to_string();
    let due = day_from_today(&harness, 11);
    harness.state_mut().apply(crate::Intent::run(
        "task.create",
        serde_json::json!({"project": project, "title": "Nach", "due": due}),
    ));
    harness.run();
    let nach = task_named(harness.state(), "Nach").unwrap().id;
    let abnahme = task_named(harness.state(), "Abnahme").unwrap().id;
    harness.state_mut().apply(crate::Intent::run(
        "dependency.add",
        serde_json::json!({"from": abnahme.to_string(), "to": nach.to_string()}),
    ));
    // The milestone two days later: "Nach" waits for it and moves along.
    let later = day_from_today(&harness, 12)
        .parse::<jiff::civil::Date>()
        .unwrap();
    harness.state_mut().apply(crate::Intent::Reschedule {
        task: abnahme,
        start: None,
        due: Some(later),
        due_time: None,
    });
    harness.run();
    assert!(asking(&harness));
    assert!(harness.query_by_label(&text::moves_along(1)).is_some());
    click(&mut harness, text::t().change_button);
    let message = harness.state().state.message.clone().map(|m| m.text);
    assert_eq!(message, Some(text::moved_along(1)));
}

/// Review 9d (K6): the date field on the card has its name and takes Enter.
#[test]
fn the_date_on_a_milestone_card_is_typed_and_taken_with_enter() {
    let mut harness = one_milestone();
    click(&mut harness, text::t().change_date);
    let later = day_from_today(&harness, 14)
        .parse::<jiff::civil::Date>()
        .unwrap();
    type_into(
        &mut harness,
        text::t().milestone_date,
        &crate::dates::format(later),
        true,
    );
    assert!(asking(&harness));
    click(&mut harness, text::t().change_button);
    assert_eq!(
        task_named(harness.state(), "Abnahme").unwrap().due,
        Some(later)
    );
}

/// Review 9d (B1): saving with a typed milestone date asks first and saves nothing meanwhile
/// (not the file without the date, as if it were taken).
#[test]
fn saving_waits_for_the_answer_about_a_typed_milestone_date() {
    let (mut harness, path) = saved_sample_with_task("ms-save", "♦ Inbetriebnahme");
    let stored = std::fs::read(&path).unwrap();
    // Another change waiting to be saved: saving now would write the file.
    let other = task_named(harness.state(), "Einbau Wache 11").unwrap().id;
    harness.state_mut().apply(crate::Intent::run(
        "task.update",
        serde_json::json!({"task": other.to_string(), "title": "Einbau Wache 12"}),
    ));
    harness.run();
    type_into(&mut harness, text::t().due, "20.12.2026", false);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run();
    assert!(asking(&harness));
    assert_eq!(std::fs::read(&path).unwrap(), stored, "nothing saved yet");
    click(&mut harness, text::t().change_button);
    assert_eq!(milestone(&harness).due.unwrap().to_string(), "2026-12-20");
    // Saving goes on after the answer (follow-up review 9d): no second ⌘S needed.
    harness.run();
    // Read from the file (it is still open, so not through `reopen`).
    let saved = plnwrk_store::open(&std::fs::read(&path).unwrap(), None).unwrap();
    let due = saved
        .document
        .workspace
        .tasks()
        .find(|t| t.title == "Inbetriebnahme")
        .and_then(|t| t.due);
    assert_eq!(due.map(|d| d.to_string()), Some("2026-12-20".to_owned()));
    assert!(!harness.state().has_unsaved_changes());
}

/// One milestone "Abnahme" in 10 days, waiting for "Vor" (due in 8 days).
fn milestone_waiting_for_vor() -> (Harness<'static, PlnwrkApp>, TaskId) {
    let mut harness = one_milestone();
    let project = projects(harness.state()).first().unwrap().id.to_string();
    let due = day_from_today(&harness, 8);
    harness.state_mut().apply(crate::Intent::run(
        "task.create",
        serde_json::json!({"project": project, "title": "Vor", "due": due}),
    ));
    harness.run();
    let vor = task_named(harness.state(), "Vor").unwrap().id;
    let abnahme = task_named(harness.state(), "Abnahme").unwrap().id;
    harness.state_mut().apply(crate::Intent::run(
        "dependency.add",
        serde_json::json!({"from": vor.to_string(), "to": abnahme.to_string()}),
    ));
    harness.run();
    (harness, vor)
}

/// Phase 9d (Tobi: a hint when moving): moving what a milestone waits for past it is done –
/// the milestone stays – and says so; up to the milestone's own day it says nothing.
#[test]
fn moving_past_a_milestone_says_so() {
    let (mut harness, vor) = milestone_waiting_for_vor();
    let overtaken = text::overtakes(&["Abnahme".to_owned()]);
    let message = |h: &Harness<'_, PlnwrkApp>| h.state().state.message.clone().map(|m| m.text);
    let to = |h: &Harness<'_, PlnwrkApp>, days| {
        day_from_today(h, days)
            .parse::<jiff::civil::Date>()
            .unwrap()
    };
    // To the milestone's day: allowed (it may fall on the day its predecessor ends).
    let same = to(&harness, 10);
    harness.state_mut().apply(crate::Intent::Reschedule {
        task: vor,
        start: None,
        due: Some(same),
        due_time: None,
    });
    harness.run();
    assert!(!asking(&harness), "the milestone itself does not change");
    assert!(
        message(&harness).is_none_or(|m| !m.contains(&overtaken)),
        "{:?}",
        message(&harness)
    );
    // One day further: done, and said.
    let past = to(&harness, 11);
    harness.state_mut().apply(crate::Intent::Reschedule {
        task: vor,
        start: None,
        due: Some(past),
        due_time: None,
    });
    harness.run();
    assert_eq!(task_named(harness.state(), "Vor").unwrap().due, Some(past));
    let said = message(&harness).unwrap_or_default();
    assert!(said.contains(&overtaken), "{said}");
    assert!(
        harness
            .state()
            .state
            .message
            .as_ref()
            .is_some_and(|m| m.error)
    );
}

/// The same while dragging in the schedule: the bar's spoken name (and the preview) say it.
#[test]
fn dragging_past_a_milestone_says_so_while_dragging() {
    let (mut harness, _) = milestone_waiting_for_vor();
    click(&mut harness, text::t().schedule_tab);
    let overtaken = text::overtakes(&["Abnahme".to_owned()]);
    let from = bar(&harness, "Vor").center();
    harness.hover_at(from);
    harness.run_steps(2);
    harness.drag_at(from);
    harness.run_steps(2);
    // Five days later at the level "Woche" (22 px a day).
    for step in 1..=5 {
        harness.hover_at(from + egui::vec2(22.0 * step as f32, 0.0));
        harness.run_steps(2);
    }
    let node = harness.get_by(|n| n.label().is_some_and(|l| l.starts_with("Vor: ")));
    let spoken = egui_kittest::kittest::NodeT::accesskit_node(&node)
        .label()
        .unwrap_or_default();
    assert!(spoken.contains(&overtaken), "{spoken}");
    harness.drop_at(from + egui::vec2(110.0, 0.0));
    harness.run_steps(2);
}

/// Typed input that is not stored yet must count as unsaved (and be taken on saving and
/// closing), or be named here with the reason it does not – a new input field cannot be
/// forgotten (follow-up review 9d: the fourth field found missing by hand).
const NOT_PENDING: [(&str, &str); 9] = [
    ("new_task", "nothing exists yet: Enter creates it"),
    ("board_new", "nothing exists yet: Enter creates it"),
    ("new_subtask", "nothing exists yet: Enter creates it"),
    ("detail_subtask", "nothing exists yet: Enter creates it"),
    (
        "milestone_title",
        "nothing exists yet: the button creates it",
    ),
    (
        "milestone_date",
        "nothing exists yet: the button creates it",
    ),
    (
        "milestone_time",
        "nothing exists yet: the button creates it",
    ),
    ("comment", "nothing exists yet: the button adds it"),
    (
        "person",
        "nothing exists yet: the button creates the person",
    ),
];

/// The fields of `Forms` in `state` that `lib` neither counts as unsaved nor lists above.
fn forms_not_counted(state: &str, lib: &str) -> Vec<String> {
    let body = state
        .split("pub struct Forms {")
        .nth(1)
        .and_then(|rest| rest.split("\n}").next())
        .unwrap_or_default();
    let counted = lib
        .split("pub fn has_unsaved_changes")
        .nth(1)
        .and_then(|rest| rest.split("fn flush_draft").next())
        .unwrap_or_default()
        // `self.state\n.forms\n.task` as rustfmt breaks it.
        .split_whitespace()
        .collect::<String>();
    body.lines()
        .filter_map(|line| line.trim().strip_prefix("pub "))
        .filter_map(|line| line.split(':').next())
        .map(str::to_owned)
        .filter(|field| !NOT_PENDING.iter().any(|(name, _)| name == field))
        .filter(|field| !counted.contains(&format!("forms.{field}")))
        .collect()
}

#[test]
fn every_typed_input_counts_as_unsaved_or_says_why_not() {
    let state = include_str!("state.rs");
    let lib = include_str!("lib.rs");
    assert!(
        forms_not_counted(state, lib).is_empty(),
        "{:?}",
        forms_not_counted(state, lib)
    );
    // An injected field is caught.
    let injected = state.replacen(
        "pub struct Forms {",
        "pub struct Forms {\n    pub neu: String,",
        1,
    );
    assert_eq!(forms_not_counted(&injected, lib), vec!["neu".to_owned()]);
}

/// Follow-up review 9d: after "Abbrechen" closing goes on by itself (the file is saved).
#[test]
fn closing_goes_on_after_the_answer() {
    let (mut harness, _) = saved_sample_with_task("ms-quit", "♦ Inbetriebnahme");
    type_into(&mut harness, text::t().due, "20.12.2026", false);
    assert!(!harness.state_mut().close_requested());
    assert!(asking(&harness));
    harness.run();
    click(&mut harness, text::t().cancel);
    assert!(
        harness.state().state.quitting,
        "closes without being asked again"
    );
}

/// Follow-up review 9d: a date typed on a milestone's card counts as unsaved and is taken
/// (asked about) on closing, like the detail panel's.
#[test]
fn a_date_typed_on_a_milestone_card_is_not_lost_on_closing() {
    let mut harness = one_milestone();
    click(&mut harness, text::t().change_date);
    let later = day_from_today(&harness, 14)
        .parse::<jiff::civil::Date>()
        .unwrap();
    type_into(
        &mut harness,
        text::t().milestone_date,
        &crate::dates::format(later),
        false,
    );
    assert!(harness.state().milestone_edit_pending());
    assert!(!harness.state_mut().close_requested());
    assert!(asking(&harness));
    harness.run();
    click(&mut harness, text::t().change_button);
    assert_eq!(
        task_named(harness.state(), "Abnahme").unwrap().due,
        Some(later)
    );
    // Closing went on: the file is new and unsaved, so it asks about saving.
    assert!(matches!(
        harness.state().state.dialog,
        Some(crate::state::Dialog::Unsaved(
            crate::state::FileAction::Quit
        ))
    ));
}

/// Follow-up review 9d: exporting with a typed milestone date waits for the answer, then
/// exports with the date.
#[test]
fn exporting_waits_for_the_answer_about_a_typed_milestone_date() {
    let dir = scratch("ms-export");
    let path = dir.join("zeitplan.csv");
    let host = Scripted {
        export: vec![path.clone()],
        ..Scripted::default()
    };
    let mut harness = harness(PlnwrkApp::with_host(Box::new(host)));
    sample_schedule(&mut harness);
    let id = milestone(&harness).id;
    harness.state_mut().apply(crate::Intent::Select(Some(id)));
    harness.run();
    type_into(&mut harness, text::t().due, "20.12.2026", false);
    click(&mut harness, text::t().export_schedule);
    assert!(asking(&harness));
    assert!(!path.exists(), "nothing exported yet");
    click(&mut harness, text::t().change_button);
    let csv = std::fs::read_to_string(&path).unwrap();
    assert!(csv.contains("20.12.2026"), "{csv}");
}

/// Follow-up review 9d: protecting with a typed milestone date waits; nothing is encrypted
/// meanwhile (the passphrase is not kept – it is entered again).
#[test]
fn protecting_waits_for_the_answer_about_a_typed_milestone_date() {
    let dir = scratch("ms-protect");
    let path = dir.join("geschuetzt.plnwrk");
    let host = Scripted {
        save: vec![path.clone()],
        ..Scripted::default()
    };
    let mut harness = harness(PlnwrkApp::with_host(Box::new(host)));
    click(&mut harness, text::t().open_sample);
    harness
        .get_all_by_label("Digitalfunk Umrüstung Wachen")
        .next()
        .unwrap()
        .click();
    harness.run();
    let id = milestone(&harness).id;
    harness.state_mut().apply(crate::Intent::Select(Some(id)));
    harness.run();
    type_into(&mut harness, text::t().due, "20.12.2026", false);
    harness
        .state_mut()
        .apply(crate::Intent::Protect(plnwrk_store::SecretString::from(
            "richtig-lange-passphrase".to_owned(),
        )));
    harness.run();
    assert!(asking(&harness));
    assert!(!harness.state().doc().unwrap().encrypted());
    assert!(!path.exists());
}

/// Follow-up review 9d: a link cannot be dropped onto a bar hidden under the milestone row.
#[test]
fn a_link_cannot_be_dropped_onto_a_bar_under_the_milestone_row() {
    let (mut harness, band, hidden, y) = rows_under_the_milestone_row();
    let links = |h: &Harness<'_, PlnwrkApp>| {
        h.state()
            .doc()
            .unwrap()
            .session
            .workspace()
            .dependencies()
            .count()
    };
    let before = links(&harness);
    // A visible row well below the pinned one, its link handle onto the hidden bar.
    let start = format!("{hidden}: ");
    let x = harness
        .get_by(|n| n.label().is_some_and(|l| l.starts_with(&start)))
        .rect()
        .center()
        .x;
    let handle = harness
        .query_all_by(|n| {
            n.label()
                .is_some_and(|l| l.starts_with(&format!("{} · Zeile", text::t().link_handle)))
        })
        .map(|n| n.rect())
        .find(|r| r.top() > band.bottom() + 60.0)
        .expect("a visible link handle");
    drag(
        &mut harness,
        handle.center(),
        egui::pos2(x, y) - handle.center(),
    );
    assert_eq!(links(&harness), before);
}

/// Follow-up review 9d: the milestones a hand-made `task.reschedule` overtakes are found in
/// every project it touches, not only the first.
#[test]
fn overtaken_milestones_are_found_in_every_project_moved() {
    let mut harness = harness(PlnwrkApp::default());
    click(&mut harness, text::t().new_file);
    let mut moves = Vec::new();
    for name in ["Eins", "Zwei"] {
        harness.state_mut().apply(crate::Intent::run(
            "project.create",
            serde_json::json!({"title": name}),
        ));
        harness.run();
        let project = projects(harness.state())
            .into_iter()
            .find(|p| p.title == name)
            .unwrap()
            .id
            .to_string();
        let (work, goal) = (format!("Arbeit {name}"), format!("Ziel {name}"));
        let due = day_from_today(&harness, 5);
        let goal_due = day_from_today(&harness, 10);
        for (title, due, milestone) in [(&work, &due, false), (&goal, &goal_due, true)] {
            harness.state_mut().apply(crate::Intent::run(
                "task.create",
                serde_json::json!({"project": project, "title": title, "due": due,
                                   "milestone": milestone}),
            ));
        }
        harness.run();
        let work = task_named(harness.state(), &work).unwrap().id;
        let goal = task_named(harness.state(), &goal).unwrap().id;
        harness.state_mut().apply(crate::Intent::run(
            "dependency.add",
            serde_json::json!({"from": work.to_string(), "to": goal.to_string()}),
        ));
        moves.push(serde_json::json!({"task": work.to_string(), "start": null,
                                      "due": day_from_today(&harness, 12)}));
    }
    harness.run();
    let ws = harness.state().doc().unwrap().session.workspace().clone();
    let found = plnwrk_schedule::overtaken_by(
        &ws,
        &serde_json::json!({ "moves": moves }),
        plnwrk_schedule::Calendar::standard(),
    );
    assert_eq!(found.len(), 2, "{found:?}");
}

/// Follow-up review 9d: a warning raised while saving takes what was typed (a milestone
/// overtaken) stays in view after "Gespeichert.".
#[test]
fn a_warning_from_saving_stays_after_saved() {
    let dir = scratch("ms-saved-warning");
    let path = dir.join("plan.plnwrk");
    let host = Scripted {
        save: vec![path.clone()],
        ..Scripted::default()
    };
    let mut harness = harness(PlnwrkApp::with_host(Box::new(host)));
    click(&mut harness, text::t().new_file);
    click(&mut harness, text::t().new_project);
    type_into(&mut harness, text::t().title, "Feier", false);
    click(&mut harness, text::t().create);
    let project = projects(harness.state()).first().unwrap().id.to_string();
    for (title, days, milestone) in [("Vor", 8, false), ("Abnahme", 10, true)] {
        let due = day_from_today(&harness, days);
        harness.state_mut().apply(crate::Intent::run(
            "task.create",
            serde_json::json!({"project": project, "title": title, "due": due,
                               "milestone": milestone}),
        ));
    }
    harness.run();
    let vor = task_named(harness.state(), "Vor").unwrap().id;
    let abnahme = task_named(harness.state(), "Abnahme").unwrap().id;
    harness.state_mut().apply(crate::Intent::run(
        "dependency.add",
        serde_json::json!({"from": vor.to_string(), "to": abnahme.to_string()}),
    ));
    harness.state_mut().apply(crate::Intent::Select(Some(vor)));
    harness.run();
    let past = day_from_today(&harness, 11)
        .parse::<jiff::civil::Date>()
        .unwrap();
    type_into(
        &mut harness,
        text::t().due,
        &crate::dates::format(past),
        false,
    );
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run();
    assert!(path.exists());
    let said = harness
        .state()
        .state
        .message
        .clone()
        .map(|m| m.text)
        .unwrap_or_default();
    assert!(said.contains(text::t().saved), "{said}");
    assert!(
        said.contains(&text::overtakes(&["Abnahme".to_owned()])),
        "{said}"
    );
}

/// Follow-up review 9d: a saved file with a date typed on a milestone card counts as unsaved.
#[test]
fn a_date_typed_on_a_milestone_card_counts_as_unsaved() {
    let (mut harness, _) = saved_sample_with_task("ms-card-unsaved", "♦ Inbetriebnahme");
    click(&mut harness, text::t().milestones);
    assert!(!harness.state().has_unsaved_changes());
    harness
        .get_all_by_label(text::t().change_date)
        .next()
        .unwrap()
        .click();
    harness.run();
    type_into(&mut harness, text::t().milestone_date, "20.12.2026", false);
    assert!(harness.state().has_unsaved_changes());
}

/// A board for Phase 9e: Anna Berg; cards due today, in 2, 3, 4 and 20 days, 3 days ago (Anna,
/// high priority), 5 days ago but done; one with a very long title; "Nachher" waiting for
/// "Vorher". Shown in the tab Board.
fn board_9e() -> Harness<'static, PlnwrkApp> {
    let mut harness = with_project("Wache");
    // Room for all five columns side by side.
    harness.set_size(egui::vec2(1800.0, 900.0));
    let project = projects(harness.state()).first().unwrap().id.to_string();
    harness.state_mut().apply(crate::Intent::run(
        "person.create",
        serde_json::json!({"name": "Anna Berg"}),
    ));
    harness.run();
    let anna = harness
        .state()
        .doc()
        .unwrap()
        .session
        .workspace()
        .people()
        .next()
        .unwrap()
        .id
        .to_string();
    let long = "Sehr lange Aufgabe mit einem Titel, der auf einer schmalen Karte nicht in zwei Zeilen passt, weil er so viele Wörter hat";
    let cards = [
        ("Heute", Some(0), serde_json::json!({})),
        ("Bald", Some(2), serde_json::json!({})),
        ("Grenze", Some(3), serde_json::json!({})),
        ("Dahinter", Some(4), serde_json::json!({})),
        ("Später", Some(20), serde_json::json!({})),
        (
            "Alt",
            Some(-3),
            serde_json::json!({"assignee": anna, "priority": "high"}),
        ),
        (
            "Erledigt alt",
            Some(-5),
            serde_json::json!({"status": "done"}),
        ),
        (long, None, serde_json::json!({})),
        ("Vorher", None, serde_json::json!({})),
        ("Nachher", None, serde_json::json!({})),
    ];
    for (title, days, extra) in cards {
        let mut params = serde_json::json!({"project": project, "title": title});
        if let Some(days) = days {
            params["due"] = serde_json::json!(day_from_today(&harness, days));
        }
        if let (Some(params), Some(extra)) = (params.as_object_mut(), extra.as_object()) {
            params.extend(extra.clone());
        }
        harness
            .state_mut()
            .apply(crate::Intent::run("task.create", params));
    }
    harness.run();
    let vorher = task_named(harness.state(), "Vorher").unwrap().id;
    let nachher = task_named(harness.state(), "Nachher").unwrap().id;
    harness.state_mut().apply(crate::Intent::run(
        "dependency.add",
        serde_json::json!({"from": vorher.to_string(), "to": nachher.to_string()}),
    ));
    harness.run();
    click(&mut harness, text::t().board);
    harness
}

/// The heading of a board column ("Offen (7)"): a plain label, so its text is the value; the
/// first of the nodes that carry it.
fn column_heading_node<'a>(
    harness: &'a Harness<'_, PlnwrkApp>,
    status: TaskStatus,
) -> egui_kittest::Node<'a> {
    let start = format!("{} (", text::task_status(status));
    harness
        .query_all_by(move |n| n.value().is_some_and(|l| l.starts_with(&start)))
        .next()
        .expect("a column heading")
}

#[test]
fn board_cards_say_how_their_deadline_stands() {
    let harness = board_9e();
    let now = harness.state().doc().unwrap().session.clock().now_local();
    let day = |days: i64| now.date() + jiff::Span::new().days(days);
    for (days, expected) in [
        (0, text::deadline_state(day(0), None, now).0),
        (2, text::deadline_state(day(2), None, now).0),
        // Soon up to 3 days ahead; the day after, the plain date.
        (3, text::deadline_state(day(3), None, now).0),
        (4, crate::dates::format_short(day(4))),
        (20, crate::dates::format_short(day(20))),
        (-3, text::deadline_state(day(-3), None, now).0),
        // Done: the date, plain – not "overdue".
        (-5, crate::dates::format(day(-5))),
    ] {
        assert!(
            harness.query_by_label(&expected).is_some(),
            "{days}: {expected}"
        );
    }
    let done_late = text::deadline_state(day(-5), None, now).0;
    assert!(harness.query_by_label(&done_late).is_none(), "{done_late}");
}

#[test]
fn a_board_card_names_its_person_and_priority() {
    let harness = board_9e();
    assert!(harness.query_by_label("Anna Berg").is_some());
    assert!(
        harness
            .query_by_label(text::priority(plnwrk_core::Priority::High))
            .is_some()
    );
}

#[test]
fn a_long_title_takes_two_lines_and_is_read_whole() {
    let harness = board_9e();
    let long = "Sehr lange Aufgabe mit einem Titel, der auf einer schmalen Karte nicht in zwei Zeilen passt, weil er so viele Wörter hat";
    let title = harness.get_by_label(long).rect();
    let line = harness.get_by_label("Heute").rect().height();
    assert!(
        title.height() <= 2.0 * line + 2.0,
        "{} for a line of {line}",
        title.height()
    );
    assert!(title.height() > 1.5 * line, "it wraps: {}", title.height());
}

/// The columns share the window's width; narrower than 220 px each, the board scrolls sideways.
#[test]
fn board_columns_fill_the_window_and_scroll_when_narrow() {
    let mut harness = board_9e();
    let heading = |h: &Harness<'_, PlnwrkApp>, status| column_heading_node(h, status).rect();
    let gap = |h: &Harness<'_, PlnwrkApp>| {
        heading(h, TaskStatus::InProgress).left() - heading(h, TaskStatus::Todo).left()
    };
    assert!(gap(&harness) > 228.0 + 20.0, "wide: {}", gap(&harness));
    harness.set_size(egui::vec2(900.0, 900.0));
    harness.run();
    assert!(
        (gap(&harness) - 228.0).abs() < 1.0,
        "narrow: {}",
        gap(&harness)
    );
    assert!(
        heading(&harness, TaskStatus::Done).left() > 900.0,
        "the last column lies beyond the window"
    );
}

#[test]
fn the_plus_of_a_column_adds_a_task_there() {
    let mut harness = board_9e();
    click(&mut harness, &text::new_in_column(TaskStatus::Blocked));
    type_into(&mut harness, text::t().new_task_hint, "Funk prüfen", true);
    let created = task_named(harness.state(), "Funk prüfen").unwrap();
    assert_eq!(created.status, TaskStatus::Blocked);
    assert!(harness.state().state.forms.board_new.is_none());
    // Escape drops what was typed; nothing is created.
    click(&mut harness, &text::new_in_column(TaskStatus::Review));
    type_into(&mut harness, text::t().new_task_hint, "Doch nicht", false);
    harness.key_press(Key::Escape);
    harness.run();
    assert!(harness.state().state.forms.board_new.is_none());
    assert!(task_named(harness.state(), "Doch nicht").is_none());
}

#[test]
fn the_board_filter_keeps_what_was_asked_for() {
    let mut harness = board_9e();
    let todo = |h: &Harness<'_, PlnwrkApp>| {
        let node = column_heading_node(h, TaskStatus::Todo);
        egui_kittest::kittest::NodeT::accesskit_node(&node)
            .value()
            .unwrap_or_default()
    };
    // Nine open cards (one is done).
    assert_eq!(todo(&harness), text::column_heading(TaskStatus::Todo, 9, 9));
    click(&mut harness, text::t().board_filter_overdue);
    assert_eq!(todo(&harness), text::column_heading(TaskStatus::Todo, 1, 9));
    assert!(harness.query_by_label("Alt").is_some());
    assert!(harness.query_by_label("Heute").is_none());
    click(&mut harness, text::t().board_filter_reset);
    assert_eq!(todo(&harness), text::column_heading(TaskStatus::Todo, 9, 9));
    // Words in the title, any case.
    type_into(&mut harness, text::t().board_search, "VOR", false);
    assert_eq!(todo(&harness), text::column_heading(TaskStatus::Todo, 1, 9));
    assert!(harness.query_by_label("Vorher").is_some());
    click(&mut harness, text::t().board_filter_reset);
    // A person; nobody in charge.
    let anna = harness
        .state()
        .doc()
        .unwrap()
        .session
        .workspace()
        .people()
        .next()
        .unwrap()
        .id;
    let project = projects(harness.state()).first().unwrap().id;
    harness
        .state_mut()
        .state
        .board_filters
        .entry(project)
        .or_default()
        .assignee = Some(Some(anna));
    harness.run();
    assert_eq!(todo(&harness), text::column_heading(TaskStatus::Todo, 1, 9));
    harness
        .state_mut()
        .state
        .board_filters
        .entry(project)
        .or_default()
        .assignee = Some(None);
    harness.run();
    assert_eq!(todo(&harness), text::column_heading(TaskStatus::Todo, 8, 9));
}

#[test]
fn folding_a_card_hides_its_subtasks_and_back() {
    let mut harness = board_with_subtasks();
    let ticks = |h: &Harness<'_, PlnwrkApp>| {
        h.query_all_by(|n| {
            n.role() == egui::accesskit::Role::CheckBox && n.label().as_deref() == Some("Bücher")
        })
        .count()
    };
    assert_eq!(ticks(&harness), 1);
    click(&mut harness, text::t().fold_subtasks);
    assert_eq!(ticks(&harness), 0);
    // The bar still says how far it is.
    assert!(harness.query_by_label("0/2").is_some());
    // With the keyboard too.
    harness.get_by_label(text::t().unfold_subtasks).focus();
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    assert_eq!(ticks(&harness), 1);
}

#[test]
fn dragging_to_done_says_what_it_still_waits_for() {
    let mut harness = board_9e();
    harness.get_by_label("Nachher").scroll_to_me();
    harness.run();
    let card = harness.get_by_label("Nachher").rect().center();
    // Beside the card, in the column "Erledigt" (short: one card), far below its card.
    let done = egui::pos2(
        column_heading_node(&harness, TaskStatus::Done)
            .rect()
            .center()
            .x,
        card.y,
    );
    harness.drag_at(card);
    harness.run();
    harness.hover_at(card + egui::vec2(10.0, 10.0));
    harness.run();
    harness.hover_at(done);
    harness.run();
    assert!(
        harness.query_by_label(&text::still_waits_for(1)).is_some(),
        "said while dragging"
    );
    harness.drop_at(done);
    harness.run();
    // Said, not stopped.
    assert_eq!(
        task_named(harness.state(), "Nachher").unwrap().status,
        TaskStatus::Done
    );
    assert!(harness.query_by_label(&text::still_waits_for(1)).is_none());
}

/// Each project keeps its own board filter (Tobi 2026-10-17): filtering one leaves the other
/// as it was; back in the first, its filter is still there.
#[test]
fn each_project_keeps_its_own_board_filter() {
    let mut harness = board_9e();
    let first = projects(harness.state()).first().unwrap().id;
    click(&mut harness, text::t().board_filter_overdue);
    let filtered = text::column_heading(TaskStatus::Todo, 1, 9);
    assert!(harness.query_by_label(&filtered).is_some());
    // A second project with one open task.
    harness.state_mut().apply(crate::Intent::run(
        "project.create",
        serde_json::json!({"title": "Zweites"}),
    ));
    harness.run();
    let second = projects(harness.state())
        .into_iter()
        .find(|p| p.title == "Zweites")
        .unwrap()
        .id;
    harness.state_mut().apply(crate::Intent::run(
        "task.create",
        serde_json::json!({"project": second.to_string(), "title": "Eins"}),
    ));
    harness
        .state_mut()
        .apply(crate::Intent::Go(crate::state::View::Project(second)));
    harness.run();
    click(&mut harness, text::t().board);
    assert!(
        harness
            .query_by_label(&text::column_heading(TaskStatus::Todo, 1, 1))
            .is_some(),
        "unfiltered there"
    );
    harness
        .state_mut()
        .apply(crate::Intent::Go(crate::state::View::Project(first)));
    harness.run();
    click(&mut harness, text::t().board);
    assert!(harness.query_by_label(&filtered).is_some(), "kept here");
}

/// Types with the keyboard into whatever has the focus (no `focus()` first: the app must have
/// put it there), then Enter.
fn type_where_the_focus_is(harness: &mut Harness<'_, PlnwrkApp>, text: &str) {
    harness.event(egui::Event::Text(text.to_owned()));
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
}

/// Review 9e (F1): "+" puts the cursor into the new field every time – twice in one column,
/// and after another column's "+".
#[test]
fn the_plus_of_a_column_puts_the_cursor_there_every_time() {
    let mut harness = board_9e();
    for (status, title) in [
        (TaskStatus::Blocked, "Eins"),
        (TaskStatus::Blocked, "Zwei"),
        (TaskStatus::Review, "Drei"),
        (TaskStatus::Blocked, "Vier"),
        (TaskStatus::Review, "Fünf"),
    ] {
        click(&mut harness, &text::new_in_column(status));
        type_where_the_focus_is(&mut harness, title);
        let created = task_named(harness.state(), title);
        assert_eq!(created.map(|t| t.status), Some(status), "{title}");
    }
}

/// Review 9e (F2): a title half typed with "+" belongs to its project: not shown in another,
/// still there on coming back, and created in its own project.
#[test]
fn a_half_typed_board_card_stays_with_its_project() {
    let mut harness = board_9e();
    let first = projects(harness.state()).first().unwrap().id;
    click(&mut harness, &text::new_in_column(TaskStatus::Blocked));
    harness.event(egui::Event::Text("Funk".to_owned()));
    harness.run();
    harness.state_mut().apply(crate::Intent::run(
        "project.create",
        serde_json::json!({"title": "Zweites"}),
    ));
    harness.run();
    let second = projects(harness.state())
        .into_iter()
        .find(|p| p.title == "Zweites")
        .unwrap()
        .id;
    harness
        .state_mut()
        .apply(crate::Intent::Go(crate::state::View::Project(second)));
    harness.run();
    click(&mut harness, text::t().board);
    let typed = |h: &Harness<'_, PlnwrkApp>| {
        h.query_all_by(|n| n.value().as_deref() == Some("Funk"))
            .count()
    };
    assert_eq!(typed(&harness), 0, "not in the other project");
    harness
        .state_mut()
        .apply(crate::Intent::Go(crate::state::View::Project(first)));
    harness.run();
    click(&mut harness, text::t().board);
    assert!(typed(&harness) > 0, "kept in its own");
    harness
        .query_all_by(|n| n.value().as_deref() == Some("Funk"))
        .next()
        .unwrap()
        .focus();
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    assert_eq!(
        task_named(harness.state(), "Funk").map(|t| t.project),
        Some(first)
    );
}

/// Review 9e (F3): the person filtered for stays named in the choice when nothing is left to
/// them; a person gone from the file takes the filter along.
#[test]
fn the_person_filtered_for_stays_named_or_the_filter_goes() {
    let mut harness = board_9e();
    let project = projects(harness.state()).first().unwrap().id;
    let ws = harness.state().doc().unwrap().session.workspace().clone();
    let anna = ws.people().next().unwrap().id;
    harness
        .state_mut()
        .state
        .board_filters
        .entry(project)
        .or_default()
        .assignee = Some(Some(anna));
    harness.run();
    let alt = task_named(harness.state(), "Alt").unwrap().id;
    harness.state_mut().apply(crate::Intent::run(
        "task.update",
        serde_json::json!({"task": alt.to_string(), "assignee": null}),
    ));
    harness.run();
    // The choice shows her (the card no longer does).
    let named = harness
        .query_all_by(|n| {
            n.label().as_deref() == Some("Anna Berg") || n.value().as_deref() == Some("Anna Berg")
        })
        .count();
    assert!(named > 0, "still named");
    assert!(
        harness
            .query_by_label(&text::column_heading(TaskStatus::Todo, 0, 9))
            .is_some()
    );
    // Someone new, filtered for, then taken back (undo): the filter goes with them.
    harness.state_mut().apply(crate::Intent::run(
        "person.create",
        serde_json::json!({"name": "Bert Kaya"}),
    ));
    harness.run();
    let bert = harness
        .state()
        .doc()
        .unwrap()
        .session
        .workspace()
        .people()
        .find(|p| p.name == "Bert Kaya")
        .unwrap()
        .id;
    harness
        .state_mut()
        .state
        .board_filters
        .entry(project)
        .or_default()
        .assignee = Some(Some(bert));
    harness.run();
    harness
        .state_mut()
        .apply(crate::Intent::run("undo", serde_json::json!({})));
    harness.run();
    harness.run();
    assert!(
        harness
            .query_by_label(&text::column_heading(TaskStatus::Todo, 9, 9))
            .is_some()
    );
}

/// Review 9e (F4): the hint about open work shows at the pointer – also with the board
/// scrolled far down.
#[test]
fn the_hint_about_open_work_shows_at_the_pointer() {
    let mut harness = board_9e();
    let project = projects(harness.state()).first().unwrap().id.to_string();
    for n in 0..25 {
        harness.state_mut().apply(crate::Intent::run(
            "task.create",
            serde_json::json!({"project": project, "title": format!("Füller {n}")}),
        ));
    }
    harness.run();
    // "Nachher" waits for "Vorher"; moved to the very end of the long column.
    let nachher = task_named(harness.state(), "Nachher").unwrap().id;
    let last = task_named(harness.state(), "Füller 24").unwrap().id;
    harness.state_mut().apply(crate::Intent::run(
        "task.move",
        serde_json::json!({"task": nachher.to_string(), "after": last.to_string()}),
    ));
    harness.set_size(egui::vec2(1800.0, 600.0));
    harness.run();
    harness.get_by_label("Nachher").scroll_to_me();
    harness.run();
    let card = harness.get_by_label("Nachher").rect().center();
    let done = egui::pos2(
        column_heading_node(&harness, TaskStatus::Done)
            .rect()
            .center()
            .x,
        card.y,
    );
    harness.drag_at(card);
    harness.run();
    harness.hover_at(card + egui::vec2(10.0, 10.0));
    harness.run();
    harness.hover_at(done);
    harness.run();
    let hint = harness.get_by_label(&text::still_waits_for(1)).rect();
    assert!(
        hint.top() >= 0.0 && hint.bottom() <= 600.0,
        "in view: {hint:?}"
    );
    harness.drop_at(done);
    harness.run();
}

/// Review 9e (M6, M7): no hint where nothing waits – the predecessor done, or the card itself
/// done already.
#[test]
fn no_hint_when_nothing_open_is_waited_for() {
    let mut harness = board_9e();
    let held = |h: &mut Harness<'_, PlnwrkApp>, title: &str| -> bool {
        h.get_by_label(title).scroll_to_me();
        h.run();
        let card = h.get_by_label(title).rect().center();
        // In the column "Erledigt", below its cards (it reaches down to the bottom).
        let done = egui::pos2(
            column_heading_node(h, TaskStatus::Done).rect().center().x,
            (card.y + 90.0).min(820.0),
        );
        h.drag_at(card);
        h.run();
        h.hover_at(card + egui::vec2(10.0, 10.0));
        h.run();
        h.hover_at(done);
        h.run();
        let shown = h.query_by_label(&text::still_waits_for(1)).is_some();
        h.drop_at(card);
        h.run();
        shown
    };
    // The case itself: an open card waiting for open work is warned about (so the two cases
    // below are checked where the hint would show).
    assert!(held(&mut harness, "Nachher"), "open, waiting for open work");
    // The card done already, waiting for open work: nothing to warn about.
    let vorher = task_named(harness.state(), "Vorher").unwrap().id;
    let finished = task_named(harness.state(), "Erledigt alt").unwrap().id;
    harness.state_mut().apply(crate::Intent::run(
        "dependency.add",
        serde_json::json!({"from": vorher.to_string(), "to": finished.to_string()}),
    ));
    harness.run();
    assert!(!held(&mut harness, "Erledigt alt"), "done already");
    // The predecessor done: nothing open.
    harness.state_mut().apply(crate::Intent::run(
        "task.set_status",
        serde_json::json!({"task": vorher.to_string(), "status": "done"}),
    ));
    harness.run();
    assert!(!held(&mut harness, "Nachher"), "predecessor done");
}

/// Review 9e (M4): with a filter, where a card lands is counted among the cards in view.
#[test]
fn with_a_filter_a_card_lands_among_the_cards_in_view() {
    let mut harness = board_9e();
    type_into(&mut harness, text::t().board_search, "a", false);
    // Moved one place up among the cards in view – above "Dahinter", below "Bald": counting
    // the hidden ones too would put it elsewhere (counter-check). (Let go over its own place,
    // egui hands the drop to the held card, not the column: nothing to prove there.)
    let dahinter = harness.get_by_label("Dahinter").rect();
    let at = harness.get_by_label("Alt").rect().center();
    drag(
        &mut harness,
        at,
        egui::pos2(at.x, dahinter.center().y - 4.0) - at,
    );
    let order: Vec<String> = query_titles_in_todo(&harness);
    let bald = order.iter().position(|t| t == "Bald").unwrap();
    let alt = order.iter().position(|t| t == "Alt").unwrap();
    let dahinter = order.iter().position(|t| t == "Dahinter").unwrap();
    assert!(bald < alt && alt < dahinter, "{order:?}");
}

/// Review 9e (M10, M11): a chosen card is read as selected; the flag only from "hoch", a
/// critical one says so in words.
#[test]
fn a_board_card_is_read_as_chosen_and_its_priority_in_words() {
    let mut harness = board_9e();
    let alt = task_named(harness.state(), "Alt").unwrap().id;
    harness.state_mut().apply(crate::Intent::Select(Some(alt)));
    harness.run();
    let node = harness.get_by(|n| n.label().as_deref() == Some("Alt"));
    // egui says "selected" as toggled.
    assert_eq!(
        egui_kittest::kittest::NodeT::accesskit_node(&node).toggled(),
        Some(egui::accesskit::Toggled::True)
    );
    assert!(
        harness
            .query_by_label(text::priority(plnwrk_core::Priority::Normal))
            .is_none()
    );
    let heute = task_named(harness.state(), "Heute").unwrap().id;
    harness.state_mut().apply(crate::Intent::run(
        "task.update",
        serde_json::json!({"task": heute.to_string(), "priority": "critical"}),
    ));
    harness.run();
    // In words: a chip wider than the 16 px flag of "hoch".
    let critical = harness
        .get_by_label(text::priority(plnwrk_core::Priority::Critical))
        .rect();
    assert!(critical.width() > 24.0, "{critical:?}");
}

/// Review 9e: a card made with "+" that the filter hides is said, not lost from sight.
#[test]
fn a_new_card_hidden_by_the_filter_is_said() {
    let mut harness = board_9e();
    click(&mut harness, text::t().board_filter_overdue);
    click(&mut harness, &text::new_in_column(TaskStatus::Todo));
    type_where_the_focus_is(&mut harness, "Neu");
    assert!(task_named(harness.state(), "Neu").is_some());
    let said = harness.state().state.message.clone().map(|m| m.text);
    assert_eq!(said, Some(text::created_but_filtered("Neu")));
}

/// The titles of the open top-level tasks of the first project, in their order.
fn query_titles_in_todo(harness: &Harness<'_, PlnwrkApp>) -> Vec<String> {
    let doc = harness.state().doc().unwrap();
    let project = projects(harness.state()).first().unwrap().id;
    let now = doc.session.clock().now_local();
    plnwrk_engine::query::board(doc.session.workspace(), project, now)
        .unwrap()
        .columns
        .into_iter()
        .find(|c| c.status == TaskStatus::Todo)
        .unwrap()
        .tasks
        .into_iter()
        .map(|c| c.title)
        .collect()
}

/// Nachreview 9e (E1): a "+" card that is refused (title too long) is not said as created –
/// the reason stays in view.
#[test]
fn a_refused_new_card_is_not_said_as_created() {
    let mut harness = board_9e();
    click(&mut harness, text::t().board_filter_overdue);
    click(&mut harness, &text::new_in_column(TaskStatus::Todo));
    let long = "x".repeat(201);
    type_where_the_focus_is(&mut harness, &long);
    assert!(task_named(harness.state(), &long).is_none());
    let said = harness.state().state.message.clone();
    assert!(said.as_ref().is_some_and(|m| m.error), "{said:?}");
}

/// Nachreview 9e: a new card in view is not said to be hidden.
#[test]
fn a_new_card_in_view_is_not_said_to_be_hidden() {
    let mut harness = board_9e();
    click(&mut harness, &text::new_in_column(TaskStatus::Todo));
    type_where_the_focus_is(&mut harness, "Sichtbar");
    assert!(task_named(harness.state(), "Sichtbar").is_some());
    assert_ne!(
        harness.state().state.message.clone().map(|m| m.text),
        Some(text::created_but_filtered("Sichtbar"))
    );
}

/// Nachreview 9e (E2): the hint sits at the pointer, below the card held there – also a tall
/// one with subtasks (egui paints the held card over it).
#[test]
fn the_hint_sits_below_a_tall_held_card() {
    let mut harness = board_9e();
    let project = projects(harness.state()).first().unwrap().id.to_string();
    let nachher = task_named(harness.state(), "Nachher")
        .unwrap()
        .id
        .to_string();
    for title in ["Teil 1", "Teil 2", "Teil 3"] {
        harness.state_mut().apply(crate::Intent::run(
            "task.create",
            serde_json::json!({"project": project, "title": title, "parent": nachher}),
        ));
    }
    harness.run();
    harness.get_by_label("Nachher").scroll_to_me();
    harness.run();
    let card = harness.get_by_label("Nachher").rect().center();
    // From the title down to the last subtask: less than the whole card.
    let tall = harness.get_by_label("Teil 3").rect().bottom()
        - harness.get_by_label("Nachher").rect().top();
    let done = egui::pos2(
        column_heading_node(&harness, TaskStatus::Done)
            .rect()
            .center()
            .x,
        card.y.min(700.0),
    );
    harness.drag_at(card);
    harness.run();
    harness.hover_at(card + egui::vec2(10.0, 10.0));
    harness.run();
    harness.hover_at(done);
    harness.run();
    let hint = harness.get_by_label(&text::still_waits_for(1)).rect();
    // Below the held card (its half below the pointer), and close by.
    assert!(
        hint.top() >= done.y + tall / 2.0,
        "{hint:?} at {done:?}, card {tall}"
    );
    assert!(hint.top() <= done.y + 400.0, "{hint:?} at {done:?}");
    assert!((hint.left() - done.x).abs() < 40.0, "{hint:?} at {done:?}");
    harness.drop_at(done);
    harness.run();
}

/// What the last frame painted as text, in paint order (later lies on top), with where –
/// for checks the accessibility tree cannot make: what is drawn, and what covers what.
fn painted_texts(harness: &Harness<'_, PlnwrkApp>) -> Vec<(String, egui::Rect)> {
    fn texts_in(shape: &egui::Shape, out: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Text(text) => {
                out.push((text.galley.text().to_owned(), text.visual_bounding_rect()));
            }
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    texts_in(shape, out);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for clipped in &harness.output().shapes {
        // Only what is inside its clip shows.
        let before = out.len();
        texts_in(&clipped.shape, &mut out);
        for (_, rect) in out.iter_mut().skip(before) {
            *rect = rect.intersect(clipped.clip_rect);
        }
    }
    out.retain(|(_, rect)| rect.width() > 0.0 && rect.height() > 0.0);
    out
}

/// The helper itself: a heading painted in the overview is found where its label is.
#[test]
fn painted_texts_finds_what_is_drawn_and_where() {
    let harness = harness_with_sample();
    let painted = painted_texts(&harness);
    let label = harness
        .get_all_by_label("Digitalfunk Umrüstung Wachen")
        .next()
        .unwrap()
        .rect();
    assert!(
        painted
            .iter()
            .any(|(t, r)| t == "Digitalfunk Umrüstung Wachen" && r.intersects(label)),
        "{painted:?}"
    );
    assert!(!painted.iter().any(|(t, _)| t == "Nicht gezeichnet"));
}

/// Phase 9f (Tobi: "Datumsleiste anheften"): the time axis – weeks, "Heute", the headings of
/// the table – stays at the top of the view while the rows scroll, painted over them.
#[test]
fn the_time_axis_stays_in_view_when_the_rows_scroll() {
    let mut harness = wide_schedule(40);
    let week = |h: &Harness<'_, PlnwrkApp>| {
        painted_texts(h)
            .into_iter()
            .find(|(t, _)| t.starts_with(text::week(1).split(' ').next().unwrap_or("KW")))
            .map(|(_, r)| r)
    };
    let painted = |h: &Harness<'_, PlnwrkApp>, said: &str| {
        painted_texts(h)
            .into_iter()
            .filter(|(t, _)| t == said)
            .map(|(_, r)| r)
            .collect::<Vec<_>>()
    };
    let week_before = week(&harness).expect("a week label");
    let heading_before = painted(&harness, text::t().ribbon.task)[0];
    let today_before = painted(&harness, text::t().today);
    let first = bar(&harness, "Jetzt").top();
    let at = bar(&harness, "Jetzt").center() + egui::vec2(120.0, 0.0);
    for _ in 0..3 {
        wheel(&mut harness, at, Modifiers::NONE);
    }
    let moved = bar(&harness, "Jetzt").top();
    assert!(
        moved < first - 100.0,
        "the rows scrolled: {first} → {moved}"
    );
    let week_after = week(&harness).expect("still painted");
    assert!(
        (week_after.top() - week_before.top()).abs() < 1.0,
        "{week_before:?} → {week_after:?}"
    );
    let heading_after = painted(&harness, text::t().ribbon.task)[0];
    assert!((heading_after.top() - heading_before.top()).abs() < 1.0);
    // "Heute" (the button above says it too): the one in the axis stays in the axis.
    let in_axis = |r: &egui::Rect| {
        r.top() >= heading_before.top() - 40.0 && r.bottom() <= heading_before.bottom() + 2.0
    };
    assert!(today_before.iter().any(in_axis), "{today_before:?}");
    assert!(painted(&harness, text::t().today).iter().any(in_axis));
    // Over the rows scrolled under it: nothing is painted on top of the heading.
    let texts = painted_texts(&harness);
    let last = texts
        .iter()
        .rposition(|(_, r)| r.intersects(heading_after.shrink(1.0)))
        .unwrap();
    assert_eq!(texts[last].0, text::t().ribbon.task, "{:?}", texts[last]);
}

/// Phase 9f: a bar hidden under the pinned time axis cannot be dragged through it.
#[test]
fn a_bar_under_the_time_axis_cannot_be_dragged_through_it() {
    let mut harness = wide_schedule(40);
    let at = bar(&harness, "Jetzt").center() + egui::vec2(120.0, 0.0);
    for _ in 0..3 {
        wheel(&mut harness, at, Modifiers::NONE);
    }
    // The axis: from the top of the view down to just below its headings.
    let heading = painted_texts(&harness)
        .into_iter()
        .find(|(t, _)| t == text::t().ribbon.task)
        .map(|(_, r)| r)
        .unwrap();
    let axis = egui::Rect::from_min_max(
        egui::pos2(0.0, heading.bottom() + 3.0 - 40.0),
        egui::pos2(f32::INFINITY, heading.bottom() + 3.0),
    );
    let (hidden, y) = (0..40)
        .map(|n| format!("Zeile {n}"))
        .find_map(|title| {
            let start = format!("{title}: ");
            let y = harness
                .query_all_by(|n| n.label().is_some_and(|l| l.starts_with(&start)))
                .map(|n| n.rect().center().y)
                .find(|y| *y - 8.0 >= axis.top() && *y + 8.0 <= axis.bottom())?;
            Some((title, y))
        })
        .expect("a bar under the axis");
    let id = task_named(harness.state(), &hidden).unwrap().id;
    let before = dates_of(&harness, id);
    let start = format!("{hidden}: ");
    let x = harness
        .query_all_by(|n| n.label().is_some_and(|l| l.starts_with(&start)))
        .map(|n| n.rect().center().x)
        .fold(f32::MIN, f32::max);
    drag(&mut harness, egui::pos2(x, y), egui::vec2(66.0, 0.0));
    assert_eq!(dates_of(&harness, id), before);
}

/// A new file with the new-project dialog open ("+ Neu": a case is preselected).
fn new_project_dialog() -> Harness<'static, PlnwrkApp> {
    let mut harness = harness(PlnwrkApp::default());
    click(&mut harness, text::t().new_file);
    click(&mut harness, text::t().new_project);
    harness
}

/// Phase 9f: Enter creates; without a title nothing is created, and it says why.
#[test]
fn the_new_dialog_creates_with_enter_and_waits_for_a_title() {
    let mut harness = new_project_dialog();
    harness.get_by_label(text::t().create).click();
    harness.run();
    assert!(
        projects(harness.state()).is_empty(),
        "no title, nothing created"
    );
    assert!(harness.query_by_label(text::t().title_needed).is_some());
    // Said in the dialog only – not as a refusal in the status line too.
    assert!(
        harness.state().state.message.is_none(),
        "{:?}",
        harness.state().state.message
    );
    type_into(&mut harness, text::t().title, "SKA 23/999", true);
    let created = projects(harness.state());
    assert_eq!(created.len(), 1);
    // Preselected: a case.
    assert!(created[0].is_case(), "{:?}", created[0].kind);
}

/// Phase 9f: the choice writes the kind; a reference only for a case – typed and then
/// switched to a project, it is kept in the form but not given to the project.
#[test]
fn a_project_from_the_dialog_is_no_case_even_with_a_reference_typed() {
    let mut harness = new_project_dialog();
    type_into(&mut harness, text::t().reference, "23/1234", false);
    click(&mut harness, text::t().kinds[0]);
    assert!(
        harness.query_by_label(text::t().reference).is_none(),
        "no reference for a project"
    );
    type_into(&mut harness, text::t().title, "Umbau Wache", true);
    let created = projects(harness.state());
    assert_eq!(created.len(), 1);
    assert!(!created[0].is_case());
    assert_eq!(created[0].reference, None);
    assert_eq!(created[0].kind.as_deref(), Some(text::t().kinds[0]));
}

/// Phase 9f: "Person anlegen…" right in the choice: Enter adds the person and puts them in
/// charge – the project is not created by that Enter.
#[test]
fn a_person_is_added_right_in_the_new_dialog() {
    let mut harness = new_project_dialog();
    type_into(&mut harness, text::t().title, "Übung", false);
    harness.get_by_label(text::t().owner).click();
    harness.run();
    click(&mut harness, text::t().add_person);
    type_where_the_focus_is(&mut harness, "Anna Berg");
    assert!(
        projects(harness.state()).is_empty(),
        "Enter added the person only"
    );
    let anna = harness
        .state()
        .doc()
        .unwrap()
        .session
        .workspace()
        .people()
        .find(|p| p.name == "Anna Berg")
        .map(|p| p.id);
    assert!(anna.is_some());
    let Some(crate::state::Dialog::NewProject(form)) = harness.state().state.dialog.clone() else {
        panic!("the dialog stays open")
    };
    assert_eq!(form.owner, anna);
    assert!(form.new_person.is_none());
}

/// Phase 9f: "Aus Vorlage" is greyed while there is no template – and possible once there is.
#[test]
fn from_template_waits_for_a_template() {
    let mut harness = harness(PlnwrkApp::default());
    click(&mut harness, text::t().new_file);
    // In the sidebar and in the ribbon (each has its own wording).
    let disabled = |h: &Harness<'_, PlnwrkApp>| {
        let all: Vec<bool> = [text::t().from_template, text::t().from_template_title]
            .iter()
            .flat_map(|label| h.query_all_by_label(label).collect::<Vec<_>>())
            .map(|n| egui_kittest::kittest::NodeT::accesskit_node(&n).is_disabled())
            .collect();
        assert!(all.len() >= 2, "{all:?}");
        all.iter().all(|d| *d)
    };
    assert!(disabled(&harness));
    harness.state_mut().apply(crate::Intent::run(
        "project.create",
        serde_json::json!({"title": "Muster", "template": true}),
    ));
    harness.run();
    assert!(!disabled(&harness));
}

/// Phase 9f: the base kinds are shown in the language of the interface; others as typed.
#[test]
fn base_kinds_are_shown_in_the_language_of_the_interface() {
    assert_eq!(text::kind_shown("Vorgang"), text::t().kinds[1]);
    assert_eq!(text::kind_shown("case"), text::t().kinds[1]);
    assert_eq!(text::kind_shown("Project"), text::t().kinds[0]);
    assert_eq!(text::kind_shown("SKA"), "SKA");
}

/// Phase 9f (known since the real test): a case without a reference gets a time at its
/// deadlines too – the one rule `is_case`, here on a milestone's card.
#[test]
fn a_case_without_a_reference_offers_a_time_at_its_deadlines() {
    let mut harness = one_milestone();
    assert!(projects(harness.state())[0].reference.is_none());
    let times = |h: &Harness<'_, PlnwrkApp>| {
        h.query_all_by(|n| n.placeholder() == Some(text::t().time_hint))
            .count()
    };
    // The form for a new milestone has one already; the card's date gets its own.
    let before = times(&harness);
    assert!(before >= 1);
    click(&mut harness, text::t().change_date);
    assert_eq!(times(&harness), before + 1);
}

/// Phase 9f: the start page offers two ways in, each said in a sentence, with buttons side by
/// side and level; in the browser its limits stand in the footer only, not in the page.
#[test]
fn the_start_page_offers_two_ways_in_said_in_a_sentence() {
    let (harness, _browser) = in_browser();
    for sentence in [text::t().new_file_explained, text::t().sample_explained] {
        assert!(harness.query_by_label(sentence).is_some(), "{sentence}");
    }
    let new = harness.get_by_label(text::t().new_file).rect();
    let sample = harness.get_by_label(text::t().open_sample).rect();
    assert!(sample.left() > new.right(), "side by side");
    assert!(
        (new.top() - sample.top()).abs() < 1.0,
        "level: {new:?} {sample:?}"
    );
    // The browser's limits: in the footer, said once – not in the page as well.
    assert_eq!(
        harness.query_all_by_label(text::t().browser_not_vs).count(),
        1
    );
    assert!(
        harness
            .query_by_label(text::t().browser_not_vs_hint)
            .is_none()
    );
}

/// A project with: "Alt" 2 days overdue, "Heute" due today, "Morgen" tomorrow, "Fertig" due
/// today but done, "Mit Teilen" due today with an open subtask, milestone "Ziel" due today.
fn today_and_overdue() -> Harness<'static, PlnwrkApp> {
    let mut harness = with_project("Lage");
    let project = projects(harness.state()).first().unwrap().id.to_string();
    for (title, days, extra) in [
        ("Alt", -2, serde_json::json!({})),
        ("Heute", 0, serde_json::json!({})),
        ("Morgen", 1, serde_json::json!({})),
        ("Fertig", 0, serde_json::json!({"status": "done"})),
        ("Mit Teilen", 0, serde_json::json!({})),
        ("Ziel", 0, serde_json::json!({"milestone": true})),
    ] {
        let mut params = serde_json::json!({"project": project, "title": title,
                                            "due": day_from_today(&harness, days)});
        if let (Some(p), Some(e)) = (params.as_object_mut(), extra.as_object()) {
            p.extend(e.clone());
        }
        harness
            .state_mut()
            .apply(crate::Intent::run("task.create", params));
    }
    harness.run();
    let parent = task_named(harness.state(), "Mit Teilen")
        .unwrap()
        .id
        .to_string();
    harness.state_mut().apply(crate::Intent::run(
        "task.create",
        serde_json::json!({"project": project, "title": "Teil", "parent": parent}),
    ));
    harness
        .state_mut()
        .apply(crate::Intent::Go(crate::state::View::Overview));
    harness.run();
    harness
}

/// Phase 9f: "Heute und überfällig" lists what is overdue and what is due today – not what is
/// done or due tomorrow – each ticked off right there.
#[test]
fn today_and_overdue_are_ticked_off_in_the_overview() {
    let mut harness = today_and_overdue();
    let tick = |title: &str| text::mark_done(title);
    for listed in ["Alt", "Heute", "Mit Teilen", "Ziel"] {
        assert!(harness.query_by_label(&tick(listed)).is_some(), "{listed}");
    }
    for not_listed in ["Morgen", "Fertig"] {
        assert!(
            harness.query_by_label(&tick(not_listed)).is_none(),
            "{not_listed}"
        );
    }
    // A task: done at once.
    harness.get_by_label(&tick("Heute")).click();
    harness.run();
    assert_eq!(
        task_named(harness.state(), "Heute").unwrap().status,
        TaskStatus::Done
    );
    // With an open subtask: asked about it first.
    harness.get_by_label(&tick("Mit Teilen")).click();
    harness.run();
    assert!(matches!(
        harness.state().state.dialog,
        Some(crate::state::Dialog::CloseSubtasks { .. })
    ));
    click(&mut harness, text::t().cancel);
    // A milestone: the question about milestones.
    harness.get_by_label(&tick("Ziel")).click();
    harness.run();
    assert!(asking(&harness));
    assert_ne!(
        task_named(harness.state(), "Ziel").unwrap().status,
        TaskStatus::Done
    );
}

/// Phase 9f: the project's light on its card – at risk with overdue work; on plan without;
/// overdue once its own deadline passed.
#[test]
fn a_project_card_shows_its_light() {
    let mut harness = today_and_overdue();
    let chip = |h: &Harness<'_, PlnwrkApp>, light: super::views::milestones::Light| {
        h.query_by_label(light.name()).is_some()
    };
    use super::views::milestones::Light;
    assert!(chip(&harness, Light::AtRisk), "overdue work");
    assert!(!chip(&harness, Light::OnPlan));
    // Its reason is in the facts already ("… · 1 überfällig"): not said twice.
    assert!(harness.query_by_label(&text::overdue_tasks(1)).is_none());
    let alt = task_named(harness.state(), "Alt").unwrap().id.to_string();
    harness.state_mut().apply(crate::Intent::run(
        "task.set_status",
        serde_json::json!({"task": alt, "status": "done"}),
    ));
    harness.run();
    assert!(chip(&harness, Light::OnPlan), "nothing late any more");
    let project = projects(harness.state()).first().unwrap().id.to_string();
    let yesterday = day_from_today(&harness, -1);
    harness.state_mut().apply(crate::Intent::run(
        "project.update",
        serde_json::json!({"project": project, "due": yesterday}),
    ));
    harness.run();
    assert!(chip(&harness, Light::Overdue), "its own deadline passed");
}

/// Phase 9f: the sidebar groups cases and projects (the rule `is_case`), each group only when
/// it has entries.
#[test]
fn the_sidebar_groups_cases_and_projects() {
    let harness = harness_with_sample();
    assert!(harness.query_by_label(text::t().cases_heading).is_some());
    assert!(harness.query_by_label(text::t().projects_heading).is_some());
    let mut harness = harness_after_new_file();
    harness.state_mut().apply(crate::Intent::run(
        "project.create",
        serde_json::json!({"title": "Umbau", "kind": text::t().kinds[0]}),
    ));
    harness.run();
    assert!(harness.query_by_label(text::t().projects_heading).is_some());
    assert!(
        harness.query_by_label(text::t().cases_heading).is_none(),
        "no cases, no heading"
    );
}

fn harness_after_new_file() -> Harness<'static, PlnwrkApp> {
    let mut harness = harness(PlnwrkApp::default());
    click(&mut harness, text::t().new_file);
    harness
}

/// Phase 9f: cards that do not fit side by side go to the next row (egui does not wrap
/// framed cards itself).
#[test]
fn overview_cards_wrap_into_rows() {
    let mut harness = harness_with_sample();
    harness.set_size(egui::vec2(900.0, 900.0));
    harness.run();
    let first = harness
        .get_all_by_label("SKA 23/1234 Sirenenwarnung")
        .map(|n| n.rect())
        .find(|r| r.left() > 250.0)
        .unwrap();
    let second = harness
        .get_all_by_label("Digitalfunk Umrüstung Wachen")
        .map(|n| n.rect())
        .find(|r| r.left() > 250.0)
        .unwrap();
    assert!(second.top() > first.bottom(), "{first:?} {second:?}");
    assert!(second.right() <= 900.0);
}

/// "+ Unteraufgabe" of a task in the list: shown where the pointer is (Phase 9f), so the
/// pointer goes to its row first.
fn add_subtask_to(harness: &mut Harness<'_, PlnwrkApp>, title: &str) {
    let row = harness.get_by_label(title).rect().center();
    harness.hover_at(row);
    // A tooltip fades in there: a few frames, not "until still".
    harness.run_steps(3);
    harness.get_by_label(text::t().add_subtask).click();
    harness.run_steps(3);
}

/// The filled rectangles the last frame painted (colour and where) – for highlights.
fn painted_fills(harness: &Harness<'_, PlnwrkApp>) -> Vec<(egui::Color32, egui::Rect)> {
    fn fills_in(shape: &egui::Shape, out: &mut Vec<(egui::Color32, egui::Rect)>) {
        match shape {
            egui::Shape::Rect(rect) if rect.fill != egui::Color32::TRANSPARENT => {
                out.push((rect.fill, rect.rect));
            }
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    fills_in(shape, out);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for clipped in &harness.output().shapes {
        fills_in(&clipped.shape, &mut out);
    }
    out
}

/// A project with "Packen" (in progress, Anna Berg) and "Laden" (open), in the list.
fn list_with_columns() -> Harness<'static, PlnwrkApp> {
    let mut harness = with_project("Umzug");
    let project = projects(harness.state()).first().unwrap().id.to_string();
    harness.state_mut().apply(crate::Intent::run(
        "person.create",
        serde_json::json!({"name": "Anna Berg"}),
    ));
    harness.run();
    let anna = harness
        .state()
        .doc()
        .unwrap()
        .session
        .workspace()
        .people()
        .next()
        .unwrap()
        .id
        .to_string();
    for (title, extra) in [
        (
            "Packen",
            serde_json::json!({"status": "in_progress", "assignee": anna}),
        ),
        ("Laden", serde_json::json!({})),
    ] {
        let mut params = serde_json::json!({"project": project, "title": title});
        if let (Some(p), Some(e)) = (params.as_object_mut(), extra.as_object()) {
            p.extend(e.clone());
        }
        harness
            .state_mut()
            .apply(crate::Intent::run("task.create", params));
    }
    harness.run();
    harness
}

/// Phase 9f: the list has columns with headings – deadline, who, status – that give way from
/// the right in a narrow window; the values stand in them.
#[test]
fn the_list_has_columns_with_headings() {
    let mut harness = list_with_columns();
    let painted = |h: &Harness<'_, PlnwrkApp>| {
        painted_texts(h)
            .into_iter()
            .map(|(t, _)| t)
            .collect::<Vec<_>>()
    };
    let headings = [
        text::t().ribbon.task,
        text::t().due,
        text::t().assignee,
        text::t().status,
    ];
    let now = painted(&harness);
    for heading in headings {
        assert!(now.iter().any(|t| t == heading), "{heading}: {now:?}");
    }
    // The values: the person's full name, a status other than "Offen".
    assert!(harness.query_by_label("Anna Berg").is_some());
    assert!(
        harness
            .query_by_label(text::task_status(TaskStatus::InProgress))
            .is_some()
    );
    assert!(
        harness
            .query_by_label(text::task_status(TaskStatus::Todo))
            .is_none(),
        "„Offen“ goes without saying"
    );
    // Narrow: the status gives way first; the title stays.
    harness.set_size(egui::vec2(760.0, 900.0));
    harness.run();
    let narrow = painted(&harness);
    assert!(!narrow.iter().any(|t| t == text::t().status), "{narrow:?}");
    assert!(narrow.iter().any(|t| t == "Packen"));
}

/// Phase 9f: "+ Unteraufgabe" shows where the pointer is and at the chosen task – not on
/// every row.
#[test]
fn add_subtask_shows_only_at_the_row_in_question() {
    let mut harness = list_with_columns();
    // The pointer away from the list.
    harness.hover_at(egui::pos2(5.0, 5.0));
    harness.run_steps(3);
    assert_eq!(harness.query_all_by_label(text::t().add_subtask).count(), 0);
    let row = harness.get_by_label("Laden").rect().center();
    harness.hover_at(row);
    harness.run_steps(3);
    assert_eq!(harness.query_all_by_label(text::t().add_subtask).count(), 1);
    harness.hover_at(egui::pos2(5.0, 5.0));
    let packen = task_named(harness.state(), "Packen").unwrap().id;
    harness
        .state_mut()
        .apply(crate::Intent::Select(Some(packen)));
    harness.run_steps(3);
    assert_eq!(
        harness.query_all_by_label(text::t().add_subtask).count(),
        1,
        "at the chosen one"
    );
}

/// Phase 9f: a task just made from the list is lit up – that one only – and not for long.
#[test]
fn a_new_task_is_lit_up_for_a_moment() {
    let mut harness = list_with_columns();
    type_into(&mut harness, text::t().new_task_hint, "Fahren", true);
    let fahren = task_named(harness.state(), "Fahren").unwrap().id;
    assert_eq!(harness.state().state.fresh, Some(fahren));
    let lit = |h: &Harness<'_, PlnwrkApp>, title: &str| {
        let soft = [
            crate::design::tokens::of(false).accent_soft,
            crate::design::tokens::of(true).accent_soft,
        ];
        let row = h.get_by_label(title).rect();
        painted_fills(h)
            .iter()
            .any(|(c, r)| soft.contains(c) && r.contains(row.center()))
    };
    assert!(lit(&harness, "Fahren"));
    assert!(!lit(&harness, "Laden"), "only the new one");
    // Two seconds later (60 frames a second): no longer.
    harness.run_steps(120);
    assert!(!lit(&harness, "Fahren"));
    assert_eq!(harness.state().state.fresh, None);
}

/// Phase 9f: the details in groups that fold away; a date typed and then folded away is not
/// lost – it counts as unsaved and is taken like any typed text.
#[test]
fn the_details_fold_in_groups_and_keep_what_was_typed() {
    let (mut harness, _, _) = two_dated_tasks();
    click(&mut harness, "Vergabe");
    let t = text::t();
    for heading in [
        t.group_assignment,
        t.group_dates,
        t.subtasks,
        t.waits_for,
        t.group_notes,
    ] {
        assert!(harness.query_by_label(heading).is_some(), "{heading}");
    }
    type_into(&mut harness, t.due, "20.10.2026", false);
    click(&mut harness, t.group_dates);
    assert!(harness.query_by_label(t.due).is_none(), "folded away");
    assert!(harness.state().has_unsaved_changes());
    assert!(harness.state_mut().flush_draft().is_none());
    assert_eq!(
        task_named(harness.state(), "Vergabe")
            .unwrap()
            .due
            .map(|d| d.to_string()),
        Some("2026-10-20".to_owned())
    );
}

/// Phase 9f: "Aufgabe löschen" stands apart at the bottom, below everything else.
#[test]
fn delete_stands_apart_at_the_bottom_of_the_details() {
    let (mut harness, _, _) = two_dated_tasks();
    harness.set_size(egui::vec2(1400.0, 2400.0));
    click(&mut harness, "Vergabe");
    let delete = harness.get_by_label(text::t().delete_task).rect();
    let notes = harness.get_by_label(text::t().group_notes).rect();
    let history = harness.get_by_label(text::t().history).rect();
    assert!(delete.top() > notes.bottom() && delete.top() > history.bottom() + 16.0);
}

/// Phase 9f: a date that does not fit what a task waits for is marked at both tasks – in the
/// schedule's words – and not at a task that has nothing to do with it.
#[test]
fn a_date_conflict_is_marked_in_the_details() {
    let (mut harness, first, second) = two_dated_tasks();
    let project = projects(harness.state()).first().unwrap().id.to_string();
    harness.state_mut().apply(crate::Intent::run(
        "task.create",
        serde_json::json!({"project": project, "title": "Abseits", "due": "2026-10-20"}),
    ));
    // "Vergabe" (from 15.10.) waits for "Ausschreibung" moved to end on 18.10.: a conflict.
    harness.state_mut().apply(crate::Intent::run(
        "task.update",
        serde_json::json!({"task": first.to_string(), "due": "2026-10-18"}),
    ));
    harness.state_mut().apply(crate::Intent::run(
        "dependency.add",
        serde_json::json!({"from": first.to_string(), "to": second.to_string()}),
    ));
    harness.run();
    let said = text::waits_too_short("Ausschreibung", "Vergabe");
    let abseits = task_named(harness.state(), "Abseits").unwrap().id;
    for (task, marked) in [(second, true), (first, true), (abseits, false)] {
        harness.state_mut().apply(crate::Intent::Select(Some(task)));
        harness.run();
        assert_eq!(harness.query_by_label(&said).is_some(), marked, "{task}");
    }
}

/// Phase 9f: every empty view says so with a sentence and offers the step that helps – which
/// puts the cursor where the next thing is typed.
#[test]
fn empty_views_offer_the_next_step() {
    let t = text::t();
    // The overview: a new case.
    let mut harness = harness_after_new_file();
    assert!(harness.query_by_label(t.nothing_yet).is_some());
    click(&mut harness, t.first_case);
    let Some(crate::state::Dialog::NewProject(form)) = harness.state().state.dialog.clone() else {
        panic!("the dialog for a new case")
    };
    assert!(plnwrk_core::is_case(Some(&form.kind), None));
    // The list: the cursor in the field for a new task.
    let mut harness = with_project("Leer");
    assert!(harness.query_by_label(t.no_tasks).is_some());
    click(&mut harness, t.first_task);
    type_where_the_focus_is(&mut harness, "Erste");
    assert!(task_named(harness.state(), "Erste").is_some());
    assert!(
        harness.query_by_label(t.no_tasks).is_none(),
        "not empty any more"
    );
    // The milestones: the cursor in the title of a new one.
    click(&mut harness, t.milestones);
    assert!(harness.query_by_label(t.milestones_empty).is_some());
    click(&mut harness, t.first_milestone);
    harness.event(egui::Event::Text("Abgabe".to_owned()));
    harness.run();
    assert_eq!(harness.state().state.forms.milestone_title, "Abgabe");
    // The people: the cursor in the name of a new person.
    harness
        .state_mut()
        .apply(crate::Intent::Go(crate::state::View::People));
    harness.run();
    assert!(harness.query_by_label(t.people_empty).is_some());
    click(&mut harness, t.add_person);
    harness.event(egui::Event::Text("Anna".to_owned()));
    harness.run();
    assert_eq!(harness.state().state.forms.person.name, "Anna");
}

/// Phase 9f: a message stands once, right under the ribbon (not in the status bar), until it
/// is closed.
#[test]
fn a_message_stands_once_under_the_ribbon() {
    let mut harness = with_project("Umzug");
    harness.state_mut().state.message = Some(crate::state::Message::error("Etwas ging schief."));
    harness.run();
    let shown: Vec<egui::Rect> = harness
        .query_all_by_label("Etwas ging schief.")
        .map(|n| n.rect())
        .collect();
    assert_eq!(shown.len(), 1, "one place");
    // Above the content (the project's heading), not at the bottom.
    let heading = harness
        .get_all_by_label("Umzug")
        .map(|n| n.rect())
        .fold(f32::MAX, |a, r| a.min(r.top()));
    assert!(shown[0].bottom() <= heading, "{shown:?} above {heading}");
    click(&mut harness, text::t().dismiss_message);
    assert!(harness.state().state.message.is_none());
    assert!(harness.query_by_label("Etwas ging schief.").is_none());
}

/// Known since the real test (fixed in 9f): with the details open the middle is narrow – the
/// project's buttons must not lie over its title; there is room, they stand beside it.
#[test]
fn the_project_buttons_never_lie_over_its_title() {
    let mut harness = harness_with_sample();
    harness
        .get_all_by_label("Digitalfunk Umrüstung Wachen")
        .next()
        .unwrap()
        .click();
    harness.run();
    let overlaps = |h: &Harness<'_, PlnwrkApp>| {
        // The heading is a plain label: its text is the value.
        let title = h
            .query_all_by(|n| n.value().as_deref() == Some("Digitalfunk Umrüstung Wachen"))
            .map(|n| n.rect())
            .find(|r| r.left() > 200.0)
            .unwrap();
        let edit = h.get_by_label(text::t().edit).rect();
        (title.intersects(edit), edit.top() > title.bottom())
    };
    // Wide: beside it on one line.
    assert_eq!(overlaps(&harness), (false, false));
    // Narrow with the details open: below it, not over it.
    harness.set_size(egui::vec2(980.0, 900.0));
    let task = task_named(harness.state(), "Einbau Wache 11").unwrap().id;
    harness.state_mut().apply(crate::Intent::Select(Some(task)));
    harness.run();
    assert_eq!(overlaps(&harness), (false, true));
}

/// Review 9f (F1): Enter on a button of the new dialog is a click of that button – on
/// "Abbrechen", the segment or the calendar it creates nothing.
#[test]
fn enter_on_a_button_of_the_new_dialog_creates_nothing() {
    for button in [text::t().cancel, text::t().kinds[0], text::t().pick_date] {
        let mut harness = new_project_dialog();
        type_into(&mut harness, text::t().title, "Übung", false);
        harness.get_all_by_label(button).next().unwrap().focus();
        harness.run();
        harness.key_press(Key::Enter);
        harness.run_steps(3);
        assert!(projects(harness.state()).is_empty(), "Enter on {button}");
    }
}

/// Review 9f (F2, F3): the field for a new person takes the cursor once; typing the title
/// afterwards goes into the title; left empty it goes; Escape there drops only the person.
#[test]
fn the_new_person_field_does_not_take_over_the_form() {
    let mut harness = new_project_dialog();
    harness.get_by_label(text::t().owner).click();
    harness.run();
    click(&mut harness, text::t().add_person);
    // Back to the title: typed there, Enter creates the project – no person "Titel".
    type_into(&mut harness, text::t().title, "Titel", true);
    let people = |h: &Harness<'_, PlnwrkApp>| {
        h.state()
            .doc()
            .unwrap()
            .session
            .workspace()
            .people()
            .map(|p| p.name.clone())
            .collect::<Vec<_>>()
    };
    assert!(people(&harness).is_empty(), "{:?}", people(&harness));
    assert!(projects(harness.state()).iter().any(|p| p.title == "Titel"));
    // Escape while a person is being named: only the person goes.
    let mut harness = new_project_dialog();
    type_into(&mut harness, text::t().title, "Übung", false);
    harness.get_by_label(text::t().owner).click();
    harness.run();
    click(&mut harness, text::t().add_person);
    harness.event(egui::Event::Text("Ann".to_owned()));
    harness.run();
    harness.key_press(Key::Escape);
    harness.run();
    let Some(crate::state::Dialog::NewProject(form)) = harness.state().state.dialog.clone() else {
        panic!("the dialog stays")
    };
    assert_eq!(form.title, "Übung");
    assert!(form.new_person.is_none());
}

/// Review 9f (F4): a title too long is said at the field and "Anlegen" waits; a refusal while
/// a dialog is open is said in the dialog – once – not behind it.
#[test]
fn refusals_are_said_in_the_dialog() {
    let mut harness = new_project_dialog();
    type_into(&mut harness, text::t().title, &"x".repeat(201), false);
    let said = text::title_too_long(plnwrk_core::text::TITLE_MAX);
    assert!(harness.query_by_label(&said).is_some());
    let create = harness.get_by_label(text::t().create);
    assert!(egui_kittest::kittest::NodeT::accesskit_node(&create).is_disabled());
    harness.state_mut().state.message = Some(crate::state::Message::error("Abgelehnt."));
    harness.run();
    assert_eq!(harness.query_all_by_label("Abgelehnt.").count(), 1);
    // In the dialog: above its buttons' bottom, not in the bar under the ribbon.
    assert!(harness.query_by_label(text::t().dismiss_message).is_none());
}

/// Review 9f (F5): "Aus Vorlage" decides "case" by the one rule – a template of kind "SKA"
/// gives a case, with a time at its deadline, also without a reference typed.
#[test]
fn a_case_from_a_template_offers_a_time_without_a_reference() {
    let mut harness = harness_with_sample();
    let template = projects(harness.state())
        .into_iter()
        .find(|p| p.template)
        .unwrap();
    assert!(template.is_case());
    harness
        .state_mut()
        .apply(crate::Intent::Open(crate::state::Dialog::FromTemplate(
            crate::state::TemplateForm {
                template: Some(template.id),
                ..crate::state::TemplateForm::default()
            },
        )));
    harness.run();
    assert!(
        harness
            .query_by(|n| n.placeholder() == Some(text::t().time_hint))
            .is_some()
    );
}

/// Review 9f (gap): a wrong date keeps "Anlegen" greyed (not only said at the field).
#[test]
fn a_wrong_date_greys_create() {
    let mut harness = new_project_dialog();
    type_into(&mut harness, text::t().due, "31.02.2026", false);
    type_into(&mut harness, text::t().title, "Umzug", false);
    let create = harness.get_by_label(text::t().create);
    assert!(egui_kittest::kittest::NodeT::accesskit_node(&create).is_disabled());
}

/// Review 9f (K1, K2): another view clears the last message; cards never stick out at the
/// right of a narrow window – and stand side by side where there is room.
#[test]
fn messages_go_with_the_view_and_cards_fit_the_window() {
    let mut harness = harness_with_sample();
    harness.state_mut().state.message = Some(crate::state::Message::error("Alt."));
    harness
        .state_mut()
        .apply(crate::Intent::Go(crate::state::View::People));
    harness.run();
    assert!(harness.state().state.message.is_none());
    harness
        .state_mut()
        .apply(crate::Intent::Go(crate::state::View::Overview));
    harness.set_size(egui::vec2(1600.0, 900.0));
    harness.run();
    let card = |h: &Harness<'_, PlnwrkApp>, title: &str| {
        h.get_all_by_label(title)
            .map(|n| n.rect())
            .find(|r| r.left() > 250.0)
            .unwrap()
    };
    let first = card(&harness, "SKA 23/1234 Sirenenwarnung");
    let second = card(&harness, "Digitalfunk Umrüstung Wachen");
    assert!(
        (first.top() - second.top()).abs() < 1.0,
        "side by side when wide"
    );
    // Narrower than a card beside the sidebar: the card gives way.
    harness.set_size(egui::vec2(480.0, 900.0));
    harness.run();
    for title in ["SKA 23/1234 Sirenenwarnung", "Digitalfunk Umrüstung Wachen"] {
        assert!(
            card(&harness, title).right() <= 480.0,
            "{title}: {:?}",
            card(&harness, title)
        );
    }
}

/// The new dialog with the field for a new person open (chosen in the list).
fn new_person_chosen() -> Harness<'static, PlnwrkApp> {
    let mut harness = new_project_dialog();
    type_into(&mut harness, text::t().title, "Übung", false);
    harness.get_by_label(text::t().owner).click();
    harness.run();
    click(&mut harness, text::t().add_person);
    harness.run();
    harness
}

fn new_form(harness: &Harness<'_, PlnwrkApp>) -> crate::state::ProjectForm {
    let Some(crate::state::Dialog::NewProject(form)) = harness.state().state.dialog.clone() else {
        panic!("the new dialog is open")
    };
    form
}

/// Nachreview 9f (N1): Escape at once in the (empty) field for a new person drops only it.
#[test]
fn escape_in_the_empty_person_field_keeps_the_dialog() {
    let mut harness = new_person_chosen();
    harness.key_press(Key::Escape);
    harness.run();
    let form = new_form(&harness);
    assert_eq!(form.title, "Übung");
    assert!(form.new_person.is_none());
}

/// Nachreview 9f (K1): the person's field does not take the cursor back – typing in the title
/// goes there – and left empty it goes away.
#[test]
fn the_person_field_lets_go_of_the_cursor_and_goes_when_left_empty() {
    let mut harness = new_person_chosen();
    assert!(new_form(&harness).new_person.is_some());
    // Into the title with the mouse, then typing.
    let title = harness.get_by_label(text::t().title).rect().center();
    harness.hover_at(title);
    harness.run();
    tap(&mut harness, title);
    harness.run_steps(3);
    harness.event(egui::Event::Text("!".to_owned()));
    harness.run();
    let form = new_form(&harness);
    assert_eq!(form.title, "Übung!");
    assert!(form.new_person.is_none(), "left empty: gone");
}

/// Nachreview 9f (N3): a name typed for a new person and not added yet is not lost by Enter in
/// the title – nothing is created until it is added or emptied, and it says so.
#[test]
fn a_typed_person_is_not_lost_by_enter() {
    let mut harness = new_person_chosen();
    harness.event(egui::Event::Text("Ann".to_owned()));
    harness.run();
    type_into(&mut harness, text::t().title, "Übung", true);
    assert!(projects(harness.state()).is_empty());
    assert!(harness.query_by_label(text::t().person_pending).is_some());
    assert_eq!(new_form(&harness).new_person.as_deref(), Some("Ann"));
}

/// Nachreview 9f (N2): a refusal said earlier is not said again in a later dialog.
#[test]
fn an_old_refusal_does_not_show_in_a_new_dialog() {
    let mut harness = harness_after_new_file();
    harness.state_mut().state.message = Some(crate::state::Message::error("Früher abgelehnt."));
    harness.run();
    click(&mut harness, text::t().new_project);
    assert!(harness.query_by_label("Früher abgelehnt.").is_none());
    // One that comes up while it is open is said in it.
    harness.state_mut().state.message = Some(crate::state::Message::error("Jetzt abgelehnt."));
    harness.run();
    assert_eq!(harness.query_all_by_label("Jetzt abgelehnt.").count(), 1);
}

/// Nachreview 9f (K2): Enter in every field of the form creates – reference, start, deadline,
/// its time – not only in the title.
#[test]
fn enter_in_any_field_of_the_new_dialog_creates() {
    let t = text::t();
    let today = jiff::Zoned::now().date();
    let day = crate::dates::format(today);
    for (field, typed) in [
        (t.reference, "23/1"),
        (t.start, day.as_str()),
        (t.due, day.as_str()),
        (t.time_hint, "14:00"),
    ] {
        let mut harness = new_project_dialog();
        type_into(&mut harness, t.title, "Übung", false);
        // A time needs its day.
        if field == t.time_hint {
            type_into(&mut harness, t.due, &day, false);
        }
        type_into(&mut harness, field, typed, true);
        assert_eq!(projects(harness.state()).len(), 1, "Enter in {field}");
    }
}

/// Nachreview 9f (K3): a long overdue line does not widen the overview past the window; the
/// card's title is a link.
#[test]
fn a_long_line_does_not_widen_the_overview() {
    let mut harness = harness_with_sample();
    let project = projects(harness.state())
        .into_iter()
        .find(|p| !p.template && !p.is_case())
        .unwrap()
        .id
        .to_string();
    let long = "Eine sehr lange überfällige Aufgabe mit einem Titel, der in einer Zeile weit über das Fenster hinausreichen würde";
    let yesterday = day_from_today(&harness, -1);
    harness.state_mut().apply(crate::Intent::run(
        "task.create",
        serde_json::json!({"project": project, "title": long, "due": yesterday}),
    ));
    harness.set_size(egui::vec2(700.0, 900.0));
    harness.run();
    let roles: Vec<(egui::accesskit::Role, egui::Rect)> = harness
        .get_all_by_label("Digitalfunk Umrüstung Wachen")
        .map(|n| {
            (
                egui_kittest::kittest::NodeT::accesskit_node(&n).role(),
                n.rect(),
            )
        })
        .collect();
    let card = roles
        .iter()
        .find(|(role, _)| *role == egui::accesskit::Role::Link)
        .map(|(_, r)| *r)
        .unwrap_or_else(|| panic!("the card's title is a link: {roles:?}"));
    assert!(card.right() <= 700.0, "{card:?}");
    // The long line itself wraps inside the window.
    let line = harness
        .query_all_by_label_contains("Eine sehr lange überfällige Aufgabe")
        .map(|n| n.rect())
        .next()
        .unwrap();
    assert!(line.right() <= 700.0, "{line:?}");
}

/// Nachreview 9f (K4): a refusal on the way to another view (a title emptied) stays said there.
#[test]
fn a_refusal_on_the_way_out_stays_said() {
    let (mut harness, first, _) = two_dated_tasks();
    harness
        .state_mut()
        .apply(crate::Intent::Select(Some(first)));
    harness.run();
    let title = harness
        .query_all_by(|n| n.value().as_deref() == Some("Ausschreibung"))
        .find(|n| {
            egui_kittest::kittest::NodeT::accesskit_node(n).role()
                == egui::accesskit::Role::TextInput
        })
        .unwrap();
    title.focus();
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.run();
    harness.key_press(Key::Backspace);
    harness.run();
    harness
        .state_mut()
        .apply(crate::Intent::Go(crate::state::View::People));
    harness.run();
    let said = harness
        .state()
        .state
        .message
        .clone()
        .map(|m| m.text)
        .unwrap_or_default();
    assert!(said.contains(text::t().not_taken), "{said}");
}
