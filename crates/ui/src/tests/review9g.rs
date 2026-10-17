//! Review 9g: what the independent review found, reproduced and kept as tests.

use super::*;
use crate::state::{Dialog, FileAction, Message};

fn said(h: &Harness<'_, PlnwrkApp>) -> Option<Message> {
    h.state().state.message().cloned()
}

/// R1: a typed date that cannot be taken, then "Schließen" with unsaved changes, "Speichern"
/// in the question: the person must learn that the date was not taken.
#[test]
fn r1_unsaved_save_keeps_the_refusal_of_a_typed_date() {
    let (mut h, _) = saved_sample_with_task("r9g-r1", "Einbau Wache 11");
    // A real change, so closing asks.
    let project = task_named(h.state(), "Einbau Wache 11").unwrap().project;
    h.state_mut().apply(crate::Intent::run(
        "task.create",
        serde_json::json!({"project": project.to_string(), "title": "Neu"}),
    ));
    h.run();
    // Typed, not taken yet (the cursor is still in the field).
    type_into(&mut h, text::t().start, "31.02.2026", false);
    h.state_mut().apply(crate::Intent::File(FileAction::Close));
    h.run();
    assert!(matches!(h.state().state.dialog, Some(Dialog::Unsaved(_))));
    let before = said(&h).expect("the typed date is named");
    assert!(before.text.contains("31.02.2026"), "{before:?}");
    // Said in the question, where it can be seen (behind it, it was lost).
    assert_eq!(h.query_all_by_label(&before.text).count(), 1);
    click(&mut h, text::t().save);
}

/// R2 (review 9g, F2): a milestone date typed in the details, then ⌘K – the date is asked
/// about first, and the palette asked for comes after the answer (it was replaced and lost).
#[test]
fn r2_palette_opened_while_a_milestone_date_is_typed() {
    let mut h = one_milestone();
    let id = task_named(h.state(), "Abnahme").unwrap().id;
    h.state_mut().apply(crate::Intent::Select(Some(id)));
    h.run();
    let later = crate::dates::format(day_from_today(&h, 14).parse().unwrap());
    type_into(&mut h, text::t().due, &later, false);
    h.key_press_modifiers(Modifiers::COMMAND, Key::K);
    h.run();
    assert!(matches!(
        h.state().state.dialog,
        Some(Dialog::ConfirmMilestone { .. })
    ));
    click(&mut h, text::t().change_button);
    assert!(
        matches!(h.state().state.dialog, Some(Dialog::Palette { .. })),
        "the palette asked for is open after the answer"
    );
}

/// R2b: the same with the mouse: "+ Neu" (new project) while a milestone date is typed.
#[test]
fn r2b_new_project_clicked_while_a_milestone_date_is_typed() {
    let mut h = one_milestone();
    let id = task_named(h.state(), "Abnahme").unwrap().id;
    h.state_mut().apply(crate::Intent::Select(Some(id)));
    h.run();
    let later = crate::dates::format(day_from_today(&h, 14).parse().unwrap());
    type_into(&mut h, text::t().due, &later, false);
    click(&mut h, text::t().new_project);
    let what = h.state().state.dialog.as_ref().map(|d| match d {
        Dialog::ConfirmMilestone { .. } => "ConfirmMilestone",
        Dialog::NewProject(_) => "NewProject",
        _ => "other",
    });
    assert_eq!(
        what,
        Some("ConfirmMilestone"),
        "the typed date is asked about first"
    );
    click(&mut h, text::t().change_button);
    let after = h.state().state.dialog.as_ref().map(|d| match d {
        Dialog::NewProject(_) => "NewProject",
        _ => "other",
    });
    assert_eq!(after, Some("NewProject"), "the click on + Neu was lost");
}

/// R3: a wrong date corrected back to the stored one: the field is right, its refusal goes.
#[test]
fn r3_a_field_refusal_goes_when_the_field_is_put_right() {
    let (mut h, _) = saved_sample_with_task("r9g-r3", "Einbau Wache 11");
    let stored = task_named(h.state(), "Einbau Wache 11")
        .unwrap()
        .start
        .unwrap();
    type_into(&mut h, text::t().start, "31.02.2026", true);
    assert!(said(&h).is_some());
    let wrong = said(&h).unwrap();
    type_into(&mut h, text::t().start, &crate::dates::format(stored), true);
    assert_eq!(
        task_named(h.state(), "Einbau Wache 11").unwrap().start,
        Some(stored)
    );
    assert!(said(&h).is_none(), "still said: {:?}", said(&h));
    assert!(h.query_by_label(&wrong.text).is_none());
}

/// R4: the keyboard path "Karte nach Erledigt" starts on the board – does it stay there?
#[test]
fn r4_palette_task_keeps_the_board() {
    let (mut h, _, _) = two_dated_tasks();
    click(&mut h, text::t().board);
    assert_eq!(h.state().state.tab, Tab::Board);
    palette(&mut h, "Ausschreibung");
    assert_eq!(h.state().state.tab, Tab::Board, "thrown into the list");
}

/// R8: a wrong date under its field, then a change elsewhere that succeeds: the field still
/// shows the wrong date, so its message must still stand under it (plan 9.1: "solange es
/// gezeichnet wird").
#[test]
fn r8_a_field_message_stays_while_its_field_shows_the_wrong_date() {
    let (mut h, _) = saved_sample_with_task("r9g-r8", "Einbau Wache 11");
    type_into(&mut h, text::t().start, "31.02.2026", true);
    let m = said(&h).unwrap();
    let other = tasks(h.state())
        .into_iter()
        .find(|t| t.title != "Einbau Wache 11" && !t.milestone && t.parent.is_none())
        .unwrap();
    h.state_mut().apply(crate::Intent::run(
        "task.update",
        serde_json::json!({"task": other.id.to_string(), "title": "Umbenannt"}),
    ));
    h.run();
    let field_shows = h
        .query_all_by(|n| n.value().is_some_and(|v| v.contains("31.02.2026")))
        .count();
    let message_shown = h.query_by_label(&m.text).is_some();
    assert!(
        field_shows == 0 || message_shown,
        "wrong date in the field, no word about it"
    );
}

/// R5: a recovery copy that starts failing while an unrelated dialog is open is said in that
/// dialog like one of its refusals, and goes with it.
#[test]
fn r5_a_failing_recovery_copy_while_a_dialog_is_open() {
    let dir = scratch("r9g-r5");
    let blocked = dir.join("Notkopien");
    std::fs::write(&blocked, b"").unwrap();
    let mut h = harness_with_sample();
    click(&mut h, text::t().new_project);
    assert!(matches!(
        h.state().state.dialog,
        Some(Dialog::NewProject(_))
    ));
    h.state_mut().set_autosave(Some(blocked), true, 0.0);
    for _ in 0..200 {
        h.run();
        if said(&h).is_some_and(|m| m.error) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let m = said(&h).unwrap();
    h.run();
    let in_dialog = h.query_by_label(&m.text).is_some();
    click(&mut h, text::t().cancel);
    let after = said(&h);
    assert!(
        !in_dialog && after.is_some(),
        "said in the project dialog as its refusal ({in_dialog}), then gone ({after:?})"
    );
}

/// The node with the keyboard focus is the title of `task` in the details.
fn title_focused(h: &Harness<'_, PlnwrkApp>, task: &str) -> bool {
    h.query_by(|n| n.is_focused()).is_some_and(|n| {
        egui_kittest::kittest::NodeT::accesskit_node(&n)
            .value()
            .as_deref()
            == Some(task)
    })
}

/// L3, K4: going to a task by the palette or from the overview puts the cursor into its title.
#[test]
fn going_to_a_task_puts_the_cursor_into_its_title() {
    let (mut h, _, _) = two_dated_tasks();
    palette(&mut h, "Vergabe");
    assert!(title_focused(&h, "Vergabe"), "palette");
    let mut h = sample_with_overtaken_milestone();
    click(&mut h, "Inbetriebnahme (Digitalfunk Umrüstung Wachen)");
    assert!(title_focused(&h, "Inbetriebnahme"), "overview");
}

/// L1: the block keeps to the one rule of deadlines – not for a finished project nor a
/// template; a done task keeps its dates and holds back, as in the schedule (STATUS 4.12).
#[test]
fn overtaken_milestones_keep_to_the_rule_of_deadlines() {
    fn shown(h: &Harness<'_, PlnwrkApp>) -> bool {
        h.query_by_label(text::t().overtaken_milestones).is_some()
    }
    fn to_overview(h: &mut Harness<'static, PlnwrkApp>) {
        h.state_mut().apply(crate::Intent::Go(View::Overview));
        h.run();
    }
    for change in [
        serde_json::json!({"status": "done"}),
        serde_json::json!({"template": true}),
    ] {
        let mut h = sample_with_overtaken_milestone();
        assert!(shown(&h));
        let project = task_named(h.state(), "Inbetriebnahme").unwrap().project;
        let mut params = change.clone();
        params["project"] = serde_json::json!(project.to_string());
        h.state_mut()
            .apply(crate::Intent::run("project.update", params));
        to_overview(&mut h);
        assert!(!shown(&h), "{change}");
    }
    let mut h = sample_with_overtaken_milestone();
    let install = task_named(h.state(), "Einbau Wache 11").unwrap().id;
    h.state_mut().apply(crate::Intent::run(
        "task.set_status",
        serde_json::json!({"task": install.to_string(), "status": "done"}),
    ));
    to_overview(&mut h);
    assert!(shown(&h), "a done task keeps its dates and holds back");
}

/// F1 on the second way: closing the window with a typed date that cannot be taken – said in
/// the question about unsaved changes.
#[test]
fn r1b_closing_the_window_says_the_refusal_of_a_typed_date() {
    let (mut h, _) = saved_sample_with_task("r9g-r1b", "Einbau Wache 11");
    let project = task_named(h.state(), "Einbau Wache 11").unwrap().project;
    h.state_mut().apply(crate::Intent::run(
        "task.create",
        serde_json::json!({"project": project.to_string(), "title": "Neu"}),
    ));
    h.run();
    type_into(&mut h, text::t().start, "31.02.2026", false);
    assert!(!h.state_mut().close_requested(), "asks first");
    h.run();
    assert!(matches!(h.state().state.dialog, Some(Dialog::Unsaved(_))));
    let named = said(&h).expect("the typed date is named");
    assert!(named.text.contains("31.02.2026"), "{named:?}");
    assert_eq!(h.query_all_by_label(&named.text).count(), 1);
}

/// K2 for every field with a message of its own: wrong, then right – the message goes with the
/// right value (and does not wander to the bar when the field closes).
#[test]
fn r9_every_field_takes_its_message_back_once_it_reads() {
    let wrong = "31.02.2026";
    // The deadline in the details.
    let (mut h, _) = saved_sample_with_task("r9g-r9", "Einbau Wache 11");
    let due = task_named(h.state(), "Einbau Wache 11")
        .unwrap()
        .due
        .unwrap();
    type_into(&mut h, text::t().due, wrong, true);
    assert!(said(&h).is_some_and(|m| m.text.contains(wrong)));
    type_into(&mut h, text::t().due, &crate::dates::format(due), true);
    assert!(said(&h).is_none(), "deadline: {:?}", said(&h));
    // The start, to another day than stored.
    let start = task_named(h.state(), "Einbau Wache 11")
        .unwrap()
        .start
        .unwrap();
    type_into(&mut h, text::t().start, wrong, true);
    assert!(said(&h).is_some_and(|m| m.text.contains(wrong)));
    let earlier = start.saturating_sub(jiff::Span::new().days(1));
    type_into(
        &mut h,
        text::t().start,
        &crate::dates::format(earlier),
        true,
    );
    assert!(
        said(&h).is_none_or(|m| !m.text.contains(wrong)),
        "start: {:?}",
        said(&h)
    );

    // The date on a milestone card (taken after the question).
    let mut h = one_milestone();
    click(&mut h, text::t().change_date);
    type_into(&mut h, text::t().milestone_date, wrong, true);
    assert!(said(&h).is_some_and(|m| m.text.contains(wrong)));
    let later = crate::dates::format(day_from_today(&h, 14).parse().unwrap());
    type_into(&mut h, text::t().milestone_date, &later, true);
    // Asked; "Abbrechen" – no stale word about the wrong date in the bar afterwards.
    click(&mut h, text::t().cancel);
    assert!(
        said(&h).is_none_or(|m| !m.text.contains(wrong)),
        "milestone card: {:?}",
        said(&h)
    );

    // The project form.
    let mut h = with_project("Umzug");
    open_project_form(&mut h);
    type_into(&mut h, text::t().due, wrong, false);
    click(&mut h, text::t().apply);
    assert!(said(&h).is_some_and(|m| m.text.contains(wrong)));
    type_into(&mut h, text::t().due, "30.11.2026", false);
    click(&mut h, text::t().apply);
    assert!(
        said(&h).is_none_or(|m| !m.text.contains(wrong)),
        "project form: {:?}",
        said(&h)
    );
}
