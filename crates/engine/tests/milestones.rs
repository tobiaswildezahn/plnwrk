//! What counts as a change of a milestone (Phase 9d): found by a run on a copy.

#[cfg(test)]
mod milestones {
    use plnwrk_core::{EntityKey, FixedClock, SequentialIds};
    use plnwrk_engine::milestones::Field;
    use plnwrk_engine::{Origin, Session};
    use serde_json::{Value, json};

    fn session() -> Session {
        Session::new(
            Box::new(FixedClock::at("2026-10-14").unwrap()),
            Box::new(SequentialIds::default()),
        )
    }

    fn create(s: &mut Session, command: &str, params: Value) -> String {
        let outcome = s.run(command, &params, Origin::User).unwrap();
        match outcome.created.first() {
            Some(EntityKey::Project(id)) => id.to_string(),
            Some(EntityKey::Task(id)) => id.to_string(),
            Some(EntityKey::Person(id)) => id.to_string(),
            other => panic!("{other:?}"),
        }
    }

    /// A project due 31.10. with a task "Vorarbeit" (due 20.10.) and a milestone "Abgabe"
    /// (due 24.10.) that waits for it (milestones are never subtasks).
    fn setup() -> (Session, String, String) {
        let mut s = session();
        let project = create(
            &mut s,
            "project.create",
            json!({"title": "P", "due": "2026-10-31"}),
        );
        let work = create(
            &mut s,
            "task.create",
            json!({"project": project, "title": "Vorarbeit", "due": "2026-10-20"}),
        );
        let milestone = create(
            &mut s,
            "task.create",
            json!({"project": project, "title": "Abgabe", "due": "2026-10-24",
                   "milestone": true}),
        );
        s.run(
            "dependency.add",
            &json!({"from": work, "to": milestone}),
            Origin::User,
        )
        .unwrap();
        (s, work, milestone)
    }

    #[test]
    fn a_new_date_is_found_with_what_follows() {
        let (mut s, _, milestone) = setup();
        let before = s.log().len();
        let changes = s.milestone_changes(
            "task.update",
            &json!({"task": milestone, "due": "2026-10-18"}),
        );
        assert_eq!(s.log().len(), before, "nothing is run for real");
        let [change] = changes.as_slice() else {
            panic!("{changes:?}")
        };
        assert_eq!(change.title, "Abgabe");
        assert!(matches!(change.fields.as_slice(), [Field::Due { .. }]));
        assert_eq!(change.late_predecessors, 1, "Vorarbeit ends after the 18th");
        assert!(!change.after_project_due);
        let later = s.milestone_changes(
            "task.update",
            &json!({"task": milestone, "due": "2026-11-05"}),
        );
        assert!(later[0].after_project_due);
    }

    #[test]
    fn every_way_to_change_a_milestone_is_found() {
        let (mut s, _, milestone) = setup();
        let cases: Vec<(&str, Value)> = vec![
            (
                "task.update",
                json!({"task": milestone, "title": "Abgabe neu"}),
            ),
            (
                "task.update",
                json!({"task": milestone, "milestone": false}),
            ),
            (
                "task.set_status",
                json!({"task": milestone, "status": "done"}),
            ),
            ("task.move", json!({"task": milestone, "status": "done"})),
            ("task.delete", json!({"task": milestone})),
            (
                "task.reschedule",
                json!({"moves": [{"task": milestone, "start": null, "due": "2026-10-25"}]}),
            ),
        ];
        for (command, params) in cases {
            let changes = s.milestone_changes(command, &params);
            assert!(!changes.is_empty(), "{command} {params}");
        }
    }

    #[test]
    fn what_is_not_a_change_of_a_milestone_asks_nothing() {
        let (mut s, work, milestone) = setup();
        let person = create(&mut s, "person.create", json!({"name": "Anna Berg"}));
        let cases: Vec<(&str, Value)> = vec![
            // Ordinary tasks, the milestone's assignee and description, new milestones.
            ("task.update", json!({"task": work, "due": "2026-10-30"})),
            (
                "task.update",
                json!({"task": milestone, "assignee": person}),
            ),
            (
                "task.update",
                json!({"task": milestone, "description": "Mit Anlage"}),
            ),
            (
                "task.update",
                json!({"task": milestone, "due": "2026-10-24"}),
            ),
            (
                "task.create",
                json!({"project": "x", "title": "Neu", "milestone": true}),
            ),
            ("undo", json!({})),
            // Refused commands say why when run; here they ask nothing.
            ("task.update", json!({"task": milestone, "title": ""})),
            ("task.update", json!({"task": "nope", "title": "x"})),
        ];
        for (command, params) in cases {
            let changes = s.milestone_changes(command, &params);
            assert!(changes.is_empty(), "{command} {params}: {changes:?}");
        }
    }
    /// A clock the test moves on, day by day.
    #[derive(Clone)]
    struct Days(std::sync::Arc<std::sync::atomic::AtomicI32>);

    impl Days {
        fn on(&self, day: i32) {
            self.0.store(day, std::sync::atomic::Ordering::SeqCst);
        }
        fn date(&self) -> jiff::civil::Date {
            let day = self.0.load(std::sync::atomic::Ordering::SeqCst);
            jiff::civil::date(2026, 10, 1)
                .checked_add(jiff::Span::new().days(day))
                .unwrap()
        }
    }

    impl plnwrk_core::Clock for Days {
        fn now(&self) -> jiff::Timestamp {
            self.date()
                .to_zoned(jiff::tz::TimeZone::UTC)
                .unwrap()
                .timestamp()
        }
        fn today(&self) -> jiff::civil::Date {
            self.date()
        }
        fn now_local(&self) -> jiff::civil::DateTime {
            self.date().to_datetime(jiff::civil::Time::midnight())
        }
    }

    /// "erreicht am …" (Phase 9d): the last time it became done, from the log.
    #[test]
    fn the_day_a_milestone_was_reached_is_the_last_time_it_became_done() {
        let days = Days(std::sync::Arc::default());
        let mut s = Session::new(Box::new(days.clone()), Box::new(SequentialIds::default()));
        let project = create(&mut s, "project.create", json!({"title": "P"}));
        let milestone = create(
            &mut s,
            "task.create",
            json!({"project": project, "title": "Abgabe", "due": "2026-10-24",
                   "milestone": true}),
        );
        let id: plnwrk_core::TaskId = milestone.parse().unwrap();
        let at = |s: &Session| {
            plnwrk_engine::query::done_at(s.log(), id)
                .map(|t| t.to_zoned(jiff::tz::TimeZone::UTC).date().day())
        };
        let status = |s: &mut Session, status: &str| {
            s.run(
                "task.set_status",
                &json!({"task": milestone, "status": status}),
                Origin::User,
            )
            .unwrap();
        };
        assert_eq!(at(&s), None, "never done");
        days.on(4);
        status(&mut s, "done");
        days.on(6);
        // Another field changes while done: not the day it was reached.
        s.run(
            "task.update",
            &json!({"task": milestone, "title": "Abgabe Bericht"}),
            Origin::User,
        )
        .unwrap();
        assert_eq!(at(&s), Some(5));
        days.on(8);
        status(&mut s, "todo");
        days.on(10);
        status(&mut s, "done");
        assert_eq!(at(&s), Some(11), "the last time");
        // Created done: the day it was created.
        days.on(12);
        let done = create(
            &mut s,
            "task.create",
            json!({"project": project, "title": "Alt", "status": "done", "milestone": true,
                   "due": "2026-10-02"}),
        );
        let done: plnwrk_core::TaskId = done.parse().unwrap();
        let day = plnwrk_engine::query::done_at(s.log(), done)
            .map(|t| t.to_zoned(jiff::tz::TimeZone::UTC).date().day());
        assert_eq!(day, Some(13));
    }
}
