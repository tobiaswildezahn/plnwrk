//! Click paths (Phase 9g, plan §9 point 1): the core tasks as a newcomer does them, each with
//! the mouse and again with the keyboard only (STATUS 4.17), with a bound on the steps. Typing
//! text is free; with the mouse every click and drag counts, with the keyboard every key. The
//! bounds are the lengths of today (the plan's bounds for the mouse hold): a path that grows
//! longer – or breaks – fails here; one that gets shorter gets its bound lowered.

use super::*;

/// A walk through the app, counted: with the mouse its clicks and drags (Enter after typing
/// belongs to the typing, as in the plan: "≤ 5 Klicks + Tippen"); with the keyboard every key.
struct Walk {
    harness: Harness<'static, PlnwrkApp>,
    steps: usize,
    keyboard: bool,
}

impl Walk {
    fn with_mouse(harness: Harness<'static, PlnwrkApp>) -> Self {
        Self {
            harness,
            steps: 0,
            keyboard: false,
        }
    }

    fn with_keyboard(harness: Harness<'static, PlnwrkApp>) -> Self {
        Self {
            harness,
            steps: 0,
            keyboard: true,
        }
    }

    // The mouse.

    /// Clicks what carries this label (the first, where it is shown twice).
    fn click(&mut self, label: &str) {
        assert!(!self.keyboard, "a click on a walk with the keyboard");
        self.harness
            .get_all_by_label(label)
            .next()
            .unwrap_or_else(|| panic!("nothing to click called {label:?}"))
            .click();
        self.harness.run();
        self.steps += 1;
    }

    /// Clicks into a field and types (one click; typing is free).
    fn fill(&mut self, label: &str, text: &str) {
        assert!(!self.keyboard, "a click on a walk with the keyboard");
        let node = self
            .harness
            .query_by_label(label)
            .unwrap_or_else(|| self.harness.get_by(|n| n.placeholder() == Some(label)));
        node.click();
        self.harness.run();
        self.steps += 1;
        self.harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
        self.harness.run();
        self.text(text);
    }

    /// Drags from one point to another (one step), slowly enough for egui to see a drag.
    fn drag(&mut self, from: egui::Pos2, to: egui::Pos2) {
        assert!(!self.keyboard, "a drag on a walk with the keyboard");
        drag(&mut self.harness, from, to - from);
        self.steps += 1;
    }

    // The keyboard.

    /// Types text where the cursor is (free).
    fn text(&mut self, text: &str) {
        self.harness.event(egui::Event::Text(text.to_owned()));
        self.harness.run();
    }

    /// Presses a key (one step).
    fn key(&mut self, key: Key) {
        self.harness.key_press(key);
        self.harness.run();
        self.steps += usize::from(self.keyboard);
    }

    /// Presses a shortcut (one step).
    fn shortcut(&mut self, modifiers: Modifiers, key: Key) {
        self.harness.key_press_modifiers(modifiers, key);
        self.harness.run();
        self.steps += usize::from(self.keyboard);
    }

    /// The command palette: ⌘K, what is typed, Enter.
    fn palette(&mut self, query: &str) {
        self.shortcut(Modifiers::COMMAND, Key::K);
        self.text(query);
        self.key(Key::Enter);
    }

    /// The label of what has the keyboard focus.
    fn focused(&self) -> Option<String> {
        let node = self.harness.query_by(|n| n.is_focused())?;
        let node = egui_kittest::kittest::NodeT::accesskit_node(&node);
        node.label()
            .or_else(|| node.placeholder().map(str::to_owned))
            .or_else(|| node.value())
    }

    /// Tab until the focus is on what carries this label (each press a step).
    fn tab_to(&mut self, label: &str) {
        for _ in 0..120 {
            if self.focused().as_deref() == Some(label) {
                return;
            }
            self.key(Key::Tab);
        }
        panic!("Tab never reached {label:?} (at {:?})", self.focused());
    }

    /// The path held its bound.
    fn at_most(self, bound: usize, path: &str) -> Harness<'static, PlnwrkApp> {
        // What the app has to say at the end is in view, once – never only stored (review 9g,
        // L4: a message can stand in the state and nowhere on the screen).
        if let Some(message) = self.harness.state().state.message() {
            assert_eq!(
                self.harness.query_all_by_label(&message.text).count(),
                1,
                "{path}: {message:?} is not in view once"
            );
        }
        assert!(
            self.steps <= bound,
            "{path}: {} steps, at most {bound}",
            self.steps
        );
        self.harness
    }
}

/// The case made by path 1: with its reference and deadline.
fn the_case(harness: &Harness<'_, PlnwrkApp>) {
    let project = projects(harness.state()).into_iter().next().expect("made");
    assert_eq!(project.title, "Anfrage Wache 12");
    assert!(project.is_case());
    assert_eq!(project.reference.as_deref(), Some("SKA 23/1234"));
    assert_eq!(
        project.due.map(|d| d.to_string()).as_deref(),
        Some("2026-11-30")
    );
}

/// Path 1: a case – new file, "Vorgang", title, reference, deadline – in at most 5 clicks.
#[test]
fn path_a_case_with_the_mouse() {
    let mut walk = Walk::with_mouse(harness(PlnwrkApp::default()));
    walk.click(text::t().new_file);
    walk.click(text::t().ribbon.new_case_long);
    walk.text("Anfrage Wache 12");
    walk.fill(text::t().reference, "SKA 23/1234");
    walk.fill(text::t().due, "30.11.2026");
    walk.click(text::t().create);
    the_case(&walk.at_most(5, "a case with the mouse"));
}

/// Path 1 with the keyboard only.
#[test]
fn path_a_case_with_the_keyboard() {
    let mut walk = Walk::with_keyboard(harness(PlnwrkApp::default()));
    walk.palette(text::t().new_file);
    walk.palette(text::t().ribbon.new_case);
    walk.text("Anfrage Wache 12");
    walk.tab_to(text::t().reference);
    walk.text("SKA 23/1234");
    walk.tab_to(text::t().due);
    walk.text("30.11.2026");
    walk.key(Key::Enter);
    the_case(&walk.at_most(9, "a case with the keyboard"));
}

/// The project made by path 2.
fn the_project(harness: &Harness<'_, PlnwrkApp>) {
    let project = projects(harness.state()).into_iter().next().expect("made");
    assert_eq!(project.title, "Umzug Leitstelle");
    assert!(!project.is_case());
}

/// Path 2: a project – new file, "Projekt", title – in at most 4 clicks.
#[test]
fn path_a_project_with_the_mouse() {
    let mut walk = Walk::with_mouse(harness(PlnwrkApp::default()));
    walk.click(text::t().new_file);
    walk.click(text::t().ribbon.new_project_long);
    walk.text("Umzug Leitstelle");
    walk.click(text::t().create);
    the_project(&walk.at_most(3, "a project with the mouse"));
}

/// Path 2 with the keyboard only.
#[test]
fn path_a_project_with_the_keyboard() {
    let mut walk = Walk::with_keyboard(harness(PlnwrkApp::default()));
    walk.palette(text::t().new_file);
    walk.palette(text::t().ribbon.new_project);
    walk.text("Umzug Leitstelle");
    walk.key(Key::Enter);
    the_project(&walk.at_most(5, "a project with the keyboard"));
}

/// The task made by path 3.
fn the_task(harness: &Harness<'_, PlnwrkApp>) {
    let task = task_named(harness.state(), "Kabel bestellen").expect("made");
    assert_eq!(
        task.due.map(|d| d.to_string()).as_deref(),
        Some("2026-11-20")
    );
}

/// Path 3: a task with a deadline in an open project, in at most 4 clicks.
#[test]
fn path_a_task_with_a_deadline_with_the_mouse() {
    let mut walk = Walk::with_mouse(with_project("Umzug"));
    walk.fill(text::t().new_task, "Kabel bestellen");
    walk.key(Key::Enter);
    walk.click("Kabel bestellen");
    walk.fill(text::t().due, "20.11.2026");
    walk.key(Key::Enter);
    the_task(&walk.at_most(3, "a task with a deadline with the mouse"));
}

/// Path 3 with the keyboard only.
#[test]
fn path_a_task_with_a_deadline_with_the_keyboard() {
    let mut walk = Walk::with_keyboard(with_project("Umzug"));
    walk.palette(text::t().new_task);
    walk.text("Kabel bestellen");
    walk.key(Key::Enter);
    walk.palette("Kabel bestellen");
    walk.tab_to(text::t().due);
    walk.text("20.11.2026");
    walk.key(Key::Enter);
    the_task(&walk.at_most(15, "a task with a deadline with the keyboard"));
}

/// Path 4: back to today in the schedule, in one click (after scrolling away).
#[test]
fn path_today_in_the_schedule_with_the_mouse() {
    let mut h = wide_schedule(0);
    h.state_mut().state.schedule_request = None;
    let at = bar(&h, "Jetzt").right_center() + egui::vec2(30.0, 0.0);
    for _ in 0..6 {
        wheel(&mut h, at, Modifiers::SHIFT);
    }
    assert!(!in_view(&h, "Jetzt"), "scrolled away");
    let mut walk = Walk::with_mouse(h);
    walk.click(text::t().today);
    let h = walk.at_most(1, "today with the mouse");
    assert!(in_view(&h, "Jetzt"));
}

/// Path 4 with the keyboard only.
#[test]
fn path_today_in_the_schedule_with_the_keyboard() {
    let mut h = wide_schedule(0);
    h.state_mut().state.schedule_request = None;
    let at = bar(&h, "Jetzt").right_center() + egui::vec2(30.0, 0.0);
    for _ in 0..6 {
        wheel(&mut h, at, Modifiers::SHIFT);
    }
    assert!(!in_view(&h, "Jetzt"), "scrolled away");
    let mut walk = Walk::with_keyboard(h);
    walk.palette(text::t().today);
    let h = walk.at_most(2, "today with the keyboard");
    assert!(in_view(&h, "Jetzt"));
}

/// "Ausschreibung" a week later (start and deadline), "Vergabe" moved along.
fn a_week_later(harness: &Harness<'_, PlnwrkApp>, first: TaskId, second: TaskId) {
    assert_eq!(
        dates_of(harness, first),
        ("2026-10-19".into(), "2026-10-21".into())
    );
    assert!(
        dates_of(harness, second).0.as_str() > "2026-10-21",
        "moved along"
    );
}

/// Path 5: a bar a week later, in one drag.
#[test]
fn path_a_bar_a_week_later_with_the_mouse() {
    let (mut h, first, second) = two_dated_tasks();
    h.state_mut().apply(crate::Intent::run(
        "dependency.add",
        serde_json::json!({"from": first.to_string(), "to": second.to_string()}),
    ));
    click(&mut h, text::t().schedule_tab);
    let mut walk = Walk::with_mouse(h);
    let from = bar(&walk.harness, "Ausschreibung").center();
    walk.drag(from, from + egui::vec2(7.0 * 22.0, 0.0));
    a_week_later(
        &walk.at_most(1, "a bar a week later with the mouse"),
        first,
        second,
    );
}

/// Path 5 with the keyboard only: the dates in the details, each taken by leaving its field
/// with Tab – the deadline first (so the start never lies after it), then the task chosen again
/// for its start. (Shift+Tab out of a changed date loses the focus: STATUS, finding 9g.)
#[test]
fn path_a_bar_a_week_later_with_the_keyboard() {
    let (mut h, first, second) = two_dated_tasks();
    h.state_mut().apply(crate::Intent::run(
        "dependency.add",
        serde_json::json!({"from": first.to_string(), "to": second.to_string()}),
    ));
    click(&mut h, text::t().schedule_tab);
    let mut walk = Walk::with_keyboard(h);
    walk.palette("Ausschreibung");
    walk.tab_to(text::t().due);
    walk.shortcut(Modifiers::COMMAND, Key::A);
    walk.text("21.10.2026");
    walk.key(Key::Tab);
    walk.palette("Ausschreibung");
    walk.tab_to(text::t().start);
    walk.shortcut(Modifiers::COMMAND, Key::A);
    walk.text("19.10.2026");
    walk.key(Key::Tab);
    let h = walk.at_most(24, "a bar a week later with the keyboard");
    assert_eq!(h.state().state.tab, Tab::Schedule, "still in the schedule");
    a_week_later(&h, first, second);
}

/// Path 6: a milestone moved, with the question, in at most 4 clicks.
#[test]
fn path_a_milestone_moved_with_the_mouse() {
    let h = one_milestone();
    let later = day_from_today(&h, 14);
    let mut walk = Walk::with_mouse(h);
    walk.click(text::t().change_date);
    walk.fill(
        text::t().milestone_date,
        &crate::dates::format(later.parse().unwrap()),
    );
    walk.click(text::t().apply);
    walk.click(text::t().change_button);
    let h = walk.at_most(4, "a milestone moved with the mouse");
    assert_eq!(
        task_named(h.state(), "Abnahme").unwrap().due,
        Some(later.parse().unwrap())
    );
}

/// Path 6 with the keyboard only: the milestone by name, its date in the details, the
/// question answered.
#[test]
fn path_a_milestone_moved_with_the_keyboard() {
    let h = one_milestone();
    let later = day_from_today(&h, 14);
    let mut walk = Walk::with_keyboard(h);
    walk.palette("Abnahme");
    walk.tab_to(text::t().due);
    walk.shortcut(Modifiers::COMMAND, Key::A);
    walk.text(&crate::dates::format(later.parse().unwrap()));
    walk.key(Key::Enter);
    walk.tab_to(text::t().change_button);
    walk.key(Key::Enter);
    let h = walk.at_most(15, "a milestone moved with the keyboard");
    assert_eq!(
        task_named(h.state(), "Abnahme").unwrap().due,
        Some(later.parse().unwrap())
    );
}

/// Path 7: a card to "Erledigt" on the board, in one drag.
#[test]
fn path_a_card_done_with_the_mouse() {
    let (mut h, first, _) = two_dated_tasks();
    click(&mut h, text::t().board);
    let mut walk = Walk::with_mouse(h);
    let card = walk.harness.get_by_label("Ausschreibung").rect().center();
    let column = column_heading_node(&walk.harness, TaskStatus::Done)
        .rect()
        .center()
        + egui::vec2(0.0, 60.0);
    walk.drag(card, column);
    let h = walk.at_most(1, "a card done with the mouse");
    assert_eq!(
        h.state()
            .doc()
            .unwrap()
            .session
            .workspace()
            .task(first)
            .unwrap()
            .status,
        TaskStatus::Done
    );
}

/// Path 7 with the keyboard only: the task by name, then its status.
#[test]
fn path_a_card_done_with_the_keyboard() {
    let (mut h, first, _) = two_dated_tasks();
    click(&mut h, text::t().board);
    let mut walk = Walk::with_keyboard(h);
    walk.palette("Ausschreibung");
    walk.palette(text::task_status(TaskStatus::Done));
    let h = walk.at_most(4, "a card done with the keyboard");
    assert_eq!(h.state().state.tab, Tab::Board, "still on the board");
    assert_eq!(
        h.state()
            .doc()
            .unwrap()
            .session
            .workspace()
            .task(first)
            .unwrap()
            .status,
        TaskStatus::Done
    );
}

/// Path 8: a dependency drawn from one bar onto another, in one drag.
#[test]
fn path_a_dependency_with_the_mouse() {
    let (mut h, first, second) = two_dated_tasks();
    click(&mut h, text::t().schedule_tab);
    let mut walk = Walk::with_mouse(h);
    let handle = walk
        .harness
        .get_by_label(&format!("{} · Ausschreibung", text::t().link_handle))
        .rect()
        .center();
    let target = bar(&walk.harness, "Vergabe").center();
    walk.drag(handle, target);
    let h = walk.at_most(1, "a dependency with the mouse");
    assert!(waits_for(&h, first, second));
}

/// Path 8 with the keyboard only: "Wartet auf" in the details.
#[test]
fn path_a_dependency_with_the_keyboard() {
    let (mut h, first, second) = two_dated_tasks();
    click(&mut h, text::t().schedule_tab);
    let mut walk = Walk::with_keyboard(h);
    walk.palette("Vergabe");
    walk.tab_to(text::t().add_predecessor);
    walk.key(Key::Enter);
    walk.tab_to("Ausschreibung");
    walk.key(Key::Enter);
    let h = walk.at_most(24, "a dependency with the keyboard");
    assert_eq!(h.state().state.tab, Tab::Schedule, "still in the schedule");
    assert!(waits_for(&h, first, second));
}
