//! Queries on the sample workspace: behaviour checks and JSON snapshots.

#[cfg(test)]
mod queries {
    use plnwrk_core::{EntityKey, FixedClock, SequentialIds, TaskStatus};
    use plnwrk_engine::fixture::{Sample, sample};
    use plnwrk_engine::query::{
        self, Action, MilestoneState, TaskGroup, TaskQuery, TaskSort, query_registry,
    };
    use plnwrk_engine::{CommandError, Origin, Session};
    use serde_json::{Value, json};

    fn setup() -> (Session, Sample) {
        let mut session = Session::new(
            Box::new(FixedClock::at("2026-10-14").unwrap()),
            Box::new(SequentialIds::default()),
        );
        let ids = sample(&mut session).unwrap();
        (session, ids)
    }

    /// Midnight at the start of the sample's "today" (local time).
    fn today() -> jiff::civil::DateTime {
        "2026-10-14T00:00".parse().unwrap()
    }

    #[test]
    fn sample_is_consistent() {
        let (s, _) = setup();
        assert!(s.workspace().check().is_empty());
    }

    #[test]
    fn overview_lists_live_projects_by_deadline_without_templates() {
        let (s, ids) = setup();
        let cards = query::overview(s.workspace(), today());
        let names: Vec<&str> = cards.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(
            names,
            ["SKA 23/1234 Sirenenwarnung", "Digitalfunk Umrüstung Wachen"]
        );
        let project = cards.iter().find(|c| c.id == ids.project).unwrap();
        // Milestones are reported on their own, not as overdue work.
        assert_eq!(project.overdue_tasks, 1);
        assert_eq!(project.open_tasks, 4);
        assert_eq!(project.days_left, Some(62));
        let next = project.next_milestone.as_ref().unwrap();
        assert_eq!(
            (next.title.as_str(), next.state),
            ("Zuschlag erteilt", MilestoneState::Missed)
        );
        let case = cards.iter().find(|c| c.id == ids.case).unwrap();
        assert_eq!(case.reference.as_deref(), Some("23/1234"));
        assert_eq!(case.due.unwrap().to_string(), "2026-10-19");
    }

    #[test]
    fn archived_projects_leave_the_overview() {
        let (mut s, ids) = setup();
        s.run(
            "project.archive",
            &json!({"project": ids.case.to_string()}),
            Origin::User,
        )
        .unwrap();
        assert_eq!(query::overview(s.workspace(), today()).len(), 1);
        assert_eq!(query::templates(s.workspace(), today()).len(), 1);
    }

    #[test]
    fn progress_counts_leaf_tasks_and_done_as_full() {
        let (s, ids) = setup();
        let progress = query::progress(s.workspace(), ids.project).unwrap();
        // Leaves: Bestandsaufnahme (done), Leistungsverzeichnis (done), Vergabevermerk (0),
        // Einbau (0), Schulung (0) → 2 of 5 done, (100+100)/5 = 40 %.
        assert_eq!(
            (progress.tasks, progress.done, progress.percent),
            (5, 2, 40)
        );
        let ghost = plnwrk_core::ProjectId::from_ulid(ulid::Ulid(999_999));
        assert!(query::progress(s.workspace(), ghost).is_none());
    }

    #[test]
    fn overdue_finds_tasks_and_missed_milestones() {
        let (s, _) = setup();
        let items = query::overdue(s.workspace(), today());
        let titles: Vec<(&str, i64)> = items
            .iter()
            .map(|i| (i.title.as_str(), i.days_late))
            .collect();
        assert_eq!(
            titles,
            [("Ausschreibung vorbereiten", 4), ("Zuschlag erteilt", 2)]
        );
        assert!(items.iter().all(|i| matches!(i.entity, EntityKey::Task(_))));
    }

    /// Phase 9f: what is due today and not past yet – the counterpart of `overdue`, which takes
    /// over once the time has passed; nothing done, nothing in templates, nothing tomorrow.
    #[test]
    fn due_today_is_what_overdue_does_not_have_yet() {
        let (mut s, ids) = setup();
        let project = ids.project.to_string();
        for (title, due, extra) in [
            ("Heute", "2026-10-14", json!({})),
            ("Heute 15 Uhr", "2026-10-14T15:00", json!({})),
            ("Heute 8 Uhr", "2026-10-14T08:00", json!({})),
            ("Heute erledigt", "2026-10-14", json!({"status": "done"})),
            ("Morgen", "2026-10-15", json!({})),
        ] {
            let mut params = json!({"project": project, "title": title, "due": due});
            if let (Some(p), Some(e)) = (params.as_object_mut(), extra.as_object()) {
                p.extend(e.clone());
            }
            s.run("task.create", &params, Origin::User).unwrap();
        }
        let now: jiff::civil::DateTime = "2026-10-14T09:00".parse().unwrap();
        let today: Vec<String> = query::due_today(s.workspace(), now)
            .into_iter()
            .map(|i| i.title)
            .collect();
        // Earliest first: a time of day before the whole day (the app's deadline order).
        assert_eq!(today, ["Heute 15 Uhr", "Heute"]);
        // The one at 8 is overdue at 9 – there, not here.
        let overdue: Vec<String> = query::overdue(s.workspace(), now)
            .into_iter()
            .map(|i| i.title)
            .collect();
        assert!(overdue.contains(&"Heute 8 Uhr".to_owned()), "{overdue:?}");
    }

    #[test]
    fn board_has_five_columns_in_order() {
        let (s, ids) = setup();
        let board = query::board(s.workspace(), ids.project, today()).unwrap();
        let columns: Vec<TaskStatus> = board.columns.iter().map(|c| c.status).collect();
        assert_eq!(columns, TaskStatus::ALL);
        let todo: Vec<&str> = board.columns[0]
            .tasks
            .iter()
            .map(|t| t.title.as_str())
            .collect();
        assert_eq!(
            todo,
            // Subtasks such as "Vergabevermerk" are in their parent's card, not cards.
            [
                "Einbau Wache 11",
                "Schulung Einsatzkräfte",
                "Zuschlag erteilt",
                "Inbetriebnahme",
            ]
        );
        let tender = &board.columns[1].tasks[0];
        assert_eq!(
            (tender.subtasks, tender.subtasks_done, tender.overdue),
            (2, 1, true)
        );
        assert_eq!(
            tender.assignee.as_ref().map(|a| a.initials.as_str()),
            Some("AB")
        );
    }

    #[test]
    fn task_list_filters_sorts_and_groups() {
        let (s, ids) = setup();
        let ws = s.workspace();
        let mine = query::tasks(
            ws,
            &TaskQuery {
                assignee: Some(ids.anna),
                sort: TaskSort::Title,
                ..TaskQuery::default()
            },
            today(),
        );
        let titles: Vec<&str> = mine[0].tasks.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, ["Ausschreibung vorbereiten", "Vergabevermerk"]);
        let by_status = query::tasks(
            ws,
            &TaskQuery {
                project: Some(ids.project),
                include_done: true,
                group: Some(TaskGroup::Status),
                ..TaskQuery::default()
            },
            today(),
        );
        let groups: Vec<(&str, usize)> = by_status
            .iter()
            .map(|g| (g.group.as_str(), g.tasks.len()))
            .collect();
        // Three tasks and two milestones are open.
        assert_eq!(groups, [("todo", 5), ("in_progress", 1), ("done", 2)]);
        let found = query::tasks(
            ws,
            &TaskQuery {
                text: Some("WACHE".into()),
                ..TaskQuery::default()
            },
            today(),
        );
        assert_eq!(found[0].tasks.len(), 1);
        let tagged = query::tasks(
            ws,
            &TaskQuery {
                tag: Some("Vergabe".into()),
                ..TaskQuery::default()
            },
            today(),
        );
        assert_eq!(tagged[0].tasks.len(), 1);
        let by_person = query::tasks(
            ws,
            &TaskQuery {
                project: Some(ids.project),
                group: Some(TaskGroup::Assignee),
                ..TaskQuery::default()
            },
            today(),
        );
        let people: Vec<&str> = by_person.iter().map(|g| g.group.as_str()).collect();
        assert_eq!(people, ["Anna Berg", "unassigned"]);
        let by_due = query::tasks(
            ws,
            &TaskQuery {
                project: Some(ids.project),
                sort: TaskSort::Due,
                ..TaskQuery::default()
            },
            today(),
        );
        assert_eq!(
            by_due[0].tasks.first().map(|t| t.title.as_str()),
            Some("Ausschreibung vorbereiten")
        );
        assert_eq!(
            by_due[0].tasks.last().map(|t| t.title.as_str()),
            Some("Vergabevermerk")
        );
    }

    #[test]
    fn task_lists_without_project_leave_out_templates_and_archive() {
        let (mut s, ids) = setup();
        let all = query::tasks(
            s.workspace(),
            &TaskQuery {
                include_done: true,
                ..TaskQuery::default()
            },
            today(),
        );
        assert!(all[0].tasks.iter().all(|t| t.project != ids.template));
        // Ten tasks and three milestones (two in the project, one in the case).
        assert_eq!(all[0].tasks.len(), 13);
        let template = query::tasks(
            s.workspace(),
            &TaskQuery {
                project: Some(ids.template),
                ..TaskQuery::default()
            },
            today(),
        );
        // Four tasks and the milestone "Abgabe Senatskanzlei".
        assert_eq!(template[0].tasks.len(), 5);
        s.run(
            "project.archive",
            &json!({"project": ids.case.to_string()}),
            Origin::User,
        )
        .unwrap();
        let live = query::tasks(
            s.workspace(),
            &TaskQuery {
                include_done: true,
                ..TaskQuery::default()
            },
            today(),
        );
        assert!(live[0].tasks.iter().all(|t| t.project == ids.project));
    }

    #[test]
    fn finished_projects_report_nothing_overdue_anywhere() {
        let (mut s, ids) = setup();
        s.run(
            "project.update",
            &json!({"project": ids.project.to_string(), "status": "done"}),
            Origin::User,
        )
        .unwrap();
        assert!(query::overdue(s.workspace(), today()).is_empty());
        let board = query::board(s.workspace(), ids.project, today()).unwrap();
        assert!(
            board
                .columns
                .iter()
                .flat_map(|c| &c.tasks)
                .all(|t| !t.overdue)
        );
        let detail = s
            .query("project", &json!({"project": ids.project.to_string()}))
            .unwrap();
        assert!(
            detail["milestones"]
                .as_array()
                .unwrap()
                .iter()
                .all(|m| m["state"] != "missed")
        );
        let card = query::overview(s.workspace(), today())
            .into_iter()
            .find(|c| c.id == ids.project)
            .unwrap();
        assert_eq!(card.overdue_tasks, 0);
        assert_ne!(
            card.next_milestone.map(|m| m.state),
            Some(MilestoneState::Missed)
        );
    }

    #[test]
    fn rank_order_without_project_keeps_each_project_together() {
        let (s, ids) = setup();
        let all = query::tasks(
            s.workspace(),
            &TaskQuery {
                include_done: true,
                ..TaskQuery::default()
            },
            today(),
        );
        let mut seen = Vec::new();
        for task in &all[0].tasks {
            if seen.last() != Some(&task.project) {
                assert!(
                    !seen.contains(&task.project),
                    "tasks of a project are split up"
                );
                seen.push(task.project);
            }
        }
        assert_eq!(seen.len(), 2);
        assert!(seen.contains(&ids.project) && seen.contains(&ids.case));
    }

    #[test]
    fn history_shows_field_changes_with_origin() {
        let (mut s, ids) = setup();
        let tender = s
            .workspace()
            .tasks()
            .find(|t| t.title == "Ausschreibung vorbereiten")
            .unwrap()
            .id;
        s.run(
            "task.set_progress",
            &json!({"task": tender.to_string(), "progress": 80}),
            Origin::Assistant,
        )
        .unwrap();
        let entries = query::history(s.log(), EntityKey::Task(tender));
        assert_eq!(entries.first().map(|e| e.action), Some(Action::Created));
        let last = entries.last().unwrap();
        assert_eq!(last.action, Action::Changed);
        assert_eq!(last.origin, Origin::Assistant);
        assert_eq!(last.actor, Some(ids.tobias));
        assert_eq!(last.changes.len(), 1);
        assert_eq!(
            (
                last.changes[0].field.as_str(),
                &last.changes[0].before,
                &last.changes[0].after
            ),
            ("progress", &json!(60), &json!(80))
        );
    }

    #[test]
    fn queries_run_by_name_with_checked_params() {
        let (s, ids) = setup();
        for spec in query_registry() {
            assert!(spec.summary.ends_with('.'), "{}", spec.name);
        }
        let board = s
            .query("board", &json!({"project": ids.project.to_string()}))
            .unwrap();
        assert_eq!(board["columns"].as_array().map(Vec::len), Some(5));
        let detail = s
            .query("project", &json!({"project": ids.case.to_string()}))
            .unwrap();
        assert_eq!(detail["project"]["reference"], "23/1234");
        let history = s
            .query(
                "history",
                &json!({"kind": "project", "id": ids.case.to_string()}),
            )
            .unwrap();
        assert_eq!(history[0]["command"], "project.instantiate");
        assert!(matches!(
            s.query("nope", &json!({})),
            Err(CommandError::UnknownCommand { .. })
        ));
        assert!(matches!(
            s.query("board", &json!({})),
            Err(CommandError::BadParam { .. })
        ));
        assert!(matches!(
            s.query("history", &json!({"kind": "task", "id": "x"})),
            Err(CommandError::BadParam { .. })
        ));
        let ghost = plnwrk_core::ProjectId::from_ulid(ulid::Ulid(999_999)).to_string();
        assert!(matches!(
            s.query("board", &json!({"project": ghost})),
            Err(CommandError::NotFound { .. })
        ));
        let found = query::search_queries("overdue deadline");
        assert_eq!(found.first().map(|q| q.name), Some("overdue"));
    }

    #[test]
    fn task_detail_has_links_comments_and_history() {
        let (s, _) = setup();
        let tender = s
            .workspace()
            .tasks()
            .find(|t| t.title == "Ausschreibung vorbereiten")
            .unwrap()
            .id;
        let detail = s
            .query("task", &json!({"task": tender.to_string()}))
            .unwrap();
        assert_eq!(detail["subtasks"].as_array().map(Vec::len), Some(2));
        assert_eq!(detail["blocks"][0]["title"], "Einbau Wache 11");
        assert_eq!(detail["comments"].as_array().map(Vec::len), Some(1));
        assert_eq!(detail["history"][0]["action"], "created");
    }

    fn snapshot_value(s: &Session, name: &str, params: Value) -> Value {
        s.query(name, &params).unwrap()
    }

    #[test]
    fn snapshots() {
        let (s, ids) = setup();
        let project = json!({"project": ids.project.to_string()});
        insta::assert_json_snapshot!("overview", snapshot_value(&s, "overview", json!({})));
        insta::assert_json_snapshot!("templates", snapshot_value(&s, "templates", json!({})));
        insta::assert_json_snapshot!("board", snapshot_value(&s, "board", project.clone()));
        insta::assert_json_snapshot!("overdue", snapshot_value(&s, "overdue", json!({})));
        insta::assert_json_snapshot!(
            "tasks_by_status",
            snapshot_value(
                &s,
                "tasks",
                json!({"project": ids.project.to_string(), "group": "status", "include_done": true})
            )
        );
        let case = json!({"project": ids.case.to_string()});
        insta::assert_json_snapshot!("case", snapshot_value(&s, "project", case));
    }
}

#[cfg(test)]
mod deadline_times {
    use plnwrk_core::{FixedClock, SequentialIds};
    use plnwrk_engine::query::{self, is_past};
    use plnwrk_engine::{Origin, Session};
    use serde_json::json;

    fn at(time: &str) -> jiff::civil::DateTime {
        format!("2026-10-14T{time}").parse().unwrap()
    }

    #[test]
    fn a_deadline_with_a_time_passes_at_that_minute_one_without_at_midnight() {
        let day: jiff::civil::Date = "2026-10-14".parse().unwrap();
        let two = Some("14:00".parse().unwrap());
        assert!(!is_past(day, two, at("13:59")));
        assert!(!is_past(day, two, at("14:00")));
        assert!(is_past(day, two, at("14:01")));
        assert!(!is_past(day, None, at("23:59")));
        assert!(is_past(day, None, "2026-10-15T00:00".parse().unwrap()));
    }

    #[test]
    fn on_one_day_a_time_comes_before_the_whole_day() {
        use plnwrk_engine::query::deadline_order;
        let day: jiff::civil::Date = "2026-10-14".parse().unwrap();
        let next: jiff::civil::Date = "2026-10-15".parse().unwrap();
        let ten = Some("10:00".parse().unwrap());
        let mut order = [
            ("none", None, None),
            ("next", Some(next), None),
            ("whole day", Some(day), None),
            ("ten", Some(day), ten),
        ];
        order.sort_by_key(|o| deadline_order(o.1, o.2));
        let names: Vec<&str> = order.iter().map(|o| o.0).collect();
        assert_eq!(names, ["ten", "whole day", "next", "none"]);
    }

    #[test]
    fn the_overdue_list_follows_the_time_of_day() {
        let clock = FixedClock::at("2026-10-14")
            .unwrap()
            .at_time("13:00")
            .unwrap();
        let mut s = Session::new(Box::new(clock), Box::new(SequentialIds::default()));
        let created = s
            .run("project.create", &json!({"title": "P"}), Origin::User)
            .unwrap()
            .created;
        let Some(plnwrk_core::EntityKey::Project(p)) = created.first().copied() else {
            panic!("{created:?}");
        };
        s.run(
            "task.create",
            &json!({"project": p.to_string(), "title": "Bis 14 Uhr", "due": "2026-10-14T14:00"}),
            Origin::User,
        )
        .unwrap();
        assert!(query::overdue(s.workspace(), at("13:59")).is_empty());
        let late = query::overdue(s.workspace(), at("14:01"));
        assert_eq!(late.len(), 1);
        assert_eq!(
            late[0].due_time.map(|t| t.to_string()).as_deref(),
            Some("14:00:00")
        );
        // The query through the session uses the clock: 13:00, not yet overdue.
        let through_session = s.query("overdue", &json!({})).unwrap();
        assert_eq!(through_session, json!([]));
    }
}
