//! Several people on one shared file in the app (Phase 10a): who works here, saving by itself,
//! fetching what others saved, conflicts.

use super::*;
use crate::state::Dialog;

/// The sample saved as a file nobody has opened in plnwrk yet.
fn shared_file(name: &str) -> PathBuf {
    let path = scratch(name).join("gemeinsam.plnwrk");
    let mut doc = crate::OpenDoc::sample().unwrap();
    doc.save_as(&path, |_, _| ()).unwrap();
    path
}

/// A plnwrk with `path` open, nobody chosen yet.
fn app_on(path: &std::path::Path) -> Harness<'static, PlnwrkApp> {
    let mut h = harness(PlnwrkApp::default());
    h.state_mut().open_path(path.to_path_buf());
    h.run();
    h
}

/// A plnwrk with `path` open, working as `person` (chosen in "who are you?").
fn working_as(path: &std::path::Path, person: &str) -> Harness<'static, PlnwrkApp> {
    let mut h = app_on(path);
    assert!(
        matches!(h.state().state.dialog, Some(Dialog::WhoAmI { .. })),
        "asked who works here"
    );
    click(&mut h, person);
    assert!(h.state().state.dialog.is_none());
    assert!(h.state().doc().unwrap().shared);
    h
}

/// Renames a task – without drawing a frame, so the shared rhythm sees only the times a test
/// gives it (a frame ticks it with egui's own clock).
fn rename(h: &mut Harness<'static, PlnwrkApp>, from: &str, to: &str) {
    let id = task_named(h.state(), from).unwrap().id;
    h.state_mut().apply(crate::Intent::run(
        "task.update",
        serde_json::json!({"task": id.to_string(), "title": to}),
    ));
}

/// Lets a shared file save itself: a tick long after any frame's clock.
fn saves_itself(h: &mut Harness<'static, PlnwrkApp>, at: f64) {
    h.state_mut().shared_at(at);
    h.state_mut().shared_at(at + 2.0);
    assert!(!h.state().has_unsaved_changes(), "saved by itself");
}

fn titles_on_disk(path: &std::path::Path) -> Vec<String> {
    crate::OpenDoc::open(path, None)
        .unwrap()
        .session
        .workspace()
        .tasks()
        .map(|t| t.title.clone())
        .collect()
}

/// Phase 10a: a file with people asks once who works here; the answer is remembered on this
/// device (also across openings) and shown in the status bar.
#[test]
fn who_works_here_is_asked_once_and_remembered() {
    let path = shared_file("who");
    let mut h = working_as(&path, "Anna Berg");
    let anna = h
        .state()
        .doc()
        .unwrap()
        .session
        .workspace()
        .people()
        .find(|p| p.name == "Anna Berg")
        .unwrap()
        .id;
    assert_eq!(h.state().doc().unwrap().session.actor(), Some(anna));
    assert!(
        h.query_by_label(&format!("{} · Anna Berg", text::t().shared_badge))
            .is_some()
    );
    // Opened again on this device: not asked, still Anna.
    let actors = h.state().state.actors.clone();
    h.state_mut()
        .apply(crate::Intent::File(crate::state::FileAction::Close));
    h.run();
    h.state_mut().state.actors = actors;
    h.state_mut().open_path(path.clone());
    h.run();
    assert!(h.state().state.dialog.is_none(), "not asked again");
    assert_eq!(h.state().doc().unwrap().session.actor(), Some(anna));
    assert!(h.state().doc().unwrap().shared);
}

/// Phase 10a: "only I use this file" – not shared, not asked again, no saving by itself.
#[test]
fn only_me_keeps_the_file_to_myself() {
    let path = shared_file("only-me");
    let mut h = app_on(&path);
    click(&mut h, text::t().only_me);
    let doc = h.state().doc().unwrap();
    assert!(!doc.shared && doc.session.actor().is_none());
    rename(&mut h, "Einbau Wache 11", "Nur meins");
    h.state_mut().shared_at(100.0);
    h.state_mut().shared_at(110.0);
    assert!(h.state().has_unsaved_changes(), "saved only by hand");
    let actors = h.state().state.actors.clone();
    let mut again = app_on(&path);
    again.state_mut().state.actors = actors;
    again
        .state_mut()
        .apply(crate::Intent::File(crate::state::FileAction::Close));
    again.state_mut().open_path(path.clone());
    again.run();
    assert!(again.state().state.dialog.is_none(), "not asked again");
}

/// Phase 10a: someone new adds themself right in the question.
#[test]
fn a_new_person_adds_themself_when_asked() {
    let path = shared_file("new-person");
    let mut h = app_on(&path);
    type_into(&mut h, text::t().new_person, "Carla Meier", true);
    let doc = h.state().doc().unwrap();
    let me = doc
        .session
        .actor()
        .and_then(|id| doc.session.workspace().person(id));
    assert_eq!(me.map(|p| p.name.as_str()), Some("Carla Meier"));
    assert!(doc.shared && h.state().state.dialog.is_none());
}

/// Phase 10a (Tobi): a shared file saves itself 1–2 s after the last change.
#[test]
fn a_shared_file_saves_itself_a_moment_after_a_change() {
    let path = shared_file("autosave");
    let mut h = working_as(&path, "Anna Berg");
    rename(&mut h, "Einbau Wache 11", "Von selbst gespeichert");
    h.state_mut().shared_at(100.0);
    h.state_mut().shared_at(100.5);
    assert!(h.state().has_unsaved_changes(), "not at once");
    h.state_mut().shared_at(101.6);
    assert!(!h.state().has_unsaved_changes());
    assert!(titles_on_disk(&path).contains(&"Von selbst gespeichert".to_owned()));
}

/// Phase 10a: what someone else saved arrives by itself – and is said with their name.
#[test]
fn what_others_saved_arrives_by_itself() {
    let path = shared_file("fetch");
    let mut anna = working_as(&path, "Anna Berg");
    let mut tobias = working_as(&path, "Tobias Wildezahn");
    rename(&mut tobias, "Einbau Wache 11", "Von Tobias");
    saves_itself(&mut tobias, 100.0);
    anna.state_mut().shared_at(105.0);
    anna.run();
    assert!(task_named(anna.state(), "Von Tobias").is_some());
    let said = anna.state().state.message().cloned().unwrap();
    assert_eq!(said.text, text::fetched(1, "Tobias Wildezahn"));
    assert!(!anna.state().has_unsaved_changes());
}

/// Phase 10a (W6): nothing is fetched while something is typed or a question is open – the
/// typed text would meet another value.
#[test]
fn nothing_is_fetched_while_typing_or_asked() {
    let path = shared_file("not-while");
    let mut anna = working_as(&path, "Anna Berg");
    let mut tobias = working_as(&path, "Tobias Wildezahn");
    rename(&mut tobias, "Einbau Wache 11", "Von Tobias");
    saves_itself(&mut tobias, 100.0);
    // Typing in the details.
    let task = task_named(anna.state(), "Schulung Einsatzkräfte").unwrap();
    anna.state_mut()
        .apply(crate::Intent::Go(View::Project(task.project)));
    anna.state_mut().apply(crate::Intent::Select(Some(task.id)));
    anna.run();
    type_into(&mut anna, text::t().description, "Halb getippt", false);
    let id = task.id;
    anna.state_mut().shared_at(105.0);
    assert!(
        task_named(anna.state(), "Von Tobias").is_none(),
        "not while typing"
    );
    // A question open.
    anna.state_mut().state.forms.task = None;
    anna.state_mut()
        .apply(crate::Intent::Open(Dialog::DeleteTask(id)));
    anna.state_mut().shared_at(110.0);
    assert!(
        task_named(anna.state(), "Von Tobias").is_none(),
        "not while asked"
    );
    anna.state_mut().state.close_dialog();
    anna.state_mut().shared_at(115.0);
    assert!(task_named(anna.state(), "Von Tobias").is_some());
}

/// Phase 10a (W6): an inline rename opened but not typed in is closed before fetching – it
/// would show the old title as if typed and set it back.
#[test]
fn an_untouched_rename_is_closed_before_fetching() {
    let path = shared_file("inline");
    let mut anna = working_as(&path, "Anna Berg");
    let mut tobias = working_as(&path, "Tobias Wildezahn");
    let id = task_named(anna.state(), "Einbau Wache 11").unwrap().id;
    anna.state_mut().state.forms.inline = Some((id, "Einbau Wache 11".to_owned()));
    rename(&mut tobias, "Einbau Wache 11", "Von Tobias");
    saves_itself(&mut tobias, 100.0);
    anna.state_mut().shared_at(105.0);
    assert!(anna.state().state.forms.inline.is_none());
    assert!(task_named(anna.state(), "Von Tobias").is_some());
    assert!(!anna.state().has_unsaved_changes(), "nothing set back");
}

/// Phase 10a: both changed the same deadline – the later save wins, says so, and the task gets
/// a comment with the other value (which is no undo step of the one saving).
#[test]
fn a_conflict_is_said_and_noted_on_the_task() {
    let path = shared_file("conflict");
    let mut anna = working_as(&path, "Anna Berg");
    let mut tobias = working_as(&path, "Tobias Wildezahn");
    let id = task_named(anna.state(), "Einbau Wache 11").unwrap().id;
    for (h, due) in [(&mut tobias, "2026-11-21"), (&mut anna, "2026-11-25")] {
        h.state_mut().apply(crate::Intent::run(
            "task.update",
            serde_json::json!({"task": id.to_string(), "due": due}),
        ));
    }
    saves_itself(&mut tobias, 100.0);
    saves_itself(&mut anna, 100.0);
    let said = anna.state().state.message().cloned().unwrap();
    assert!(said.text.contains("Tobias Wildezahn"), "{said:?}");
    let app = anna.state();
    let ws = app.doc().unwrap().session.workspace();
    assert_eq!(
        ws.task(id).unwrap().due.map(|d| d.to_string()).as_deref(),
        Some("2026-11-25")
    );
    let comment = ws
        .comments()
        .find(|c| c.task == id && c.body.contains("21.11.2026"));
    assert!(comment.is_some(), "the other value as a comment");
    // Undo takes back Anna's own step, not the comment.
    anna.state_mut()
        .apply(crate::Intent::run("undo", serde_json::json!({})));
    let app = anna.state();
    let ws = app.doc().unwrap().session.workspace();
    assert!(
        ws.comments()
            .any(|c| c.task == id && c.body.contains("21.11.2026"))
    );
    assert_eq!(
        ws.task(id).unwrap().due.map(|d| d.to_string()).as_deref(),
        Some("2026-11-21")
    );
}

/// Phase 10a (H8): an older plnwrk holds the file while it is open – said at opening.
#[test]
fn an_older_plnwrk_holding_the_file_is_said_at_once() {
    let path = shared_file("older");
    let _held = plnwrk_store::file::lock(&path).unwrap();
    let h = app_on(&path);
    assert!(
        h.query_by_label(text::t().held_elsewhere).is_some()
            || h.state()
                .state
                .message()
                .is_some_and(|m| m.text == text::t().held_elsewhere)
    );
}

/// Phase 10a: the same with ⌘S (saving by hand merges and notes the conflict too).
#[test]
fn saving_by_hand_notes_a_conflict_too() {
    let path = shared_file("conflict-hand");
    let mut anna = working_as(&path, "Anna Berg");
    let mut tobias = working_as(&path, "Tobias Wildezahn");
    let id = task_named(anna.state(), "Einbau Wache 11").unwrap().id;
    for (h, due) in [(&mut tobias, "2026-11-21"), (&mut anna, "2026-11-25")] {
        h.state_mut().apply(crate::Intent::run(
            "task.update",
            serde_json::json!({"task": id.to_string(), "due": due}),
        ));
    }
    saves_itself(&mut tobias, 100.0);
    anna.state_mut()
        .apply(crate::Intent::File(crate::state::FileAction::Save));
    let app = anna.state();
    let ws = app.doc().unwrap().session.workspace();
    assert!(
        ws.comments()
            .any(|c| c.task == id && c.body.contains("21.11.2026"))
    );
    assert!(!app.has_unsaved_changes());
}

/// The people of the sample, shown.
fn people_view() -> Harness<'static, PlnwrkApp> {
    let mut h = harness_with_sample();
    h.state_mut().apply(crate::Intent::Go(View::People));
    h.run();
    h
}

fn person(h: &Harness<'_, PlnwrkApp>, name: &str) -> Option<plnwrk_core::Person> {
    h.state()
        .doc()
        .unwrap()
        .session
        .workspace()
        .people()
        .find(|p| p.name == name)
        .cloned()
}

/// Types into the text field called `label` (by its label or placeholder) – the last one, so
/// a field in a dialog wins over one behind it – and optionally presses Enter.
fn type_in_field(h: &mut Harness<'static, PlnwrkApp>, label: &str, typed: &str, enter: bool) {
    h.query_all_by(|n| {
        n.role() == egui::accesskit::Role::TextInput
            && (n.label().as_deref() == Some(label) || n.placeholder() == Some(label))
    })
    .last()
    .unwrap_or_else(|| panic!("no field {label}"))
    .focus();
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.run();
    if typed.is_empty() {
        h.key_press(Key::Backspace);
    } else {
        type_text_into(h, label, typed);
    }
    h.run();
    if enter {
        h.key_press(Key::Enter);
        h.run();
    }
}

fn type_text_into(h: &Harness<'static, PlnwrkApp>, label: &str, typed: &str) {
    h.query_all_by(|n| {
        n.role() == egui::accesskit::Role::TextInput
            && (n.label().as_deref() == Some(label) || n.placeholder() == Some(label))
    })
    .last()
    .unwrap_or_else(|| panic!("no field {label}"))
    .type_text(typed);
}

fn edit_button(name: &str) -> String {
    format!("{} · {name}", text::t().edit_person)
}

/// Phase 10a (Tobi: „anlegen und natürlich auch ändern“): a person is changed in its dialog –
/// name, initials, role, e-mail; an emptied role is removed.
#[test]
fn a_person_is_changed() {
    let mut h = people_view();
    let anna = person(&h, "Anna Berg").unwrap();
    click(&mut h, &edit_button("Anna Berg"));
    assert!(matches!(
        h.state().state.dialog,
        Some(Dialog::EditPerson { .. })
    ));
    type_in_field(&mut h, text::t().name, "Anna Berg-Kruse", false);
    type_in_field(&mut h, text::t().role, "", false);
    type_in_field(&mut h, text::t().email, "anna@example.org", false);
    click(&mut h, text::t().apply);
    assert!(h.state().state.dialog.is_none());
    let changed = h
        .state()
        .doc()
        .unwrap()
        .session
        .workspace()
        .person(anna.id)
        .unwrap()
        .clone();
    assert_eq!(changed.name, "Anna Berg-Kruse");
    assert_eq!(changed.role, None, "emptied: removed");
    assert_eq!(changed.email.as_deref(), Some("anna@example.org"));
}

/// Phase 10a: the keyboard only – Enter in a field takes the change; unchanged, nothing is
/// recorded.
#[test]
fn a_person_is_changed_with_the_keyboard_and_nothing_unchanged_is_recorded() {
    let mut h = people_view();
    let before = h.state().doc().unwrap().log_len();
    click(&mut h, &edit_button("Anna Berg"));
    click(&mut h, text::t().apply);
    assert!(h.state().state.dialog.is_none());
    assert_eq!(
        h.state().doc().unwrap().log_len(),
        before,
        "nothing changed, nothing recorded"
    );
    click(&mut h, &edit_button("Anna Berg"));
    type_in_field(&mut h, text::t().initials, "ABK", true);
    assert!(h.state().state.dialog.is_none(), "Enter took it");
    assert_eq!(person(&h, "Anna Berg").unwrap().initials, "ABK");
}

/// Phase 10a (W9): the same person twice by mistake – a hint when adding or changing.
#[test]
fn the_same_person_twice_is_pointed_out() {
    let mut h = people_view();
    type_in_field(&mut h, text::t().name, "anna berg", false);
    assert!(
        h.query_by_label(&text::same_person("Anna Berg")).is_some(),
        "adding"
    );
    let mut h = people_view();
    click(&mut h, &edit_button("Tobias Wildezahn"));
    let anna = person(&h, "Anna Berg").unwrap();
    type_in_field(&mut h, text::t().initials, &anna.initials, false);
    assert!(
        h.query_by_label(&text::same_person("Anna Berg")).is_some(),
        "changing"
    );
    // Its own name is no double.
    let mut h = people_view();
    click(&mut h, &edit_button("Anna Berg"));
    assert!(h.query_by_label(&text::same_person("Anna Berg")).is_none());
}

/// Phase 10a: who I am is marked in the list.
#[test]
fn i_am_marked_in_the_list_of_people() {
    let mut h = people_view();
    let anna = person(&h, "Anna Berg").unwrap();
    h.state_mut().apply(crate::Intent::Actor(Some(anna.id)));
    h.run();
    assert!(
        h.query_by_label(&format!("Anna Berg {}", text::t().me_marker))
            .is_some()
    );
}
