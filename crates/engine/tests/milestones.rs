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
}
