//! Behaviour of every command, through the public API only.

#[cfg(test)]
mod commands {
    use plnwrk_core::{
        Classification, EntityKey, FixedClock, MilestoneStatus, PersonId, Priority, ProjectId,
        ProjectStatus, Rule, SequentialIds, TaskId, TaskStatus, Workspace,
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
    fn milestone_lifecycle() {
        let mut h = Harness::new();
        let p = h.project(json!({"title": "P"}));
        let created = h.ok(
            "milestone.create",
            json!({"project": p.to_string(), "title": "Abgabe", "date": "2026-11-01"}),
        );
        let Some(EntityKey::Milestone(m)) = created.first().copied() else {
            panic!("{created:?}");
        };
        h.ok(
            "milestone.update",
            json!({"milestone": m.to_string(), "date": "2026-11-02"}),
        );
        h.ok("milestone.reach", json!({"milestone": m.to_string()}));
        let milestone = h.ws().milestone(m).unwrap();
        assert_eq!(milestone.status, MilestoneStatus::Reached);
        assert_eq!(milestone.date.to_string(), "2026-11-02");
        h.ok("milestone.delete", json!({"milestone": m.to_string()}));
        assert!(h.ws().milestone(m).is_none());
        h.undo();
        assert!(h.ws().milestone(m).is_some());
        let ghost = ProjectId::from_ulid(ulid::Ulid(999));
        let error = h.err(
            "milestone.create",
            json!({"project": ghost.to_string(), "title": "x", "date": "2026-11-01"}),
        );
        assert_eq!(rule_of(&error), Some(Rule::MissingReference));
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
        h.ok(
            "milestone.create",
            json!({"project": template.to_string(), "title": "Abgabe Senatskanzlei", "date": "2026-01-08"}),
        );
        (template, [eingang, zuarbeit, teil, entwurf])
    }

    #[test]
    fn instantiate_copies_structure_and_shifts_dates() {
        let mut h = Harness::new();
        let (template, _) = ska_template(&mut h);
        let template_before = h.ws().tasks_of(template).cloned().collect::<Vec<_>>();
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
        assert_eq!(tasks.len(), 4);
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
        let milestone = h.ws().milestones().find(|m| m.project == case).unwrap();
        assert_eq!(milestone.date.to_string(), "2026-11-09");
        assert_eq!(
            h.ws().tasks_of(template).cloned().collect::<Vec<_>>(),
            template_before
        );
        let count = h.ws().len();
        h.undo();
        assert!(h.ws().project(case).is_none());
        assert!(h.ws().len() < count);
    }

    #[test]
    fn instantiate_without_template_start_drops_task_dates() {
        let mut h = Harness::new();
        let template = h.project(json!({"title": "T", "template": true}));
        h.task(template, "a", json!({"due": "2026-01-05"}));
        let created = h.ok(
            "project.instantiate",
            json!({"template": template.to_string(), "title": "Neu", "start": "2026-11-02", "due": "2026-11-30"}),
        );
        let Some(EntityKey::Project(case)) = created.first().copied() else {
            panic!("{created:?}");
        };
        assert!(h.ws().tasks_of(case).all(|t| t.due.is_none()));
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
}
