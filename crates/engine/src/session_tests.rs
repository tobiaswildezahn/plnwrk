//! Behaviour of `Session::run`, undo and redo.

use plnwrk_core::{
    Entity, EntityKey, FixedClock, Person, PersonId, Rule, SequentialIds, Workspace,
};
use serde_json::{Value, json};

use crate::command::{CommandSpec, Runner, always};
use crate::params::Params;
use crate::{CommandError, Event, Origin, Session, Tx};

fn session() -> Session {
    Session::new(
        Box::new(FixedClock::at("2026-10-14").unwrap()),
        Box::new(SequentialIds::default()),
    )
}

fn create_person(session: &mut Session, name: &str) -> PersonId {
    let outcome = session
        .run("person.create", &json!({ "name": name }), Origin::User)
        .unwrap();
    match outcome.created.first() {
        Some(EntityKey::Person(id)) => *id,
        other => panic!("no person created: {other:?}"),
    }
}

fn snapshot(session: &Session) -> (Workspace, usize) {
    (session.workspace().clone(), session.log().len())
}

#[test]
fn a_command_changes_data_and_appends_one_change() {
    let mut s = session();
    let id = create_person(&mut s, "Tobias Wildezahn");
    let person = s.workspace().person(id).unwrap();
    assert_eq!(person.initials, "TW");
    assert_eq!(s.log().len(), 1);
    let change = &s.log()[0];
    assert_eq!((change.seq, change.command.as_str()), (1, "person.create"));
    assert_eq!(change.params, json!({"name": "Tobias Wildezahn"}));
    assert_eq!(change.origin, Origin::User);
    assert_eq!(change.at.to_string(), "2026-10-14T00:00:00Z");
}

#[test]
fn undo_and_redo_append_compensating_changes() {
    let mut s = session();
    let id = create_person(&mut s, "Anna");
    s.run("undo", &json!({}), Origin::User).unwrap();
    assert!(s.workspace().person(id).is_none());
    assert_eq!(s.log()[1].undoes, Some(1));
    assert!(s.can_redo() && !s.can_undo());
    s.run("redo", &json!({}), Origin::User).unwrap();
    assert!(s.workspace().person(id).is_some());
    assert_eq!(s.log()[2].redoes, Some(1));
    assert_eq!(s.log().len(), 3);
    s.run("undo", &json!({}), Origin::Assistant).unwrap();
    assert!(s.workspace().person(id).is_none());
    assert_eq!(s.log()[3].origin, Origin::Assistant);
}

#[test]
fn several_undos_go_back_in_order() {
    let mut s = session();
    let a = create_person(&mut s, "A");
    let b = create_person(&mut s, "B");
    s.run(
        "person.update",
        &json!({"person": a.to_string(), "name": "A2"}),
        Origin::User,
    )
    .unwrap();
    s.run("undo", &json!({}), Origin::User).unwrap();
    assert_eq!(s.workspace().person(a).unwrap().name, "A");
    s.run("undo", &json!({}), Origin::User).unwrap();
    assert!(s.workspace().person(b).is_none());
    s.run("undo", &json!({}), Origin::User).unwrap();
    assert!(s.workspace().is_empty());
    let error = s.run("undo", &json!({}), Origin::User).unwrap_err();
    assert!(matches!(error, CommandError::Disabled { .. }));
}

#[test]
fn a_new_command_clears_redo() {
    let mut s = session();
    create_person(&mut s, "A");
    s.run("undo", &json!({}), Origin::User).unwrap();
    create_person(&mut s, "B");
    let before = snapshot(&s);
    let error = s.run("redo", &json!({}), Origin::User).unwrap_err();
    assert!(matches!(error, CommandError::Disabled { .. }));
    assert_eq!(snapshot(&s), before);
}

#[test]
fn unknown_command_and_bad_params_change_nothing() {
    let mut s = session();
    create_person(&mut s, "A");
    let before = snapshot(&s);
    let cases: [(&str, Value); 4] = [
        ("person.fly", json!({})),
        ("person.create", json!([])),
        ("person.create", json!({"name": "B", "nme": "x"})),
        ("person.create", json!({"name": ""})),
    ];
    for (command, params) in cases {
        assert!(
            s.run(command, &params, Origin::User).is_err(),
            "{command} {params}"
        );
        assert_eq!(snapshot(&s), before);
    }
}

#[test]
fn update_without_changes_records_nothing() {
    let mut s = session();
    let a = create_person(&mut s, "A");
    let outcome = s
        .run(
            "person.update",
            &json!({"person": a.to_string(), "name": "A"}),
            Origin::User,
        )
        .unwrap();
    assert_eq!(outcome.change, None);
    assert_eq!(s.log().len(), 1);
}

#[test]
fn update_of_unknown_person_is_not_found() {
    let mut s = session();
    let ghost = PersonId::from_ulid(ulid::Ulid(77));
    let error = s
        .run(
            "person.update",
            &json!({"person": ghost.to_string()}),
            Origin::User,
        )
        .unwrap_err();
    assert_eq!(
        error,
        CommandError::NotFound {
            entity: EntityKey::Person(ghost)
        }
    );
}

fn put_nameless(tx: &mut Tx<'_>, _: &Params<'_>) -> Result<(), CommandError> {
    let id = tx.new_id();
    tx.put(Entity::Person(Person {
        id,
        name: "ok".into(),
        email: None,
        role: None,
        initials: "O".into(),
    }));
    let id = tx.new_id();
    tx.put(Entity::Person(Person {
        id,
        name: String::new(),
        email: None,
        role: None,
        initials: "X".into(),
    }));
    Ok(())
}

#[test]
fn a_result_that_breaks_rules_is_rolled_back_completely() {
    const BROKEN: CommandSpec = CommandSpec {
        id: "test.broken",
        summary: "Writes an invalid person.",
        params: &[],
        enabled: always,
        run: Runner::Edit(put_nameless),
    };
    let mut s = session();
    create_person(&mut s, "A");
    let before = snapshot(&s);
    let error = s.run_spec(&BROKEN, &json!({}), Origin::User).unwrap_err();
    match error {
        CommandError::Invalid { violations } => assert_eq!(violations[0].rule, Rule::Text),
        other => panic!("{other:?}"),
    }
    assert_eq!(snapshot(&s), before);
}

#[test]
fn events_invert_and_detect_conflicts() {
    let person = Entity::Person(Person {
        id: PersonId::from_ulid(ulid::Ulid(1)),
        name: "A".into(),
        email: None,
        role: None,
        initials: "A".into(),
    });
    let created = Event::Put {
        before: None,
        after: Box::new(person.clone()),
    };
    assert_eq!(created.inverse().inverse(), created);
    let mut ws = Workspace::default();
    created.apply(&mut ws).unwrap();
    assert!(matches!(
        created.apply(&mut ws),
        Err(CommandError::Conflict { .. })
    ));
    created.inverse().apply(&mut ws).unwrap();
    assert!(ws.is_empty());
}

#[test]
fn errors_serialize_with_a_tag() {
    let error = CommandError::bad("title", "is required");
    let json = serde_json::to_value(&error).unwrap();
    assert_eq!(json["error"], "bad_param");
    assert_eq!(json["param"], "title");
    assert_eq!(error.to_string(), "parameter title: is required");
}

#[test]
fn error_messages_are_readable() {
    let key = EntityKey::Person(PersonId::from_ulid(ulid::Ulid(1)));
    let cases = [
        (
            CommandError::UnknownCommand {
                command: "x".into(),
            },
            "unknown command \"x\"",
        ),
        (
            CommandError::Disabled {
                command: "undo".into(),
                reason: "nothing to undo".into(),
            },
            "undo is not possible now: nothing to undo",
        ),
        (
            CommandError::NotFound { entity: key },
            "person 00000000000000000000000001 does not exist",
        ),
        (CommandError::Conflict { reason: "r".into() }, "conflict: r"),
    ];
    for (error, text) in cases {
        assert_eq!(error.to_string(), text);
    }
    let invalid = CommandError::Invalid {
        violations: vec![plnwrk_core::Violation {
            entity: key,
            rule: Rule::Text,
            message: "name is not a valid title".into(),
        }],
    };
    assert_eq!(
        invalid.to_string(),
        "the change would break 1 rule(s): person 00000000000000000000000001: name is not a valid title"
    );
}
