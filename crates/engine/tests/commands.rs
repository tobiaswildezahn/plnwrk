//! Behaviour of every command, through the public API only.

#[cfg(test)]
mod commands {
    use plnwrk_core::{
        Classification, EntityKey, FixedClock, PersonId, Priority, ProjectId, ProjectStatus, Rule,
        SequentialIds, TaskId, TaskStatus, Workspace,
    };
    use plnwrk_engine::{CommandError, Origin, Session};
    use serde_json::{Value, json};

    struct Harness {
        s: Session,
    }

    impl Harness {
        fn new() -> Self {
            Self {
                s: Session::new(
                    Box::new(FixedClock::at("2026-10-14").unwrap()),
                    Box::new(SequentialIds::default()),
                ),
            }
        }

        fn ok(&mut self, command: &str, params: Value) -> Vec<EntityKey> {
            match self.s.run(command, &params, Origin::User) {
                Ok(outcome) => outcome.created,
                Err(error) => panic!("{command} {params}: {error}"),
            }
        }

        fn err(&mut self, command: &str, params: Value) -> CommandError {
            let before = (self.s.workspace().clone(), self.s.log().len());
            let error = self
                .s
                .run(command, &params, Origin::User)
                .expect_err(command);
            assert_eq!(
                (self.s.workspace().clone(), self.s.log().len()),
                before,
                "a failed {command} must not change anything"
            );
            error
        }

        fn ws(&self) -> &Workspace {
            self.s.workspace()
        }

        fn person(&mut self, name: &str) -> PersonId {
            match self.ok("person.create", json!({ "name": name })).first() {
                Some(EntityKey::Person(id)) => *id,
                other => panic!("{other:?}"),
            }
        }

        fn project(&mut self, params: Value) -> ProjectId {
            match self.ok("project.create", params).first() {
                Some(EntityKey::Project(id)) => *id,
                other => panic!("{other:?}"),
            }
        }

        fn task(&mut self, project: ProjectId, title: &str, extra: Value) -> TaskId {
            let mut params = json!({ "project": project.to_string(), "title": title });
            if let (Some(target), Some(extra)) = (params.as_object_mut(), extra.as_object()) {
                target.extend(extra.clone());
            }
            match self.ok("task.create", params).first() {
                Some(EntityKey::Task(id)) => *id,
                other => panic!("{other:?}"),
            }
        }

        fn undo(&mut self) {
            self.ok("undo", json!({}));
        }
    }

    fn get(h: &Harness, id: TaskId) -> plnwrk_core::Task {
        h.ws().task(id).unwrap().clone()
    }

    fn rule_of(error: &CommandError) -> Option<Rule> {
        match error {
            CommandError::Invalid { violations } => violations.first().map(|v| v.rule),
            _ => None,
        }
    }

    fn is_bad_param(error: &CommandError, name: &str) -> bool {
        matches!(error, CommandError::BadParam { param, .. } if param == name)
    }

    // ---------------------------------------------------------------- projects

    #[test]
    fn project_create_with_defaults() {
        let mut h = Harness::new();
        let id = h.project(json!({"title": "Digitalfunk Umrüstung"}));
        let p = h.ws().project(id).unwrap();
        assert_eq!(p.status, ProjectStatus::Active);
        assert_eq!(p.classification, Classification::Open);
        assert!(!p.template && p.reference.is_none() && p.kind.is_none());
        assert_eq!(p.created.to_string(), "2026-10-14T00:00:00Z");
    }

    #[test]
    fn a_case_is_a_project_with_reference_and_deadline() {
        let mut h = Harness::new();
        let id = h.project(json!({
            "title": "SKA 23/1234 Sirenen", "reference": "23/1234", "kind": "SKA",
            "due": "2026-10-21", "classification": "vs_nfd"
        }));
        let p = h.ws().project(id).unwrap();
        assert_eq!(p.reference.as_deref(), Some("23/1234"));
        assert_eq!(p.kind.as_deref(), Some("SKA"));
        assert_eq!(p.due.map(|d| d.to_string()).as_deref(), Some("2026-10-21"));
        assert_eq!(p.classification, Classification::VsNfd);
    }

    #[test]
    fn project_create_refuses_unknown_owner_and_wrong_dates() {
        let mut h = Harness::new();
        let ghost = PersonId::from_ulid(ulid::Ulid(999));
        let error = h.err(
            "project.create",
            json!({"title": "x", "owner": ghost.to_string()}),
        );
        assert_eq!(rule_of(&error), Some(Rule::MissingReference));
        let error = h.err(
            "project.create",
            json!({"title": "x", "start": "2026-10-10", "due": "2026-10-01"}),
        );
        assert_eq!(rule_of(&error), Some(Rule::DateOrder));
    }

    #[test]
    fn project_update_clears_with_null_and_keeps_missing_fields() {
        let mut h = Harness::new();
        let id = h.project(json!({"title": "A", "reference": "R-1", "kind": "SKA"}));
        h.ok(
            "project.update",
            json!({"project": id.to_string(), "reference": null, "title": "B"}),
        );
        let p = h.ws().project(id).unwrap();
        assert_eq!((p.title.as_str(), p.reference.as_deref()), ("B", None));
        assert_eq!(p.kind.as_deref(), Some("SKA"));
        let error = h.err(
            "project.update",
            json!({"project": id.to_string(), "title": null}),
        );
        assert!(is_bad_param(&error, "title"));
    }

    #[test]
    fn project_update_without_changes_adds_no_log_entry() {
        let mut h = Harness::new();
        let id = h.project(json!({"title": "A"}));
        h.ok("project.update", json!({"project": id.to_string()}));
        assert_eq!(h.s.log().len(), 1);
    }

    #[test]
    fn project_archive_and_undo() {
        let mut h = Harness::new();
        let id = h.project(json!({"title": "A"}));
        h.ok("project.archive", json!({"project": id.to_string()}));
        assert_eq!(h.ws().project(id).unwrap().status, ProjectStatus::Archived);
        h.undo();
        assert_eq!(h.ws().project(id).unwrap().status, ProjectStatus::Active);
    }

    // ---------------------------------------------------------------- tasks

    #[test]
    fn tasks_are_appended_and_can_be_placed_after_another() {
        let mut h = Harness::new();
        let p = h.project(json!({"title": "P"}));
        let a = h.task(p, "a", json!({}));
        let b = h.task(p, "b", json!({}));
        let c = h.task(p, "c", json!({"after": a.to_string()}));
        let rank = |id| h.ws().task(id).unwrap().rank.clone();
        assert!(rank(a) < rank(c) && rank(c) < rank(b));
    }

    #[test]
    fn task_create_sets_all_fields() {
        let mut h = Harness::new();
        let p = h.project(json!({"title": "P"}));
        let who = h.person("Anna Berg");
        let t = h.task(
            p,
            "Antwort entwerfen",
            json!({
                "priority": "high", "status": "in_progress", "assignee": who.to_string(),
                "start": "2026-10-14", "due": "2026-10-16", "estimate_minutes": 90,
                "progress": 20, "tags": ["SKA", "Entwurf", "SKA"], "description": "**fett**"
            }),
        );
        let task = h.ws().task(t).unwrap();
        assert_eq!(task.priority, Priority::High);
        assert_eq!(task.status, TaskStatus::InProgress);
        assert_eq!(task.assignee, Some(who));
        assert_eq!(task.tags, vec!["Entwurf", "SKA"]);
        assert_eq!((task.estimate_minutes, task.progress), (Some(90), 20));
    }

    #[test]
    fn task_create_refuses_unknown_project_and_bad_values() {
        let mut h = Harness::new();
        let ghost = ProjectId::from_ulid(ulid::Ulid(999));
        let error = h.err(
            "task.create",
            json!({"project": ghost.to_string(), "title": "x"}),
        );
        assert!(matches!(error, CommandError::NotFound { .. }));
        let p = h.project(json!({"title": "P"}));
        let error = h.err(
            "task.create",
            json!({"project": p.to_string(), "title": "x", "progress": 101}),
        );
        assert!(is_bad_param(&error, "progress"));
    }

    #[test]
    fn task_move_between_neighbours_with_status_change() {
        let mut h = Harness::new();
        let p = h.project(json!({"title": "P"}));
        let a = h.task(p, "a", json!({}));
        let b = h.task(p, "b", json!({}));
        let c = h.task(p, "c", json!({}));
        h.ok(
            "task.move",
            json!({"task": c.to_string(), "status": "review", "after": a.to_string(), "before": b.to_string()}),
        );
        assert!(get(&h, a).rank < get(&h, c).rank && get(&h, c).rank < get(&h, b).rank);
        assert_eq!(get(&h, c).status, TaskStatus::Review);
        h.ok(
            "task.move",
            json!({"task": a.to_string(), "before": c.to_string()}),
        );
        assert!(get(&h, a).rank < get(&h, c).rank);
        h.ok(
            "task.move",
            json!({"task": a.to_string(), "after": b.to_string()}),
        );
        assert!(get(&h, a).rank > get(&h, b).rank);
    }

    #[test]
    fn task_move_refuses_foreign_or_self_neighbours_and_wrong_order() {
        let mut h = Harness::new();
        let p = h.project(json!({"title": "P"}));
        let q = h.project(json!({"title": "Q"}));
        let a = h.task(p, "a", json!({}));
        let b = h.task(p, "b", json!({}));
        let x = h.task(q, "x", json!({}));
        let error = h.err(
            "task.move",
            json!({"task": a.to_string(), "after": x.to_string()}),
        );
        assert!(is_bad_param(&error, "after"));
        let error = h.err(
            "task.move",
            json!({"task": a.to_string(), "after": a.to_string()}),
        );
        assert!(is_bad_param(&error, "after"));
        let c = h.task(p, "c", json!({}));
        let error = h.err(
            "task.move",
            json!({"task": c.to_string(), "after": b.to_string(), "before": a.to_string()}),
        );
        assert!(is_bad_param(&error, "before"));
    }

    #[test]
    fn reparent_under_own_grandchild_is_a_cycle() {
        let mut h = Harness::new();
        let p = h.project(json!({"title": "P"}));
        let a = h.task(p, "a", json!({}));
        let b = h.task(p, "b", json!({"parent": a.to_string()}));
        let c = h.task(p, "c", json!({"parent": b.to_string()}));
        let error = h.err(
            "task.reparent",
            json!({"task": a.to_string(), "parent": c.to_string()}),
        );
        assert_eq!(rule_of(&error), Some(Rule::Cycle));
        h.ok("task.reparent", json!({"task": c.to_string()}));
        assert_eq!(h.ws().task(c).unwrap().parent, None);
    }

    #[test]
    fn an_inherited_classification_travels_with_the_task() {
        let mut h = Harness::new();
        let p = h.project(json!({"title": "P"}));
        let secret = h.task(p, "geheim", json!({"classification": "vs_nfd"}));
        let child = h.task(p, "Standort", json!({"parent": secret.to_string()}));
        assert_eq!(h.ws().task(child).unwrap().classification, None);
        h.ok("task.reparent", json!({"task": child.to_string()}));
        let task = h.ws().task(child).unwrap().clone();
        assert_eq!(task.classification, Some(Classification::VsNfd));
        assert_eq!(
            h.ws().effective_classification(&task),
            Classification::VsNfd
        );
        // Moving it under another open task keeps it too.
        let open = h.task(p, "offen", json!({}));
        h.ok(
            "task.reparent",
            json!({"task": child.to_string(), "parent": open.to_string()}),
        );
        assert_eq!(
            h.ws().task(child).unwrap().classification,
            Some(Classification::VsNfd)
        );
    }

    #[test]
    fn reparent_into_other_project_is_refused() {
        let mut h = Harness::new();
        let p = h.project(json!({"title": "P"}));
        let q = h.project(json!({"title": "Q"}));
        let a = h.task(p, "a", json!({}));
        let x = h.task(q, "x", json!({}));
        let error = h.err(
            "task.reparent",
            json!({"task": a.to_string(), "parent": x.to_string()}),
        );
        assert_eq!(rule_of(&error), Some(Rule::CrossProject));
    }

    #[test]
    fn assign_status_progress() {
        let mut h = Harness::new();
        let p = h.project(json!({"title": "P"}));
        let a = h.task(p, "a", json!({}));
        let who = h.person("Anna");
        h.ok(
            "task.assign",
            json!({"task": a.to_string(), "assignee": who.to_string()}),
        );
        assert_eq!(h.ws().task(a).unwrap().assignee, Some(who));
        h.ok("task.assign", json!({"task": a.to_string()}));
        assert_eq!(h.ws().task(a).unwrap().assignee, None);
        let ghost = PersonId::from_ulid(ulid::Ulid(999));
        let error = h.err(
            "task.assign",
            json!({"task": a.to_string(), "assignee": ghost.to_string()}),
        );
        assert_eq!(rule_of(&error), Some(Rule::MissingReference));
        h.ok(
            "task.set_status",
            json!({"task": a.to_string(), "status": "done"}),
        );
        assert_eq!(h.ws().task(a).unwrap().status, TaskStatus::Done);
        h.ok(
            "task.set_progress",
            json!({"task": a.to_string(), "progress": 60}),
        );
        assert_eq!(h.ws().task(a).unwrap().progress, 60);
        let error = h.err(
            "task.set_progress",
            json!({"task": a.to_string(), "progress": 101}),
        );
        assert!(is_bad_param(&error, "progress"));
    }

    #[test]
    fn task_update_patches_fields() {
        let mut h = Harness::new();
        let p = h.project(json!({"title": "P"}));
        let a = h.task(
            p,
            "a",
            json!({"due": "2026-11-01", "tags": ["x"], "description": "d"}),
        );
        h.ok(
            "task.update",
            json!({"task": a.to_string(), "due": null, "tags": null, "description": null, "priority": "critical"}),
        );
        let task = h.ws().task(a).unwrap();
        assert_eq!(task.due, None);
        assert!(task.tags.is_empty() && task.description.is_empty());
        assert_eq!(task.priority, Priority::Critical);
    }

    #[test]
    fn task_classification_below_project_is_refused() {
        let mut h = Harness::new();
        let p = h.project(json!({"title": "P", "classification": "vs_nfd"}));
        let error = h.err(
            "task.create",
            json!({"project": p.to_string(), "title": "x", "classification": "open"}),
        );
        assert_eq!(rule_of(&error), Some(Rule::Classification));
    }

    #[test]
    fn delete_takes_subtasks_dependencies_comments_and_one_undo_restores_all() {
        let mut h = Harness::new();
        let p = h.project(json!({"title": "P"}));
        let a = h.task(p, "a", json!({}));
        let a1 = h.task(p, "a1", json!({"parent": a.to_string()}));
        let a2 = h.task(p, "a2", json!({"parent": a.to_string()}));
        let b = h.task(p, "b", json!({}));
        h.ok(
            "dependency.add",
            json!({"from": a1.to_string(), "to": b.to_string()}),
        );
        h.ok(
            "comment.add",
            json!({"task": a.to_string(), "body": "eins"}),
        );
        h.ok(
            "comment.add",
            json!({"task": a2.to_string(), "body": "zwei"}),
        );
        let before = h.ws().clone();
        h.ok("task.delete", json!({"task": a.to_string()}));
        assert!(h.ws().task(a).is_none() && h.ws().task(a1).is_none() && h.ws().task(a2).is_none());
        assert_eq!(h.ws().dependencies().count(), 0);
        assert_eq!(h.ws().comments().count(), 0);
        assert!(h.ws().task(b).is_some());
        h.undo();
        assert_eq!(h.ws(), &before);
    }

    // ---------------------------------------------------------------- milestones, dependencies, people, comments

    #[test]
    fn milestones_are_top_level_tasks_with_a_deadline() {
        let mut h = Harness::new();
        let p = h.project(json!({"title": "P"}));
        let m = h.task(
            p,
            "Abgabe",
            json!({"milestone": true, "due": "2026-11-01T14:30"}),
        );
        let task = h.ws().task(m).unwrap();
        assert!(task.milestone);
        assert_eq!(task.due.unwrap().to_string(), "2026-11-01");
        assert_eq!(task.due_time.unwrap().to_string(), "14:30:00");
        // A date without a time clears the time; null clears both.
        h.ok(
            "task.update",
            json!({"task": m.to_string(), "due": "2026-11-02"}),
        );
        assert_eq!(h.ws().task(m).unwrap().due_time, None);
        h.ok("task.update", json!({"task": m.to_string(), "due": null}));
        assert_eq!(h.ws().task(m).unwrap().due, None);
        // A milestone is never a subtask and never has subtasks.
        let error = h.err(
            "task.create",
            json!({"project": p.to_string(), "title": "Teil", "parent": m.to_string()}),
        );
        assert_eq!(rule_of(&error), Some(Rule::Milestone));
        let parent = h.task(p, "Oben", json!({}));
        h.task(p, "Unten", json!({"parent": parent.to_string()}));
        let error = h.err(
            "task.update",
            json!({"task": parent.to_string(), "milestone": true}),
        );
        assert_eq!(rule_of(&error), Some(Rule::Milestone));
        let error = h.err(
            "task.reparent",
            json!({"task": m.to_string(), "parent": parent.to_string()}),
        );
        assert_eq!(rule_of(&error), Some(Rule::Milestone));
        h.ok(
            "task.update",
            json!({"task": m.to_string(), "milestone": false}),
        );
        assert!(!h.ws().task(m).unwrap().milestone);
    }

    #[test]
    fn deadlines_with_a_time_are_checked() {
        let mut h = Harness::new();
        let p = h.project(json!({"title": "P", "due": "2026-12-24T14:00"}));
        assert_eq!(
            h.ws().project(p).unwrap().due_time.unwrap().to_string(),
            "14:00:00"
        );
        for bad in [
            "2026-12-24T24:00",
            "2026-12-24T9:00",
            "2026-12-24 14:00",
            "2026-12-24T14:00:00",
        ] {
            let error = h.err(
                "project.update",
                json!({"project": p.to_string(), "due": bad}),
            );
            assert!(is_bad_param(&error, "due"), "{bad}: {error}");
        }
    }

    #[test]
    fn retired_milestone_commands_refuse_to_run() {
        let mut h = Harness::new();
        let p = h.project(json!({"title": "P"}));
        let error = h.err(
            "milestone.create",
            json!({"project": p.to_string(), "title": "Abgabe", "date": "2026-11-01"}),
        );
        assert!(matches!(error, CommandError::Disabled { .. }), "{error}");
        let error = h.err("workspace.upgrade", json!({}));
        assert!(
            matches!(error, CommandError::Disabled { .. }),
            "nothing to upgrade"
        );
    }

    #[test]
    fn reschedule_moves_several_tasks_in_one_undo_step() {
        let mut h = Harness::new();
        let p = h.project(json!({"title": "Lage"}));
        let a = h.task(
            p,
            "A",
            json!({"start": "2026-10-12", "due": "2026-10-14T14:00"}),
        );
        let b = h.task(p, "B", json!({"start": "2026-10-15", "due": "2026-10-16"}));
        let before = (get(&h, a), get(&h, b));
        h.ok(
            "task.reschedule",
            json!({"moves": [
                {"task": a.to_string(), "start": "2026-10-13", "due": "2026-10-15"},
                {"task": b.to_string(), "start": "2026-10-16", "due": "2026-10-19"},
            ]}),
        );
        let (ta, tb) = (get(&h, a), get(&h, b));
        assert_eq!(
            ta.start.map(|d| d.to_string()).as_deref(),
            Some("2026-10-13")
        );
        assert_eq!(ta.due.map(|d| d.to_string()).as_deref(), Some("2026-10-15"));
        assert_eq!(
            ta.due_time.map(|t| t.to_string()).as_deref(),
            Some("14:00:00"),
            "the time of day stays"
        );
        assert_eq!(tb.due.map(|d| d.to_string()).as_deref(), Some("2026-10-19"));
        h.undo();
        assert_eq!(
            (get(&h, a), get(&h, b)),
            before,
            "one step back restores both"
        );
    }

    #[test]
    fn reschedule_clears_with_null_and_refuses_what_does_not_fit() {
        let mut h = Harness::new();
        let p = h.project(json!({"title": "Lage"}));
        let a = h.task(
            p,
            "A",
            json!({"start": "2026-10-12", "due": "2026-10-14T14:00"}),
        );
        let b = h.task(p, "B", json!({}));
        h.ok(
            "task.reschedule",
            json!({"moves": [{"task": a.to_string(), "start": null, "due": null}]}),
        );
        let ta = get(&h, a);
        assert_eq!((ta.start, ta.due, ta.due_time), (None, None, None));
        // A time of day can be set and cleared with the dates.
        h.ok(
            "task.reschedule",
            json!({"moves": [{"task": a.to_string(), "start": "2026-10-12", "due": "2026-10-13", "due_time": "09:30"}]}),
        );
        assert_eq!(
            get(&h, a).due_time.map(|t| t.to_string()).as_deref(),
            Some("09:30:00")
        );
        h.ok(
            "task.reschedule",
            json!({"moves": [{"task": a.to_string(), "start": "2026-10-12", "due": "2026-10-13", "due_time": null}]}),
        );
        assert_eq!(get(&h, a).due_time, None);
        h.ok(
            "task.reschedule",
            json!({"moves": [{"task": a.to_string(), "start": null, "due": null}]}),
        );
        // Due before start: nothing changes, also not the other task.
        let error = h.err(
            "task.reschedule",
            json!({"moves": [
                {"task": b.to_string(), "start": "2026-10-12", "due": "2026-10-12"},
                {"task": a.to_string(), "start": "2026-10-14", "due": "2026-10-13"},
            ]}),
        );
        assert_eq!(rule_of(&error), Some(Rule::DateOrder));
        // Unknown task, the same task twice, a missing date, no moves.
        let ghost = TaskId::from(ulid::Ulid(99));
        assert!(matches!(
            h.err(
                "task.reschedule",
                json!({"moves": [{"task": ghost.to_string(), "start": null, "due": null}]}),
            ),
            CommandError::NotFound { .. }
        ));
        let twice = json!({"moves": [
            {"task": a.to_string(), "start": null, "due": null},
            {"task": a.to_string(), "start": null, "due": null},
        ]});
        assert!(is_bad_param(&h.err("task.reschedule", twice), "moves"));
        let late = json!({"moves": [{"task": b.to_string(), "start": "2026-10-12", "due": "2026-10-13", "due_time": "25:00"}]});
        assert!(is_bad_param(&h.err("task.reschedule", late), "moves"));
        let missing = json!({"moves": [{"task": a.to_string(), "start": null}]});
        assert!(is_bad_param(&h.err("task.reschedule", missing), "moves"));
        assert!(is_bad_param(
            &h.err("task.reschedule", json!({"moves": []})),
            "moves"
        ));
    }

    #[test]
    fn dependency_rules() {
        let mut h = Harness::new();
        let p = h.project(json!({"title": "P"}));
        let a = h.task(p, "a", json!({}));
        let b = h.task(p, "b", json!({}));
        let c = h.task(p, "c", json!({}));
        h.ok(
            "dependency.add",
            json!({"from": a.to_string(), "to": b.to_string()}),
        );
        let created = h.ok(
            "dependency.add",
            json!({"from": b.to_string(), "to": c.to_string(), "kind": "start_to_start", "lag_days": -2}),
        );
        let error = h.err(
            "dependency.add",
            json!({"from": c.to_string(), "to": a.to_string()}),
        );
        assert_eq!(rule_of(&error), Some(Rule::Cycle));
        let error = h.err(
            "dependency.add",
            json!({"from": a.to_string(), "to": a.to_string()}),
        );
        assert_eq!(rule_of(&error), Some(Rule::SelfReference));
        let error = h.err(
            "dependency.add",
            json!({"from": a.to_string(), "to": b.to_string()}),
        );
        assert_eq!(rule_of(&error), Some(Rule::Duplicate));
        let error = h.err(
            "dependency.add",
            json!({"from": a.to_string(), "to": c.to_string(), "lag_days": 400}),
        );
        assert!(is_bad_param(&error, "lag_days"));
        let Some(EntityKey::Dependency(d)) = created.first().copied() else {
            panic!("{created:?}");
        };
        h.ok("dependency.remove", json!({"dependency": d.to_string()}));
        let error = h.err("dependency.remove", json!({"dependency": d.to_string()}));
        assert!(matches!(error, CommandError::NotFound { .. }));
    }

    #[test]
    fn comments_are_signed_by_the_actor() {
        let mut h = Harness::new();
        let p = h.project(json!({"title": "P"}));
        let a = h.task(p, "a", json!({}));
        let me = h.person("Tobias Wildezahn");
        h.s.set_actor(Some(me));
        h.ok(
            "comment.add",
            json!({"task": a.to_string(), "body": "Erledigt bis Freitag."}),
        );
        let comment = h.ws().comments().next().unwrap();
        assert_eq!(comment.author, Some(me));
        assert_eq!(comment.created.to_string(), "2026-10-14T00:00:00Z");
        assert_eq!(h.s.log().last().unwrap().actor, Some(me));
        let error = h.err("comment.add", json!({"task": a.to_string(), "body": "  "}));
        assert_eq!(rule_of(&error), Some(Rule::Text));
    }

    #[test]
    fn person_update_clears_optional_fields() {
        let mut h = Harness::new();
        let id = h.person("Anna Berg");
        h.ok(
            "person.update",
            json!({"person": id.to_string(), "role": "Leitung", "email": "a@b.de"}),
        );
        h.ok(
            "person.update",
            json!({"person": id.to_string(), "role": null}),
        );
        let person = h.ws().person(id).unwrap();
        assert_eq!(
            (person.role.as_deref(), person.email.as_deref()),
            (None, Some("a@b.de"))
        );
    }

    // ---------------------------------------------------------------- templates

    fn ska_template(h: &mut Harness) -> (ProjectId, [TaskId; 4]) {
        let template = h.project(json!({
            "title": "SKA beantworten", "kind": "SKA", "template": true,
            "start": "2026-01-01", "due": "2026-01-08", "classification": "open",
            "description": "Ablauf für Schriftliche Kleine Anfragen"
        }));
        let eingang = h.task(
            template,
            "Eingang erfassen",
            json!({"start": "2026-01-01", "due": "2026-01-01"}),
        );
        let zuarbeit = h.task(
            template,
            "Zuarbeit anfordern",
            json!({"start": "2026-01-02", "due": "2026-01-05"}),
        );
        let teil = h.task(
            template,
            "Fachamt",
            json!({"parent": zuarbeit.to_string(), "due": "2026-01-04"}),
        );
        let entwurf = h.task(template, "Antwort entwerfen", json!({"due": "2026-01-07"}));
        h.ok(
            "dependency.add",
            json!({"from": zuarbeit.to_string(), "to": entwurf.to_string()}),
        );
        h.task(
            template,
            "Abgabe Senatskanzlei",
            json!({"milestone": true, "due": "2026-01-08T12:00"}),
        );
        (template, [eingang, zuarbeit, teil, entwurf])
    }

    #[test]
    fn instantiate_copies_structure_and_shifts_dates() {
        let mut h = Harness::new();
        let (template, _) = ska_template(&mut h);
        let template_before = h.ws().tasks_of(template).cloned().collect::<Vec<_>>();
        let workspace_before = h.ws().clone();
        let created = h.ok(
            "project.instantiate",
            json!({"template": template.to_string(), "title": "SKA 23/999", "start": "2026-11-02", "reference": "23/999"}),
        );
        let Some(EntityKey::Project(case)) = created.first().copied() else {
            panic!("{created:?}");
        };
        let project = h.ws().project(case).unwrap();
        assert!(!project.template);
        assert_eq!(project.kind.as_deref(), Some("SKA"));
        assert_eq!(project.reference.as_deref(), Some("23/999"));
        assert_eq!(project.due.unwrap().to_string(), "2026-11-09");
        let tasks: Vec<_> = h.ws().tasks_of(case).cloned().collect();
        assert_eq!(tasks.len(), 5);
        let by_title = |t: &str| tasks.iter().find(|x| x.title == t).unwrap();
        assert_eq!(
            by_title("Eingang erfassen").start.unwrap().to_string(),
            "2026-11-02"
        );
        assert_eq!(
            by_title("Antwort entwerfen").due.unwrap().to_string(),
            "2026-11-08"
        );
        assert_eq!(
            by_title("Fachamt").parent,
            Some(by_title("Zuarbeit anfordern").id)
        );
        let deps: Vec<_> = h
            .ws()
            .dependencies()
            .filter(|d| tasks.iter().any(|t| t.id == d.from))
            .collect();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].from, by_title("Zuarbeit anfordern").id);
        assert_eq!(deps[0].to, by_title("Antwort entwerfen").id);
        let milestone = by_title("Abgabe Senatskanzlei");
        assert!(milestone.milestone);
        assert_eq!(milestone.due.unwrap().to_string(), "2026-11-09");
        assert_eq!(milestone.due_time.unwrap().to_string(), "12:00:00");
        assert_eq!(
            h.ws().tasks_of(template).cloned().collect::<Vec<_>>(),
            template_before
        );
        for dependency in workspace_before.dependencies() {
            assert_eq!(h.ws().dependency(dependency.id), Some(dependency));
        }
        h.undo();
        assert_eq!(h.ws(), &workspace_before);
    }

    #[test]
    fn instantiate_refuses_dated_templates_without_start_but_takes_undated_ones() {
        let mut h = Harness::new();
        let dated = h.project(json!({"title": "T", "template": true}));
        h.task(dated, "a", json!({"due": "2026-01-05"}));
        let error = h.err(
            "project.instantiate",
            json!({"template": dated.to_string(), "title": "Neu", "start": "2026-11-02"}),
        );
        assert!(is_bad_param(&error, "template"));
        let undated = h.project(json!({"title": "U", "template": true}));
        h.task(undated, "a", json!({}));
        let created = h.ok(
            "project.instantiate",
            json!({"template": undated.to_string(), "title": "Neu", "start": "2026-11-02", "due": "2026-11-30"}),
        );
        let Some(EntityKey::Project(case)) = created.first().copied() else {
            panic!("{created:?}");
        };
        assert_eq!(h.ws().tasks_of(case).count(), 1);
        assert_eq!(
            h.ws().project(case).unwrap().due.unwrap().to_string(),
            "2026-11-30"
        );
    }

    #[test]
    fn instantiate_only_from_templates() {
        let mut h = Harness::new();
        let normal = h.project(json!({"title": "kein Template"}));
        let error = h.err(
            "project.instantiate",
            json!({"template": normal.to_string(), "title": "x", "start": "2026-11-02"}),
        );
        assert!(is_bad_param(&error, "template"));
    }

    #[test]
    fn instantiate_moves_dates_backwards_too() {
        let mut h = Harness::new();
        let (template, _) = ska_template(&mut h);
        let created = h.ok(
            "project.instantiate",
            json!({"template": template.to_string(), "title": "Früher", "start": "2025-12-25"}),
        );
        let Some(EntityKey::Project(case)) = created.first().copied() else {
            panic!("{created:?}");
        };
        assert_eq!(
            h.ws().project(case).unwrap().due.unwrap().to_string(),
            "2026-01-01"
        );
        let draft = h
            .ws()
            .tasks_of(case)
            .find(|t| t.title == "Antwort entwerfen")
            .unwrap();
        assert_eq!(draft.due.unwrap().to_string(), "2025-12-31");
    }

    #[test]
    fn instantiate_refuses_dates_that_leave_the_calendar() {
        // Each case lets exactly one kind of date overflow.
        let mut h = Harness::new();
        let project_due = h.project(json!({
            "title": "Frist", "template": true, "start": "2026-01-01", "due": "9999-12-31"
        }));
        let task_due =
            h.project(json!({"title": "Aufgabe", "template": true, "start": "2026-01-01"}));
        h.task(task_due, "spät", json!({"due": "9999-12-30"}));
        let task_start =
            h.project(json!({"title": "Beginn", "template": true, "start": "2026-01-01"}));
        h.task(task_start, "spät", json!({"start": "9999-12-30"}));
        let milestone =
            h.project(json!({"title": "Meilenstein", "template": true, "start": "2026-01-01"}));
        h.task(
            milestone,
            "Ende",
            json!({"milestone": true, "due": "9999-12-29"}),
        );
        for template in [project_due, task_due, task_start, milestone] {
            let error = h.err(
                "project.instantiate",
                json!({"template": template.to_string(), "title": "x", "start": "2026-11-02"}),
            );
            assert!(is_bad_param(&error, "start"), "{error}");
        }
    }

    #[test]
    fn two_thousand_tasks_in_one_project_keep_short_ranks() {
        let mut h = Harness::new();
        let p = h.project(json!({"title": "Groß"}));
        for n in 0..2_000 {
            h.task(p, &format!("Aufgabe {n}"), json!({}));
        }
        let mut ranks: Vec<_> = h.ws().tasks_of(p).map(|t| t.rank.clone()).collect();
        assert!(ranks.iter().all(|r| r.as_str().len() <= 4));
        let count = ranks.len();
        ranks.sort();
        ranks.dedup();
        assert_eq!(ranks.len(), count);
    }

    fn order(h: &Harness, p: ProjectId) -> Vec<TaskId> {
        let mut tasks: Vec<_> = h.ws().tasks_of(p).cloned().collect();
        tasks.sort_by(|a, b| a.rank.cmp(&b.rank));
        tasks.into_iter().map(|t| t.id).collect()
    }

    /// Whether the last change re-ranked any task other than `except`.
    fn spread_happened(before: &Workspace, after: &Workspace, except: TaskId) -> bool {
        after
            .tasks()
            .filter(|t| t.id != except)
            .any(|t| before.task(t.id).is_some_and(|b| b.rank != t.rank))
    }

    #[test]
    fn a_crowded_gap_is_spread_out_when_creating_and_one_undo_restores_it() {
        let mut h = Harness::new();
        let p = h.project(json!({"title": "P"}));
        let first = h.task(p, "first", json!({}));
        let second = h.task(p, "second", json!({}));
        let mut expected = vec![first, second];
        let mut spreads = 0;
        // Always insert right after `first`: the gap is used up after about 1 500 inserts.
        for n in 0..1_600 {
            let before = h.ws().clone();
            let new = h.task(p, &format!("n{n}"), json!({"after": first.to_string()}));
            expected.insert(1, new);
            if spread_happened(&before, h.ws(), new) {
                spreads += 1;
                assert_eq!(order(&h, p), expected, "order after spreading");
                let after = h.ws().clone();
                h.undo();
                assert_eq!(h.ws(), &before, "one undo restores the old ranks");
                h.ok("redo", json!({}));
                assert_eq!(h.ws(), &after);
            }
        }
        assert_eq!(spreads, 1);
        assert_eq!(order(&h, p), expected);
        let max = h
            .ws()
            .tasks_of(p)
            .map(|t| t.rank.as_str().len())
            .max()
            .unwrap();
        assert!(max <= 40, "ranks were not spread out: {max} characters");
    }

    #[test]
    fn a_crowded_gap_is_spread_out_when_moving_and_one_undo_restores_it() {
        let mut h = Harness::new();
        let p = h.project(json!({"title": "P"}));
        let first = h.task(p, "first", json!({}));
        let x = h.task(p, "x", json!({}));
        let y = h.task(p, "y", json!({}));
        let last = h.task(p, "last", json!({}));
        let mut spreads = 0;
        // Alternately move x and y right after `first`: the gap behind `first` keeps shrinking.
        for n in 0..1_600 {
            let (moved, other) = if n % 2 == 0 { (y, x) } else { (x, y) };
            let before = h.ws().clone();
            h.ok(
                "task.move",
                json!({"task": moved.to_string(), "after": first.to_string()}),
            );
            assert_eq!(order(&h, p), vec![first, moved, other, last]);
            if spread_happened(&before, h.ws(), moved) {
                spreads += 1;
                h.undo();
                assert_eq!(h.ws(), &before);
                h.ok("redo", json!({}));
            }
        }
        assert!(spreads >= 1);
    }

    #[test]
    fn create_refuses_a_neighbour_from_another_project() {
        let mut h = Harness::new();
        let p = h.project(json!({"title": "P"}));
        let q = h.project(json!({"title": "Q"}));
        let x = h.task(q, "x", json!({}));
        let error = h.err(
            "task.create",
            json!({"project": p.to_string(), "title": "a", "after": x.to_string()}),
        );
        assert!(is_bad_param(&error, "after"));
    }

    #[test]
    fn renaming_a_person_updates_derived_initials_only() {
        let mut h = Harness::new();
        let a = h.person("Anna Berg");
        h.ok(
            "person.update",
            json!({"person": a.to_string(), "name": "Anna Clausen"}),
        );
        assert_eq!(h.ws().person(a).unwrap().initials, "AC");
        h.ok(
            "person.update",
            json!({"person": a.to_string(), "initials": "ANC"}),
        );
        h.ok(
            "person.update",
            json!({"person": a.to_string(), "name": "Anna Dorn"}),
        );
        assert_eq!(h.ws().person(a).unwrap().initials, "ANC");
    }

    #[test]
    fn subtask_classification_below_parent_is_refused() {
        let mut h = Harness::new();
        let p = h.project(json!({"title": "P"}));
        let a = h.task(p, "geheim", json!({"classification": "vs_nfd"}));
        let error = h.err(
            "task.create",
            json!({"project": p.to_string(), "title": "offen", "parent": a.to_string(), "classification": "open"}),
        );
        assert_eq!(rule_of(&error), Some(Rule::Classification));
        let b = h.task(p, "erbt", json!({"parent": a.to_string()}));
        let task = h.ws().task(b).unwrap().clone();
        assert_eq!(
            h.ws().effective_classification(&task),
            Classification::VsNfd
        );
    }
}
