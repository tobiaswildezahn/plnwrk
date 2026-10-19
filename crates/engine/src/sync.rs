//! Several people on one shared file (Phase 10a, docs/plans/2026-10-19-phase-10a-gemeinsame-datei.md):
//! own changes since the last common state are set onto what the file holds now – field by
//! field, the later save winning where both changed the same thing (PLAN §3.4).
//!
//! Pure: the file's state and log come in, the merged state and log go out. Events carry the
//! value before and after, so nothing is run again; every merged event starts from what the
//! file holds, and the result can be checked like any file (chain, replay, rules).

use std::collections::{BTreeMap, BTreeSet};

use jiff::tz::TimeZone;
use plnwrk_core::{
    Entity, EntityKey, FixedClock, PersonId, Rank, SequentialIds, Violation, Workspace,
};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::{Change, Event, Session};

/// Fields that only make sense together: one person's value of all of them is taken, never a
/// mix (a start of one and a deadline of the other, a parent in another project).
const TOGETHER: &[&[&str]] = &[
    &["start", "due", "due_time"],
    &["project", "parent", "milestone"],
    &["name", "initials"],
];

/// The file does not continue the state the own changes start from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotContinued;

/// Both changed the same thing: the own value was kept (the later save wins).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Conflict {
    /// What both changed.
    pub entity: EntityKey,
    /// The fields, e.g. `["due"]` or `["start", "due", "due_time"]`.
    pub fields: Vec<String>,
    /// Their values, field by field, as the file held them.
    pub theirs: Map<String, Value>,
    /// The own values that were kept.
    pub ours: Map<String, Value>,
    /// Who last changed it in the file, if known.
    pub by: Option<PersonId>,
}

/// Why an own change could not be taken.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "why", rename_all = "snake_case")]
pub enum Dropped {
    /// What it changed was removed in the file meanwhile.
    Gone {
        /// The removed entity.
        entity: EntityKey,
    },
    /// Taken together with the file, it would break a rule (e.g. a circle of dependencies).
    Invalid {
        /// The rules broken.
        violations: Vec<Violation>,
    },
    /// It undid or repeated a change that could not be taken itself.
    Follows {
        /// The change it depended on (own numbering before the merge).
        on: u64,
    },
    /// It undid or repeated a change whose result someone else has changed since: undo and
    /// redo are taken only exactly, never merged (they would turn back someone's newer value).
    Changed {
        /// What was changed since.
        entity: EntityKey,
    },
}

/// One own change that could not be taken.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Lost {
    /// Its number before the merge.
    pub seq: u64,
    /// Its command, e.g. `task.update`.
    pub command: String,
    /// What was typed for it, if anything worth keeping (a title, a comment), to say it again.
    pub text: Option<String>,
    /// Why.
    pub dropped: Dropped,
}

impl Lost {
    fn of(change: &Change, dropped: Dropped) -> Self {
        let text = ["body", "title", "name"]
            .iter()
            .find_map(|key| change.params.get(*key).and_then(Value::as_str))
            .map(str::to_owned);
        Self {
            seq: change.seq,
            command: change.command.clone(),
            text,
            dropped,
        }
    }
}

/// The merged state.
#[derive(Debug, Clone)]
pub struct Rebased {
    /// The data after the merge.
    pub workspace: Workspace,
    /// The file's log followed by the own changes taken, numbered on.
    pub log: Vec<Change>,
    /// Own changes taken: number before → number now.
    pub moved: BTreeMap<u64, u64>,
    /// The changes others saved since the common state.
    pub foreign: Vec<Change>,
    /// Where both changed the same thing.
    pub conflicts: Vec<Conflict>,
    /// Own changes that could not be taken.
    pub lost: Vec<Lost>,
}

/// Sets the own changes (`ours[base..]`) onto the file (`file_ws`, `file_log`), whose log must
/// begin with `ours[..base]`. Own changes that change nothing any more (the file already holds
/// it) are left out silently.
pub fn rebase(
    file_ws: Workspace,
    file_log: Vec<Change>,
    ours: &[Change],
    base: usize,
) -> Result<Rebased, NotContinued> {
    let (Some(common), Some(own)) = (ours.get(..base), ours.get(base..)) else {
        return Err(NotContinued);
    };
    if file_log.get(..base) != Some(common) {
        return Err(NotContinued);
    }
    let foreign: Vec<Change> = file_log.get(base..).unwrap_or_default().to_vec();
    let mut ws = file_ws;
    let mut log = file_log;
    let mut moved = BTreeMap::new();
    let mut conflicts = Vec::new();
    let mut lost = Vec::new();
    for change in own {
        // Undo and redo follow what they refer to: one not taken takes them along.
        let refers = change.undoes.or(change.redoes);
        let target = match refers {
            Some(seq) if seq > base as u64 => match moved.get(&seq) {
                Some(now) => Some(*now),
                None => {
                    lost.push(Lost::of(change, Dropped::Follows { on: seq }));
                    continue;
                }
            },
            other => other,
        };
        let mut found = Vec::new();
        let attempt = match target {
            // Undo and redo: exactly the inverse or the repeat of what they refer to, as it
            // stands now – or nothing.
            Some(target) => strict_events(&ws, &log, target, change.undoes.is_some()),
            None => merged_events(&ws, &change.events, &foreign, &mut found)
                .map_err(|entity| Dropped::Gone { entity }),
        };
        let taken = attempt.and_then(|events| checked(&ws, events));
        // A change that creates nothing (deleting, moving, setting a status) is done again on
        // what the file holds, when its events no longer fit – e.g. a task deleted with
        // subtasks someone added meanwhile.
        let taken = match taken {
            Err(dropped) if target.is_none() => {
                match rerun(&ws, change).map(|events| checked(&ws, events)) {
                    Some(Ok(Some(done))) => Ok(Some(done)),
                    _ => Err(dropped),
                }
            }
            other => other,
        };
        let (events, after) = match taken {
            Ok(Some(done)) => done,
            Ok(None) => continue, // nothing left to do: the file already holds it
            Err(dropped) => {
                lost.push(Lost::of(change, dropped));
                continue;
            }
        };
        ws = after;
        let seq = log.last().map_or(1, |c| c.seq.saturating_add(1));
        moved.insert(change.seq, seq);
        conflicts.extend(found);
        log.push(Change {
            seq,
            events,
            undoes: change.undoes.and(target),
            redoes: change.redoes.and(target),
            ..change.clone()
        });
    }
    Ok(Rebased {
        workspace: ws,
        log,
        moved,
        foreign,
        conflicts,
        lost,
    })
}

/// `events` applied to a copy of `ws`, checked against every rule: the events and the new
/// state, `None` if there is nothing to do, or why not.
fn checked(ws: &Workspace, events: Vec<Event>) -> Result<Option<(Vec<Event>, Workspace)>, Dropped> {
    if events.is_empty() {
        return Ok(None);
    }
    let mut after = ws.clone();
    for event in &events {
        if event.apply(&mut after).is_err() {
            return Err(Dropped::Changed {
                entity: event.key(),
            });
        }
    }
    let violations = after.check();
    if !violations.is_empty() {
        return Err(Dropped::Invalid { violations });
    }
    Ok(Some((events, after)))
}

/// The events of an undo (`undo`) or redo of `target` as it stands in `log` now; refused when
/// someone changed what it would set back or repeat.
fn strict_events(
    ws: &Workspace,
    log: &[Change],
    target: u64,
    undo: bool,
) -> Result<Vec<Event>, Dropped> {
    let Some(found) = log.iter().find(|c| c.seq == target) else {
        return Err(Dropped::Follows { on: target });
    };
    let events: Vec<Event> = if undo {
        found.events.iter().rev().map(Event::inverse).collect()
    } else {
        found.events.clone()
    };
    let mut probe = ws.clone();
    for event in &events {
        if event.apply(&mut probe).is_err() {
            return Err(Dropped::Changed {
                entity: event.key(),
            });
        }
    }
    Ok(events)
}

/// The command of `change` run again on `ws` – only when it creates nothing (new ids would not
/// match what later own changes refer to).
fn rerun(ws: &Workspace, change: &Change) -> Option<Vec<Event>> {
    if matches!(
        change.command.as_str(),
        "undo" | "redo" | "workspace.upgrade"
    ) {
        return None;
    }
    let zoned = change.at.to_zoned(TimeZone::UTC);
    let clock = FixedClock {
        now: change.at,
        today: zoned.date(),
        local: zoned.datetime(),
    };
    let mut session = Session::with_workspace(
        ws.clone(),
        Vec::new(),
        Box::new(clock),
        Box::new(SequentialIds::default()),
    );
    session.set_actor(change.actor);
    match session.run(&change.command, &change.params, change.origin) {
        Ok(outcome) if outcome.created.is_empty() => Some(
            session
                .log()
                .last()
                .map(|c| c.events.clone())
                .unwrap_or_default(),
        ),
        _ => None,
    }
}

/// The events of one own change, starting from what `ws` holds now; the entity it needs, if
/// it is gone. Events applied in order, so later events see earlier ones.
fn merged_events(
    ws: &Workspace,
    events: &[Event],
    foreign: &[Change],
    conflicts: &mut Vec<Conflict>,
) -> Result<Vec<Event>, EntityKey> {
    let mut ws = ws.clone();
    let mut out = Vec::new();
    for event in events {
        let key = event.key();
        let current = ws.get(key);
        let merged = match (event, current) {
            // Created by me: ids are new, nobody else can have it.
            (
                Event::Put {
                    before: None,
                    after,
                },
                None,
            ) => Some(Event::Put {
                before: None,
                after: Box::new(own_place(&ws, (**after).clone(), None)),
            }),
            // The file already has it: the same is nothing to do, another is not mine to set.
            (
                Event::Put {
                    before: None,
                    after,
                },
                Some(current),
            ) => {
                if **after == current {
                    None
                } else {
                    return Err(key);
                }
            }
            (
                Event::Put {
                    before: Some(_), ..
                },
                None,
            ) => return Err(key),
            (
                Event::Put {
                    before: Some(before),
                    after,
                },
                Some(current),
            ) => {
                let (entity, found) = merge_fields(before, after, &current, foreign);
                let entity = own_place(&ws, entity, Some(&current));
                if let Some(found) = found {
                    conflicts.push(found);
                }
                (entity != current).then(|| Event::Put {
                    before: Some(Box::new(current)),
                    after: Box::new(entity),
                })
            }
            // Removed by me and already gone: nothing to do.
            (Event::Remove { .. }, None) => None,
            (Event::Remove { .. }, Some(current)) => Some(Event::Remove {
                before: Box::new(current),
            }),
        };
        if let Some(merged) = merged {
            if merged.apply(&mut ws).is_err() {
                return Err(key);
            }
            out.push(merged);
        }
    }
    Ok(out)
}

/// `current` with the fields I changed from `before` to `after` – field by field, coupled
/// fields together – and where the file holds a third value there, the conflict.
fn merge_fields(
    before: &Entity,
    after: &Entity,
    current: &Entity,
    foreign: &[Change],
) -> (Entity, Option<Conflict>) {
    let (Some(before), Some(after), Some(mut merged)) =
        (fields(before), fields(after), fields(current))
    else {
        return (current.clone(), None);
    };
    let theirs_now = merged.clone();
    let names: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    let mut done: BTreeSet<String> = BTreeSet::new();
    let mut fields_in_conflict = Vec::new();
    let mut theirs = Map::new();
    let mut ours = Map::new();
    for name in names {
        if done.contains(name.as_str()) || name == "entity" {
            continue;
        }
        let group: Vec<String> = TOGETHER
            .iter()
            .find(|g| g.contains(&name.as_str()))
            .map_or_else(
                || vec![name.clone()],
                |g| g.iter().map(|f| (*f).to_owned()).collect(),
            );
        done.extend(group.iter().cloned());
        let value = |m: &Map<String, Value>| -> Vec<Value> {
            group
                .iter()
                .map(|f| m.get(f).cloned().unwrap_or(Value::Null))
                .collect()
        };
        let (b, a, c) = (value(&before), value(&after), value(&theirs_now));
        if b == a {
            continue; // not mine: theirs stays
        }
        // Labels are a set: mine added and removed, theirs kept.
        if name == "tags" {
            if let (Some(b), Some(a), Some(c)) = (b.first(), a.first(), c.first()) {
                merged.insert(name.clone(), merge_set(b, a, c));
            }
            continue;
        }
        if c != b && c != a {
            fields_in_conflict.extend(group.iter().cloned());
            for (field, v) in group.iter().zip(&c) {
                theirs.insert(field.clone(), v.clone());
            }
            for (field, v) in group.iter().zip(&a) {
                ours.insert(field.clone(), v.clone());
            }
        }
        for (field, v) in group.iter().zip(a) {
            match v {
                Value::Null if !after.contains_key(field) => {
                    merged.remove(field);
                }
                v => {
                    merged.insert(field.clone(), v);
                }
            }
        }
    }
    let Ok(entity) = serde_json::from_value::<Entity>(Value::Object(merged)) else {
        return (current.clone(), None);
    };
    let conflict = (!fields_in_conflict.is_empty()).then(|| Conflict {
        entity: current.key(),
        fields: fields_in_conflict,
        theirs,
        ours,
        by: last_by(foreign, current.key()),
    });
    (entity, conflict)
}

/// An entity's fields as JSON.
fn fields(entity: &Entity) -> Option<Map<String, Value>> {
    match serde_json::to_value(entity) {
        Ok(Value::Object(map)) => Some(map),
        _ => None,
    }
}

/// Who last changed `key` among the changes others saved.
fn last_by(foreign: &[Change], key: EntityKey) -> Option<PersonId> {
    foreign
        .iter()
        .rev()
        .find(|c| c.events.iter().any(|e| e.key() == key))
        .and_then(|c| c.actor)
}

/// Labels merged as a set: what I removed goes, what I added comes, the rest is theirs.
fn merge_set(before: &Value, after: &Value, current: &Value) -> Value {
    let list = |v: &Value| -> Vec<Value> { v.as_array().cloned().unwrap_or_default() };
    let (before, after, current) = (list(before), list(after), list(current));
    let mut out: Vec<Value> = current
        .into_iter()
        .filter(|v| after.contains(v) || !before.contains(v))
        .collect();
    for v in after {
        if !before.contains(&v) && !out.contains(&v) {
            out.push(v);
        }
    }
    // Labels are kept sorted (a rule of the model).
    out.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
    Value::Array(out)
}

/// A task whose place (rank) another task of the same parent took meanwhile moves right behind
/// it – two equal ranks would make moving between them impossible. Others are left as they are.
fn own_place(ws: &Workspace, entity: Entity, was: Option<&Entity>) -> Entity {
    let Entity::Task(mut task) = entity else {
        return entity;
    };
    // Only a place I set (a new task, a move) gives way; one I did not touch stays.
    let moved = match was {
        Some(Entity::Task(old)) => old.rank != task.rank || old.parent != task.parent,
        _ => true,
    };
    let taken = ws.tasks().any(|t| {
        t.id != task.id
            && t.project == task.project
            && t.parent == task.parent
            && t.rank == task.rank
    });
    if moved && taken {
        let next = ws
            .tasks()
            .filter(|t| t.id != task.id && t.project == task.project && t.parent == task.parent)
            .map(|t| &t.rank)
            .filter(|r| **r > task.rank)
            .min();
        if let Ok(rank) = Rank::between(Some(&task.rank), next) {
            task.rank = rank;
        }
    }
    Entity::Task(task)
}

#[cfg(test)]
mod tests {
    use super::*;
    use plnwrk_core::FixedClock;

    /// Only what creates nothing is done again: new ids would not match what later own changes
    /// refer to.
    #[test]
    fn a_command_that_creates_is_never_run_again() {
        let mut s = Session::with_workspace(
            Workspace::default(),
            Vec::new(),
            Box::new(FixedClock::at("2026-10-19").unwrap()),
            Box::new(SequentialIds::default()),
        );
        s.run(
            "project.create",
            &serde_json::json!({"title": "P"}),
            crate::Origin::User,
        )
        .unwrap();
        let project = s.workspace().projects().next().unwrap().id.to_string();
        let ws = s.workspace().clone();
        s.run(
            "task.create",
            &serde_json::json!({"project": project, "title": "T"}),
            crate::Origin::User,
        )
        .unwrap();
        let create = s.log().last().unwrap().clone();
        assert!(rerun(&ws, &create).is_none(), "creates: not run again");
        let task = s.workspace().tasks().next().unwrap().id.to_string();
        s.run(
            "task.update",
            &serde_json::json!({"task": task, "title": "U"}),
            crate::Origin::User,
        )
        .unwrap();
        let update = s.log().last().unwrap().clone();
        assert!(
            rerun(s.workspace(), &update).is_some(),
            "creates nothing: run again"
        );
    }
}
