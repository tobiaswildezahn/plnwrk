//! Several people on one shared file (Phase 10a): own changes set onto what others saved.

#[cfg(test)]
mod sync {
    use plnwrk_core::{EntityKey, FixedClock, PersonId, SequentialIds, TaskId, Workspace};
    use plnwrk_engine::sync::{Dropped, NotContinued};
    use plnwrk_engine::{Change, Origin, Session};
    use proptest::prelude::*;
    use serde_json::{Value, json};

    /// A session on `ws`/`log` whose new ids start at `ids` (two people's computers never hand
    /// out the same id).
    fn fork(ws: &Workspace, log: &[Change], ids: u128) -> Session {
        Session::with_workspace(
            ws.clone(),
            log.to_vec(),
            Box::new(FixedClock::at("2026-10-19").unwrap()),
            Box::new(SequentialIds::starting_at(ids)),
        )
    }

    fn run(s: &mut Session, command: &str, params: Value) -> Option<EntityKey> {
        s.run(command, &params, Origin::User)
            .unwrap_or_else(|e| panic!("{command}: {e:?}"))
            .created
            .first()
            .copied()
    }

    fn person(s: &mut Session, name: &str) -> PersonId {
        match run(s, "person.create", json!({"name": name})) {
            Some(EntityKey::Person(id)) => id,
            other => panic!("{other:?}"),
        }
    }

    fn task(s: &mut Session, project: &str, title: &str, due: &str) -> TaskId {
        match run(
            s,
            "task.create",
            json!({"project": project, "title": title, "due": due}),
        ) {
            Some(EntityKey::Task(id)) => id,
            other => panic!("{other:?}"),
        }
    }

    /// The shared start: a project with two tasks and two people. Returns the base session, the
    /// people (Anna, Ben) and the tasks (Einbau, Vergabe).
    fn start() -> (Session, [PersonId; 2], [TaskId; 2]) {
        let mut s = fork(&Workspace::default(), &[], 1);
        let anna = person(&mut s, "Anna Berg");
        let ben = person(&mut s, "Ben Kruse");
        let project = match run(&mut s, "project.create", json!({"title": "Funk"})) {
            Some(EntityKey::Project(id)) => id.to_string(),
            other => panic!("{other:?}"),
        };
        let einbau = task(&mut s, &project, "Einbau", "2026-11-10");
        let vergabe = task(&mut s, &project, "Vergabe", "2026-11-20");
        (s, [anna, ben], [einbau, vergabe])
    }

    /// Anna's and Ben's sessions on the shared start, each working as themselves.
    fn two() -> (Session, Session, usize, [TaskId; 2], [PersonId; 2]) {
        let (base, people, tasks) = start();
        let mut anna = fork(base.workspace(), base.log(), 1_000);
        let mut ben = fork(base.workspace(), base.log(), 2_000);
        anna.set_actor(Some(people[0]));
        ben.set_actor(Some(people[1]));
        (anna, ben, base.log().len(), tasks, people)
    }

    /// Ben saves after Anna: his changes onto her file.
    fn ben_saves_after(
        anna: &Session,
        ben: &mut Session,
        base: usize,
    ) -> plnwrk_engine::sync::Rebased {
        ben.rebase_onto(anna.workspace().clone(), anna.log().to_vec(), base)
            .expect("continues")
    }

    /// The log replayed from nothing gives the state, and the state keeps every rule – what
    /// the store checks when the file is opened.
    fn valid(s: &Session) {
        let mut ws = Workspace::default();
        for (n, change) in s.log().iter().enumerate() {
            assert_eq!(change.seq, n as u64 + 1, "numbered on");
            for event in &change.events {
                event.apply(&mut ws).expect("replays");
            }
        }
        assert_eq!(&ws, s.workspace(), "replay gives the state");
        assert!(
            s.workspace().check().is_empty(),
            "{:?}",
            s.workspace().check()
        );
    }

    fn title(s: &Session, id: TaskId) -> Option<String> {
        s.workspace().task(id).map(|t| t.title.clone())
    }

    #[test]
    fn changes_to_different_things_both_arrive() {
        let (mut anna, mut ben, base, [einbau, vergabe], _) = two();
        run(
            &mut anna,
            "task.update",
            json!({"task": einbau.to_string(), "title": "Einbau Wache 11"}),
        );
        run(
            &mut ben,
            "task.update",
            json!({"task": vergabe.to_string(), "priority": "high"}),
        );
        let merged = ben_saves_after(&anna, &mut ben, base);
        assert_eq!(title(&ben, einbau).as_deref(), Some("Einbau Wache 11"));
        assert_eq!(
            ben.workspace().task(vergabe).unwrap().priority,
            plnwrk_core::Priority::High
        );
        assert!(merged.conflicts.is_empty() && merged.lost.is_empty());
        assert_eq!(merged.foreign.len(), 1);
        assert_eq!(ben.log().len(), base + 2, "hers, then his");
        valid(&ben);
    }

    #[test]
    fn different_fields_of_one_task_both_arrive() {
        let (mut anna, mut ben, base, [einbau, _], _) = two();
        run(
            &mut anna,
            "task.update",
            json!({"task": einbau.to_string(), "title": "Einbau neu"}),
        );
        run(
            &mut ben,
            "task.update",
            json!({"task": einbau.to_string(), "priority": "high"}),
        );
        let merged = ben_saves_after(&anna, &mut ben, base);
        let task = ben.workspace().task(einbau).unwrap();
        assert_eq!(task.title, "Einbau neu");
        assert_eq!(task.priority, plnwrk_core::Priority::High);
        assert!(merged.conflicts.is_empty());
        valid(&ben);
    }

    #[test]
    fn the_same_field_the_later_save_wins_and_it_is_named() {
        let (mut anna, mut ben, base, [einbau, _], [anna_id, _]) = two();
        run(
            &mut anna,
            "task.update",
            json!({"task": einbau.to_string(), "due": "2026-11-20"}),
        );
        run(
            &mut ben,
            "task.update",
            json!({"task": einbau.to_string(), "due": "2026-11-25"}),
        );
        let merged = ben_saves_after(&anna, &mut ben, base);
        assert_eq!(
            ben.workspace()
                .task(einbau)
                .unwrap()
                .due
                .map(|d| d.to_string())
                .as_deref(),
            Some("2026-11-25")
        );
        assert_eq!(merged.conflicts.len(), 1);
        let conflict = &merged.conflicts[0];
        assert_eq!(conflict.entity, EntityKey::Task(einbau));
        assert!(conflict.fields.contains(&"due".to_owned()), "{conflict:?}");
        assert_eq!(conflict.theirs.get("due"), Some(&json!("2026-11-20")));
        assert_eq!(conflict.ours.get("due"), Some(&json!("2026-11-25")));
        assert_eq!(conflict.by, Some(anna_id));
        valid(&ben);
    }

    #[test]
    fn dates_go_together_never_one_of_each() {
        let (mut anna, mut ben, base, [einbau, _], _) = two();
        run(
            &mut anna,
            "task.update",
            json!({"task": einbau.to_string(), "start": "2026-11-01"}),
        );
        run(
            &mut ben,
            "task.update",
            json!({"task": einbau.to_string(), "due": "2026-11-30"}),
        );
        let merged = ben_saves_after(&anna, &mut ben, base);
        let task = ben.workspace().task(einbau).unwrap();
        // Ben's dates as a whole: his deadline with the start he saw (none).
        assert_eq!(
            task.due.map(|d| d.to_string()).as_deref(),
            Some("2026-11-30")
        );
        assert_eq!(task.start, None);
        assert_eq!(merged.conflicts.len(), 1, "her start is named");
        valid(&ben);
    }

    #[test]
    fn a_change_to_what_the_other_removed_is_lost_and_named() {
        let (mut anna, mut ben, base, [einbau, _], _) = two();
        run(
            &mut anna,
            "task.delete",
            json!({"task": einbau.to_string()}),
        );
        run(
            &mut ben,
            "task.update",
            json!({"task": einbau.to_string(), "title": "Einbau neu"}),
        );
        let merged = ben_saves_after(&anna, &mut ben, base);
        assert!(ben.workspace().task(einbau).is_none(), "stays removed");
        assert_eq!(merged.lost.len(), 1);
        assert!(matches!(
            merged.lost[0].dropped,
            Dropped::Gone { entity: EntityKey::Task(id) } if id == einbau
        ));
        valid(&ben);
    }

    #[test]
    fn removing_what_the_other_changed_removes_it() {
        let (mut anna, mut ben, base, [einbau, _], _) = two();
        run(
            &mut anna,
            "task.update",
            json!({"task": einbau.to_string(), "title": "Einbau neu"}),
        );
        run(&mut ben, "task.delete", json!({"task": einbau.to_string()}));
        let merged = ben_saves_after(&anna, &mut ben, base);
        assert!(ben.workspace().task(einbau).is_none());
        assert!(merged.lost.is_empty());
        valid(&ben);
    }

    #[test]
    fn a_circle_of_dependencies_is_not_taken() {
        let (mut anna, mut ben, base, [einbau, vergabe], _) = two();
        run(
            &mut anna,
            "dependency.add",
            json!({"from": einbau.to_string(), "to": vergabe.to_string()}),
        );
        run(
            &mut ben,
            "dependency.add",
            json!({"from": vergabe.to_string(), "to": einbau.to_string()}),
        );
        let merged = ben_saves_after(&anna, &mut ben, base);
        assert_eq!(merged.lost.len(), 1);
        assert!(
            matches!(merged.lost[0].dropped, Dropped::Invalid { .. }),
            "{:?}",
            merged.lost
        );
        assert_eq!(ben.workspace().dependencies().count(), 1, "hers stays");
        valid(&ben);
    }

    #[test]
    fn undo_takes_back_the_own_step_not_the_others() {
        let (mut anna, mut ben, base, [einbau, _], _) = two();
        run(
            &mut anna,
            "task.update",
            json!({"task": einbau.to_string(), "priority": "high"}),
        );
        run(
            &mut ben,
            "task.update",
            json!({"task": einbau.to_string(), "title": "Einbau neu"}),
        );
        ben_saves_after(&anna, &mut ben, base);
        assert!(ben.can_undo());
        run(&mut ben, "undo", json!({}));
        let task = ben.workspace().task(einbau).unwrap();
        assert_eq!(task.title, "Einbau", "his step taken back");
        assert_eq!(task.priority, plnwrk_core::Priority::High, "hers stays");
        valid(&ben);
    }

    #[test]
    fn an_own_undo_points_at_the_own_step_under_its_new_number() {
        let (mut anna, mut ben, base, [einbau, vergabe], _) = two();
        run(
            &mut anna,
            "task.update",
            json!({"task": vergabe.to_string(), "priority": "high"}),
        );
        run(
            &mut ben,
            "task.update",
            json!({"task": einbau.to_string(), "title": "Einbau neu"}),
        );
        let edit = ben.log().last().unwrap().seq;
        run(&mut ben, "undo", json!({}));
        let merged = ben_saves_after(&anna, &mut ben, base);
        let now = merged.moved[&edit];
        let undo = ben.log().last().unwrap();
        assert_eq!(undo.command, "undo");
        assert_eq!(undo.undoes, Some(now), "renumbered with what it undoes");
        // Redo still repeats his step, under its new number.
        assert!(ben.can_redo());
        run(&mut ben, "redo", json!({}));
        assert_eq!(title(&ben, einbau).as_deref(), Some("Einbau neu"));
        assert_eq!(ben.log().last().unwrap().redoes, Some(now));
        valid(&ben);
    }

    #[test]
    fn an_own_redo_points_at_the_own_step_under_its_new_number() {
        let (mut anna, mut ben, base, [einbau, vergabe], _) = two();
        run(
            &mut anna,
            "task.update",
            json!({"task": vergabe.to_string(), "priority": "high"}),
        );
        run(
            &mut ben,
            "task.update",
            json!({"task": einbau.to_string(), "title": "Einbau neu"}),
        );
        let edit = ben.log().last().unwrap().seq;
        run(&mut ben, "undo", json!({}));
        run(&mut ben, "redo", json!({}));
        let merged = ben_saves_after(&anna, &mut ben, base);
        let redo = ben.log().last().unwrap();
        assert_eq!(redo.command, "redo");
        assert_eq!(redo.redoes, Some(merged.moved[&edit]));
        assert_eq!(title(&ben, einbau).as_deref(), Some("Einbau neu"));
        valid(&ben);
    }

    #[test]
    fn deleting_what_the_other_deleted_too_is_nothing_to_do() {
        let (mut anna, mut ben, base, [einbau, _], _) = two();
        run(
            &mut anna,
            "task.delete",
            json!({"task": einbau.to_string()}),
        );
        run(&mut ben, "task.delete", json!({"task": einbau.to_string()}));
        let merged = ben_saves_after(&anna, &mut ben, base);
        assert!(merged.lost.is_empty(), "{:?}", merged.lost);
        assert_eq!(ben.log().len(), base + 1, "her delete only");
        valid(&ben);
    }

    #[test]
    fn an_undo_of_a_step_not_taken_goes_with_it() {
        let (mut anna, mut ben, base, [einbau, _], _) = two();
        run(
            &mut anna,
            "task.delete",
            json!({"task": einbau.to_string()}),
        );
        run(
            &mut ben,
            "task.update",
            json!({"task": einbau.to_string(), "title": "Einbau neu"}),
        );
        run(&mut ben, "undo", json!({}));
        let merged = ben_saves_after(&anna, &mut ben, base);
        assert_eq!(merged.lost.len(), 2, "{:?}", merged.lost);
        assert!(matches!(merged.lost[1].dropped, Dropped::Follows { .. }));
        assert!(!ben.can_undo() || ben.log().iter().all(|c| c.command != "undo"));
        valid(&ben);
    }

    #[test]
    fn an_own_step_the_file_already_holds_is_left_out() {
        let (mut anna, mut ben, base, [einbau, _], _) = two();
        for s in [&mut anna, &mut ben] {
            run(
                s,
                "task.update",
                json!({"task": einbau.to_string(), "title": "Gleich"}),
            );
        }
        let merged = ben_saves_after(&anna, &mut ben, base);
        assert_eq!(ben.log().len(), base + 1, "no empty step");
        assert!(merged.conflicts.is_empty() && merged.lost.is_empty());
        valid(&ben);
    }

    #[test]
    fn a_file_that_does_not_continue_the_start_is_refused() {
        let (anna, mut ben, base, _, _) = two();
        let (other, _, _) = start();
        let mut other = fork(other.workspace(), other.log(), 3_000);
        run(&mut other, "person.create", json!({"name": "Fremd"}));
        // Another file entirely: its first changes differ.
        let mut log = other.log().to_vec();
        log[0].params = json!({"name": "Ganz anders"});
        let refused = ben.rebase_onto(other.workspace().clone(), log, base);
        assert_eq!(refused.err(), Some(NotContinued));
        // And a base longer than the file.
        let refused = ben.rebase_onto(anna.workspace().clone(), anna.log().to_vec(), base + 5);
        assert_eq!(refused.err(), Some(NotContinued));
    }

    #[test]
    fn an_undo_of_what_someone_changed_since_is_not_taken() {
        // Ben renamed it and saved; Anna renamed it again; Ben then takes his step back.
        let (base_session, [_, ben_id], [einbau, _]) = start();
        let mut ben = fork(base_session.workspace(), base_session.log(), 2_000);
        ben.set_actor(Some(ben_id));
        run(
            &mut ben,
            "task.update",
            json!({"task": einbau.to_string(), "title": "Von Ben"}),
        );
        let base = ben.log().len();
        let mut anna = fork(ben.workspace(), ben.log(), 1_000);
        run(
            &mut anna,
            "task.update",
            json!({"task": einbau.to_string(), "title": "Von Anna"}),
        );
        run(&mut ben, "undo", json!({}));
        let merged = ben_saves_after(&anna, &mut ben, base);
        assert_eq!(
            title(&ben, einbau).as_deref(),
            Some("Von Anna"),
            "her newer value stays"
        );
        assert_eq!(merged.lost.len(), 1);
        assert!(
            matches!(merged.lost[0].dropped, Dropped::Changed { .. }),
            "{:?}",
            merged.lost
        );
        valid(&ben);
    }

    #[test]
    fn the_same_new_entity_twice_is_taken_once_another_is_not() {
        let (base, _, _) = start();
        let n = base.log().len();
        for (hers, his, lost) in [
            ("Carla Meier", "Carla Meier", 0),
            ("Carla Meier", "Carl Maier", 1),
        ] {
            // Same ids on both sides: only so can both create "the same" entity.
            let mut anna = fork(base.workspace(), base.log(), 5_000);
            let mut ben = fork(base.workspace(), base.log(), 5_000);
            run(&mut anna, "person.create", json!({"name": hers}));
            run(&mut ben, "person.create", json!({"name": his}));
            let merged = ben_saves_after(&anna, &mut ben, n);
            assert_eq!(merged.lost.len(), lost, "{his}: {:?}", merged.lost);
            assert_eq!(ben.log().len(), n + 1, "{his}");
            valid(&ben);
        }
    }

    #[test]
    fn a_name_and_its_initials_go_together() {
        let (mut anna, mut ben, base, _, [_, ben_id]) = two();
        run(
            &mut anna,
            "person.update",
            json!({"person": ben_id.to_string(), "name": "Benjamin Kruse"}),
        );
        run(
            &mut ben,
            "person.update",
            json!({"person": ben_id.to_string(), "initials": "BK2"}),
        );
        let merged = ben_saves_after(&anna, &mut ben, base);
        let person = ben.workspace().person(ben_id).unwrap();
        assert_eq!(
            (person.name.as_str(), person.initials.as_str()),
            ("Ben Kruse", "BK2"),
            "his pair, not a mix"
        );
        assert_eq!(merged.conflicts.len(), 1);
        valid(&ben);
    }

    #[test]
    fn a_milestone_and_its_parent_go_together() {
        let (mut anna, mut ben, base, [einbau, vergabe], _) = two();
        run(
            &mut anna,
            "task.update",
            json!({"task": einbau.to_string(), "milestone": true}),
        );
        run(
            &mut ben,
            "task.reparent",
            json!({"task": einbau.to_string(), "parent": vergabe.to_string()}),
        );
        let merged = ben_saves_after(&anna, &mut ben, base);
        let task = ben.workspace().task(einbau).unwrap();
        assert_eq!(task.parent, Some(vergabe), "his place");
        assert!(
            !task.milestone,
            "with what he saw: no milestone under a parent"
        );
        assert!(merged.lost.is_empty(), "{:?}", merged.lost);
        assert_eq!(merged.conflicts.len(), 1);
        valid(&ben);
    }

    #[test]
    fn labels_are_merged_as_a_set() {
        let (base_session, _, [einbau, _]) = start();
        let mut start_session = fork(base_session.workspace(), base_session.log(), 900);
        run(
            &mut start_session,
            "task.update",
            json!({"task": einbau.to_string(), "tags": ["x", "y"]}),
        );
        let base = start_session.log().len();
        let mut anna = fork(start_session.workspace(), start_session.log(), 1_000);
        let mut ben = fork(start_session.workspace(), start_session.log(), 2_000);
        run(
            &mut anna,
            "task.update",
            json!({"task": einbau.to_string(), "tags": ["x", "y", "a"]}),
        );
        run(
            &mut ben,
            "task.update",
            json!({"task": einbau.to_string(), "tags": ["y", "b"]}),
        );
        let merged = ben_saves_after(&anna, &mut ben, base);
        assert_eq!(
            ben.workspace().task(einbau).unwrap().tags,
            ["a", "b", "y"],
            "x removed by him, a added by her, b by him"
        );
        assert!(merged.conflicts.is_empty());
        valid(&ben);
    }

    #[test]
    fn deleting_a_task_takes_subtasks_added_meanwhile_along() {
        let (mut anna, mut ben, base, [einbau, _], _) = two();
        let project = anna.workspace().projects().next().unwrap().id.to_string();
        run(
            &mut anna,
            "task.create",
            json!({"project": project, "title": "Neue Unteraufgabe", "parent": einbau.to_string()}),
        );
        run(&mut ben, "task.delete", json!({"task": einbau.to_string()}));
        let merged = ben_saves_after(&anna, &mut ben, base);
        assert!(merged.lost.is_empty(), "{:?}", merged.lost);
        assert!(ben.workspace().task(einbau).is_none());
        assert!(
            ben.workspace()
                .tasks()
                .all(|t| t.title != "Neue Unteraufgabe"),
            "taken along"
        );
        valid(&ben);
    }

    #[test]
    fn what_could_not_be_taken_keeps_its_text() {
        let (mut anna, mut ben, base, [einbau, _], _) = two();
        run(
            &mut anna,
            "task.delete",
            json!({"task": einbau.to_string()}),
        );
        run(
            &mut ben,
            "comment.add",
            json!({"task": einbau.to_string(), "body": "Wichtiger Hinweis"}),
        );
        let merged = ben_saves_after(&anna, &mut ben, base);
        assert_eq!(merged.lost.len(), 1);
        assert_eq!(merged.lost[0].text.as_deref(), Some("Wichtiger Hinweis"));
        valid(&ben);
    }

    #[test]
    fn two_new_tasks_at_the_same_place_get_their_own() {
        let (mut anna, mut ben, base, _, _) = two();
        let project = anna.workspace().projects().next().unwrap().id.to_string();
        run(
            &mut anna,
            "task.create",
            json!({"project": project, "title": "Ihre"}),
        );
        run(
            &mut ben,
            "task.create",
            json!({"project": project, "title": "Seine"}),
        );
        ben_saves_after(&anna, &mut ben, base);
        ranks_unique(&ben);
        let order: Vec<String> = {
            let mut tasks: Vec<_> = ben
                .workspace()
                .tasks()
                .filter(|t| t.parent.is_none())
                .collect();
            tasks.sort_by(|a, b| a.rank.cmp(&b.rank));
            tasks.iter().map(|t| t.title.clone()).collect()
        };
        assert_eq!(
            order.last().map(String::as_str),
            Some("Seine"),
            "his right behind hers"
        );
        valid(&ben);
    }

    /// No two tasks of the same parent share a place.
    fn ranks_unique(s: &Session) {
        let mut seen = std::collections::BTreeSet::new();
        for t in s.workspace().tasks() {
            assert!(
                seen.insert((t.project, t.parent, t.rank.clone())),
                "rank of {} twice",
                t.title
            );
        }
    }

    // Many random steps by both: the merged file is always valid, holds everything the other
    // saved, and every own step is either taken or named.

    #[derive(Debug, Clone)]
    enum Step {
        Rename(usize, u8),
        Priority(usize, bool),
        Due(usize, u8),
        Delete(usize),
        Create(u8),
        Depend(usize, usize),
        Undo,
    }

    fn step() -> impl Strategy<Value = Step> {
        prop_oneof![
            (0usize..4, any::<u8>()).prop_map(|(t, n)| Step::Rename(t, n)),
            (0usize..4, any::<bool>()).prop_map(|(t, h)| Step::Priority(t, h)),
            (0usize..4, 1u8..28).prop_map(|(t, d)| Step::Due(t, d)),
            (0usize..4).prop_map(Step::Delete),
            any::<u8>().prop_map(Step::Create),
            (0usize..4, 0usize..4).prop_map(|(a, b)| Step::Depend(a, b)),
            Just(Step::Undo),
        ]
    }

    /// Does one step if it can (refusals are fine: the person would see them).
    fn apply(s: &mut Session, step: &Step) {
        let project = s.workspace().projects().next().map(|p| p.id.to_string());
        let tasks: Vec<String> = s.workspace().tasks().map(|t| t.id.to_string()).collect();
        let pick = |n: usize| tasks.get(n % tasks.len().max(1)).cloned();
        let (command, params) = match step {
            Step::Rename(t, n) => (
                "task.update",
                pick(*t).map(|id| json!({"task": id, "title": format!("T{n}")})),
            ),
            Step::Priority(t, h) => (
                "task.update",
                pick(*t).map(|id| json!({"task": id, "priority": if *h { "high" } else { "low" }})),
            ),
            Step::Due(t, d) => (
                "task.update",
                pick(*t).map(|id| json!({"task": id, "due": format!("2026-12-{d:02}")})),
            ),
            Step::Delete(t) => ("task.delete", pick(*t).map(|id| json!({"task": id}))),
            Step::Create(n) => (
                "task.create",
                project.map(|p| json!({"project": p, "title": format!("Neu {n}")})),
            ),
            Step::Depend(a, b) => (
                "dependency.add",
                pick(*a)
                    .zip(pick(*b))
                    .map(|(f, t)| json!({"from": f, "to": t})),
            ),
            Step::Undo => ("undo", Some(json!({}))),
        };
        if let Some(params) = params {
            let _ = s.run(command, &params, Origin::User);
        }
    }

    proptest! {
        #[test]
        fn merging_random_work_gives_a_valid_file_with_both(
            hers in proptest::collection::vec(step(), 0..12),
            his in proptest::collection::vec(step(), 0..12),
        ) {
            let (mut anna, mut ben, base, _, _) = two();
            for s in &hers { apply(&mut anna, s); }
            for s in &his { apply(&mut ben, s); }
            let own = ben.log().len() - base;
            let merged = ben_saves_after(&anna, &mut ben, base);
            valid(&ben);
            ranks_unique(&ben);
            prop_assert_eq!(&ben.log()[..anna.log().len()], anna.log(), "everything she saved, unchanged, first");
            // Nothing of my own: the merge is her file, unchanged.
            let (start_session, _, _) = start();
            let mut quiet = fork(start_session.workspace(), start_session.log(), 3_000);
            let same = quiet.rebase_onto(anna.workspace().clone(), anna.log().to_vec(), base).expect("continues");
            prop_assert_eq!(quiet.log(), anna.log());
            prop_assert_eq!(quiet.workspace(), anna.workspace());
            prop_assert!(same.moved.is_empty() && same.lost.is_empty());
            // Every own change is accounted for: taken, not taken, or held already (L3).
            prop_assert_eq!(merged.moved.len() + merged.lost.len() + merged.held.len(), own);
            for lost in &merged.lost {
                prop_assert!(lost.seq as usize > base);
            }
        }
    }

    /// REVIEW E1: Ben deletes a task Anna has just commented on (saved before him) – her
    /// comment must not vanish without anyone being told.
    #[test]
    fn review_a_comment_saved_meanwhile_does_not_vanish_unsaid() {
        let (mut anna, mut ben, base, [einbau, _], _) = two();
        run(
            &mut anna,
            "comment.add",
            json!({"task": einbau.to_string(), "body": "Annas Hinweis"}),
        );
        run(&mut ben, "task.delete", json!({"task": einbau.to_string()}));
        let merged = ben_saves_after(&anna, &mut ben, base);
        // Deleting wins (decided in review 10a, F10) – but what it took along is named.
        let comment = anna
            .workspace()
            .comments()
            .find(|c| c.body == "Annas Hinweis")
            .unwrap()
            .id;
        assert!(ben.workspace().comment(comment).is_none(), "deleted along");
        assert!(
            merged
                .swept
                .iter()
                .any(|s| s.entity == plnwrk_core::EntityKey::Comment(comment)),
            "gone, nothing said: swept {:?}",
            merged.swept
        );
    }

    /// REVIEW E2: Anna renamed a task and saved; Ben deletes it – her change must be named.
    #[test]
    fn review_a_change_saved_meanwhile_is_not_deleted_unsaid() {
        let (mut anna, mut ben, base, [einbau, _], _) = two();
        run(
            &mut anna,
            "task.update",
            json!({"task": einbau.to_string(), "title": "Einbau – Angebot liegt vor"}),
        );
        run(&mut ben, "task.delete", json!({"task": einbau.to_string()}));
        let merged = ben_saves_after(&anna, &mut ben, base);
        assert!(ben.workspace().task(einbau).is_none(), "deleting wins");
        assert!(
            merged
                .swept
                .iter()
                .any(|s| s.entity == plnwrk_core::EntityKey::Task(einbau)),
            "her change is gone, nothing said: swept {:?}",
            merged.swept
        );
    }

    /// REVIEW E3: the conflict names who set the other value – not whoever touched the task
    /// last.
    #[test]
    fn review_a_conflict_names_who_set_the_other_value() {
        let (mut anna, mut ben, base, [einbau, _], [anna_id, _]) = two();
        run(
            &mut anna,
            "task.update",
            json!({"task": einbau.to_string(), "due": "2026-11-21"}),
        );
        let mut carla = fork(anna.workspace(), anna.log(), 3_000);
        let carla_id = person(&mut carla, "Carla Meier");
        carla.set_actor(Some(carla_id));
        run(
            &mut carla,
            "task.update",
            json!({"task": einbau.to_string(), "title": "Einbau (Carla)"}),
        );
        run(
            &mut ben,
            "task.update",
            json!({"task": einbau.to_string(), "due": "2026-11-25"}),
        );
        let merged = ben
            .rebase_onto(carla.workspace().clone(), carla.log().to_vec(), base)
            .unwrap();
        assert_eq!(merged.conflicts.len(), 1);
        assert_eq!(
            merged.conflicts[0].by,
            Some(anna_id),
            "Carla only renamed it; the deadline was Anna's"
        );
    }
}
