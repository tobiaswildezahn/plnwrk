//! Every command against hostile parameters (docs/PLAN.md §2 "Never crash").
//!
//! For each command, random JSON – wrong types, extreme numbers, control characters, real ids
//! of the wrong kind, unknown keys, non-objects – is run on the sample workspace. A run may
//! succeed or fail, but it must never panic; a failure must leave data and log untouched, and a
//! success must leave the workspace consistent.

#[cfg(test)]
mod junk {
    use std::cell::Cell;
    use std::collections::BTreeMap;

    use plnwrk_core::{EntityKind, FixedClock, SequentialIds, Workspace};
    use plnwrk_engine::fixture::sample;
    use plnwrk_engine::params::{ParamKind, ParamSpec};
    use plnwrk_engine::{Origin, Session, registry};
    use proptest::prelude::*;
    use proptest::test_runner::{Config, TestRunner};
    use serde_json::{Map, Value};

    fn fresh() -> Session {
        let mut session = Session::new(
            Box::new(FixedClock::at("2026-10-14").unwrap()),
            Box::new(SequentialIds::default()),
        );
        sample(&mut session).unwrap();
        session
    }

    fn ids(ws: &Workspace) -> Vec<String> {
        let mut ids: Vec<String> = Vec::new();
        ids.extend(ws.projects().map(|x| x.id.to_string()));
        ids.extend(ws.tasks().map(|x| x.id.to_string()));
        ids.extend(ws.milestones().map(|x| x.id.to_string()));
        ids.extend(ws.dependencies().map(|x| x.id.to_string()));
        ids.extend(ws.people().map(|x| x.id.to_string()));
        ids.extend(ws.comments().map(|x| x.id.to_string()));
        ids
    }

    const WORDS: &[&str] = &[
        "",
        " ",
        "todo",
        "in_progress",
        "blocked",
        "review",
        "done",
        "low",
        "normal",
        "high",
        "critical",
        "open",
        "reached",
        "vs_nfd",
        "active",
        "archived",
        "draft",
        "on_hold",
        "finish_to_start",
        "start_to_start",
        "finish_to_finish",
        "2026-10-14",
        "2026-02-30",
        "1999-01-01",
        "9999-12-31",
        "14.10.2026",
        "0000000000000000000000000Z",
        "ZZZZZZZZZZZZZZZZZZZZZZZZZZ",
        "\u{0}",
        "a\nb",
        "\u{202e}rtl",
        "Ärger",
        "true",
        "null",
    ];

    fn leaf(ids: Vec<String>) -> BoxedStrategy<Value> {
        prop_oneof![
            Just(Value::Null),
            any::<bool>().prop_map(Value::from),
            any::<i64>().prop_map(Value::from),
            prop::sample::select(vec![
                0i64,
                -1,
                1,
                100,
                101,
                365,
                366,
                -366,
                1_000_001,
                i64::MAX,
                i64::MIN
            ])
            .prop_map(Value::from),
            any::<f64>()
                .prop_map(|f| serde_json::Number::from_f64(f).map_or(Value::Null, Value::Number)),
            prop::sample::select(WORDS).prop_map(Value::from),
            prop::sample::select(ids).prop_map(Value::from),
            "\\PC{0,40}".prop_map(Value::from),
            Just(Value::from("x".repeat(20_001))),
            Just(Value::from("y".repeat(201))),
        ]
        .boxed()
    }

    fn value(ids: Vec<String>, keys: Vec<&'static str>) -> BoxedStrategy<Value> {
        leaf(ids)
            .prop_recursive(3, 24, 5, move |inner| {
                let keys = keys.clone();
                prop_oneof![
                    prop::collection::vec(inner.clone(), 0..5).prop_map(Value::Array),
                    prop::collection::btree_map(prop::sample::select(keys), inner, 0..4).prop_map(
                        |m| Value::Object(m.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
                    ),
                ]
            })
            .boxed()
    }

    /// Ids of the sample workspace, by kind.
    struct Pools {
        all: Vec<String>,
        by_kind: BTreeMap<EntityKind, Vec<String>>,
    }

    fn pools(ws: &Workspace) -> Pools {
        let mut by_kind: BTreeMap<EntityKind, Vec<String>> = BTreeMap::new();
        let mut add = |kind, id: String| by_kind.entry(kind).or_default().push(id);
        ws.projects()
            .for_each(|x| add(EntityKind::Project, x.id.to_string()));
        ws.tasks()
            .for_each(|x| add(EntityKind::Task, x.id.to_string()));
        ws.milestones()
            .for_each(|x| add(EntityKind::Milestone, x.id.to_string()));
        ws.dependencies()
            .for_each(|x| add(EntityKind::Dependency, x.id.to_string()));
        ws.people()
            .for_each(|x| add(EntityKind::Person, x.id.to_string()));
        ws.comments()
            .for_each(|x| add(EntityKind::Comment, x.id.to_string()));
        Pools {
            all: ids(ws),
            by_kind,
        }
    }

    /// A value of the right kind – mostly valid, sometimes at or past the edges.
    fn typed(kind: ParamKind, pools: &Pools) -> BoxedStrategy<Value> {
        match kind {
            ParamKind::Title => prop::sample::select(vec!["Neue Aufgabe", "  Lage  ", "Ä", "x"])
                .prop_map(Value::from)
                .boxed(),
            ParamKind::LongText => prop::sample::select(vec!["", "Text\nmit Zeilen", "**fett**"])
                .prop_map(Value::from)
                .boxed(),
            ParamKind::Label => prop::sample::select(vec!["SKA", "Vergabe", "23/1234", "A"])
                .prop_map(Value::from)
                .boxed(),
            ParamKind::Date => prop::sample::select(vec![
                "2026-01-01", "2026-10-01", "2026-10-14", "2026-10-20", "2026-12-31", "2025-06-30",
            ])
            .prop_map(Value::from)
            .boxed(),
            ParamKind::Id(entity) => {
                let own = pools.by_kind.get(&entity).cloned().unwrap_or_default();
                let other = pools.all.clone();
                if own.is_empty() {
                    prop::sample::select(other).prop_map(Value::from).boxed()
                } else {
                    prop_oneof![
                        9 => prop::sample::select(own).prop_map(Value::from),
                        1 => prop::sample::select(other).prop_map(Value::from),
                    ]
                    .boxed()
                }
            }
            ParamKind::Choice(names) => prop::sample::select(names.to_vec()).prop_map(Value::from).boxed(),
            ParamKind::Integer { min, max } => prop_oneof![
                4 => (min..=max).prop_map(Value::from),
                1 => prop::sample::select(vec![min, max, min.saturating_sub(1), max.saturating_add(1)])
                    .prop_map(Value::from),
            ]
            .boxed(),
            ParamKind::Bool => any::<bool>().prop_map(Value::from).boxed(),
            ParamKind::Labels => prop::collection::vec(
                prop::sample::select(vec!["a", "b", "Vergabe", " a "]),
                0..4,
            )
            .prop_map(|v| Value::from(v.into_iter().map(Value::from).collect::<Vec<_>>()))
            .boxed(),
        }
    }

    /// Parameters for one command: a valid-looking object with some fields spoiled, dropped
    /// or added – and now and then something that is not an object at all.
    fn params(spec_params: &'static [ParamSpec], pools: &Pools) -> BoxedStrategy<Value> {
        let fields: Vec<BoxedStrategy<Option<(&'static str, Value)>>> = spec_params
            .iter()
            .map(|p| {
                let present = if p.required { 0.95 } else { 0.5 };
                let name = p.name;
                prop::option::weighted(
                    present,
                    prop_oneof![
                        6 => typed(p.kind, pools),
                        1 => leaf(pools.all.clone()),
                    ],
                )
                .prop_map(move |v| v.map(|v| (name, v)))
                .boxed()
            })
            .collect();
        let extra = prop::option::weighted(
            0.05,
            (
                prop::sample::select(vec!["unknown", "Title", "id"]),
                leaf(pools.all.clone()),
            ),
        );
        let object = (fields, extra).prop_map(|(fields, extra)| {
            let mut map = Map::new();
            for (name, value) in fields.into_iter().flatten().chain(extra) {
                map.insert(name.to_owned(), value);
            }
            Value::Object(map)
        });
        let keys: Vec<&'static str> = spec_params.iter().map(|p| p.name).chain(["x"]).collect();
        prop_oneof![
            19 => object,
            1 => value(pools.all.clone(), keys),
        ]
        .boxed()
    }

    #[test]
    fn every_command_survives_junk() {
        let pools = pools(fresh().workspace());
        for spec in registry() {
            let mut runner = TestRunner::new(Config {
                cases: 256,
                failure_persistence: None,
                ..Config::default()
            });
            let successes = Cell::new(0u32);
            let result = runner.run(&params(spec.params, &pools), |input| {
                let mut session = fresh();
                let before = (session.workspace().clone(), session.log().len());
                match session.run(spec.id, &input, Origin::Script) {
                    Ok(_) => {
                        successes.set(successes.get() + 1);
                        prop_assert!(
                            session.workspace().check().is_empty(),
                            "inconsistent after {input}"
                        );
                        prop_assert!(session.log().len() <= before.1 + 1);
                    }
                    Err(_) => {
                        prop_assert_eq!(
                            session.workspace(),
                            &before.0,
                            "data changed by failed {}",
                            input
                        );
                        prop_assert_eq!(session.log().len(), before.1);
                    }
                }
                Ok(())
            });
            if let Err(error) = result {
                panic!("{}: {error}", spec.id);
            }
            // The generator must reach past parameter checks, or the test proves little.
            // `redo` has nothing to redo in a fresh session.
            if spec.id != "redo" {
                assert!(
                    successes.get() >= 5,
                    "{}: only {} successful runs",
                    spec.id,
                    successes.get()
                );
            }
        }
    }

    #[test]
    fn every_query_survives_junk() {
        let pools = pools(fresh().workspace());
        let session = fresh();
        for spec in plnwrk_engine::query::query_registry() {
            let mut runner = TestRunner::new(Config {
                cases: 128,
                failure_persistence: None,
                ..Config::default()
            });
            let result = runner.run(&params(spec.params, &pools), |input| {
                let _ = session.query(spec.name, &input);
                Ok(())
            });
            if let Err(error) = result {
                panic!("{}: {error}", spec.name);
            }
        }
    }
}
