//! Nachreview 9g: what the follow-up review found, reproduced and kept as tests.

use super::*;
use crate::state::{FileAction, Message};

fn said(h: &Harness<'_, PlnwrkApp>) -> Option<Message> {
    h.state().state.message().cloned()
}

/// N1: project form – wrong date, "Übernehmen" (message under the field), then the date put
/// right and the window closed without "Übernehmen": the form is taken, but the stale field
/// message is read as its refusal.
#[test]
fn nr_n1_project_form_put_right_then_closing_is_not_called_refused() {
    let mut h = with_project("Umzug");
    open_project_form(&mut h);
    type_into(&mut h, text::t().due, "31.02.2026", false);
    click(&mut h, text::t().apply);
    assert!(said(&h).is_some_and(|m| m.text.contains("31.02.2026")));
    type_into(&mut h, text::t().due, "30.11.2026", false);
    assert!(!h.state_mut().close_requested());
    h.run();
    let due = projects(h.state()).first().unwrap().due;
    assert_eq!(
        due.map(|d| d.to_string()),
        Some("2026-11-30".to_owned()),
        "taken"
    );
    assert!(
        said(&h).is_none_or(|m| !m.text.contains("31.02.2026")),
        "taken, but said as not taken: {:?}",
        said(&h)
    );
}

/// N1b: the same through a file action (Save on a saved file).
#[test]
fn nr_n1b_project_form_put_right_then_save() {
    let (mut h, _) = saved_sample_with_task("nr-n1b", "Einbau Wache 11");
    h.state_mut().apply(crate::Intent::Select(None));
    h.run();
    open_project_form(&mut h);
    type_into(&mut h, text::t().due, "31.02.2026", false);
    click(&mut h, text::t().apply);
    assert!(said(&h).is_some_and(|m| m.text.contains("31.02.2026")));
    type_into(&mut h, text::t().due, "30.11.2027", false);
    h.state_mut().apply(crate::Intent::File(FileAction::Save));
    h.run();
    assert!(
        said(&h).is_none_or(|m| !m.text.contains("31.02.2026")),
        "saved with the date taken, but: {:?}",
        said(&h)
    );
}

/// N1c: a wrong start under its field in the details, then the title renamed in the list and
/// saved: the rename is taken, yet the stale field message is said again as its refusal.
#[test]
fn nr_n1c_inline_rename_is_not_called_refused_by_a_field_message() {
    let (mut h, _path) = saved_sample_with_task("nr-n1c", "Einbau Wache 11");
    type_into(&mut h, text::t().start, "31.02.2026", true);
    assert!(said(&h).is_some_and(|m| m.text.contains("31.02.2026")));
    double_click(&mut h, "Einbau Wache 11");
    assert!(h.state().state.forms.inline.is_some());
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.run();
    h.get_by(|n| n.role() == egui::accesskit::Role::TextInput && n.is_focused())
        .type_text("Einbau Wache 12");
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::S);
    h.run();
    assert!(task_named(h.state(), "Einbau Wache 12").is_some(), "taken");
    let m = said(&h).unwrap();
    assert_eq!(m.text.matches("31.02.2026").count(), 1, "{m:?}");
}

/// K1: a recovery copy starts failing while the new-project dialog is open; the project is
/// then created – the message meant for the bar is wiped by the dialog's own success, unseen.
#[test]
fn nr_k1_recovery_failure_survives_the_dialogs_success() {
    let dir = scratch("nr-k1");
    let blocked = dir.join("Notkopien");
    std::fs::write(&blocked, b"").unwrap();
    let mut h = harness_with_sample();
    click(&mut h, text::t().new_project);
    h.state_mut().set_autosave(Some(blocked), true, 0.0);
    for _ in 0..200 {
        h.run();
        if said(&h).is_some_and(|m| m.error) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let m = said(&h).unwrap();
    assert!(h.query_by_label(&m.text).is_none(), "not in the dialog");
    type_into(&mut h, text::t().title, "Neu", false);
    click(&mut h, text::t().create);
    assert!(h.state().state.dialog.is_none());
    assert!(
        said(&h).is_some_and(|s| s.text == m.text),
        "never seen: {:?}",
        said(&h)
    );
}

/// NL2: an upload that fails while a dialog is open is the app's word, not the dialog's: not
/// in the dialog, in the bar once it closes.
#[test]
fn nr_an_upload_failing_behind_a_dialog_is_said_in_the_bar() {
    let (mut h, browser) = in_browser();
    click(&mut h, text::t().open_sample);
    click(&mut h, text::t().new_project);
    browser.0.borrow_mut().upload = Some(Err("NotReadableError".to_owned()));
    h.run();
    let m = said(&h).expect("said");
    assert!(m.text.starts_with(text::t().upload_failed), "{m:?}");
    assert!(h.query_by_label(&m.text).is_none(), "not in the dialog");
    type_into(&mut h, text::t().title, "Neu", false);
    click(&mut h, text::t().create);
    assert_eq!(h.query_all_by_label(&m.text).count(), 1, "in the bar");
}

/// NK2: a typed date not taken on the way into another dialog (here "+ Neu") is not said in
/// that dialog as if it were its refusal, but in the bar once it closes.
#[test]
fn nr_a_warning_on_the_way_into_another_dialog_waits_for_the_bar() {
    let (mut h, _) = saved_sample_with_task("nr-nk2", "Einbau Wache 11");
    type_into(&mut h, text::t().start, "31.02.2026", false);
    click(&mut h, text::t().new_project);
    assert!(h.state().state.dialog.is_some());
    let m = said(&h).expect("the typed date is named");
    assert!(m.text.contains("31.02.2026"), "{m:?}");
    assert!(
        h.query_by_label(&m.text).is_none(),
        "not in the new-project dialog"
    );
    click(&mut h, text::t().cancel);
    assert_eq!(h.query_all_by_label(&m.text).count(), 1, "in the bar");
}

/// NL1: the lines of "Heute und überfällig" put the cursor into the task's title too.
#[test]
fn nr_a_line_of_today_and_overdue_puts_the_cursor_into_the_title() {
    let mut h = harness_with_sample();
    click(
        &mut h,
        "Ausschreibung vorbereiten (Digitalfunk Umrüstung Wachen)",
    );
    let focused = h
        .query_by(|n| n.is_focused())
        .and_then(|n| egui_kittest::kittest::NodeT::accesskit_node(&n).value());
    assert_eq!(focused.as_deref(), Some("Ausschreibung vorbereiten"));
}

/// Taken on the way (closing the details) after a wrong try: what the date field said about the
/// wrong try goes – it is not passed on to the bar. (⌘S would prove nothing: "Gespeichert."
/// replaces any message.)
#[test]
fn nr_a_date_taken_on_the_way_takes_back_the_wrong_try() {
    let (mut h, _) = saved_sample_with_task("nr-flush-date", "Einbau Wache 11");
    type_into(&mut h, text::t().start, "31.02.2026", true);
    assert!(said(&h).is_some_and(|m| m.text.contains("31.02.2026")));
    let start = task_named(h.state(), "Einbau Wache 11")
        .unwrap()
        .start
        .unwrap();
    let earlier = start.saturating_sub(jiff::Span::new().days(1));
    type_into(
        &mut h,
        text::t().start,
        &crate::dates::format(earlier),
        false,
    );
    h.state_mut().apply(crate::Intent::Select(None));
    h.run();
    assert_eq!(
        task_named(h.state(), "Einbau Wache 11").unwrap().start,
        Some(earlier)
    );
    assert!(
        said(&h).is_none_or(|m| !m.text.contains("31.02.2026")),
        "{:?}",
        said(&h)
    );
}

/// The same on a milestone card: taken on the way, asked, "Abbrechen" – no stale word about
/// the wrong try in the bar afterwards.
#[test]
fn nr_a_milestone_date_taken_on_the_way_takes_back_the_wrong_try() {
    let mut h = one_milestone();
    click(&mut h, text::t().change_date);
    type_into(&mut h, text::t().milestone_date, "31.02.2026", true);
    assert!(said(&h).is_some_and(|m| m.text.contains("31.02.2026")));
    let later = crate::dates::format(day_from_today(&h, 14).parse().unwrap());
    type_into(&mut h, text::t().milestone_date, &later, false);
    h.state_mut().apply(crate::Intent::File(FileAction::Save));
    h.run();
    assert!(matches!(
        h.state().state.dialog,
        Some(crate::state::Dialog::ConfirmMilestone { .. })
    ));
    click(&mut h, text::t().cancel);
    assert!(
        said(&h).is_none_or(|m| !m.text.contains("31.02.2026")),
        "{:?}",
        said(&h)
    );
}
