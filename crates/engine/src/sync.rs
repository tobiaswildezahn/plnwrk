//! Several people on one shared file (Phase 10a, docs/plans/2026-10-19-phase-10a-gemeinsame-datei.md):
//! own changes since the last common state are set onto what the file holds now – field by
//! field, the later save winning where both changed the same thing (PLAN §3.4).
//!
//! Pure: the file's state and log come in, the merged state and log go out. Events carry the
//! value before and after, so nothing is run again; every merged event starts from what the
//! file holds, and the result can be checked like any file (chain, replay, rules).

use std::collections::{BTreeMap, BTreeSet};

use plnwrk_core::{Entity, EntityKey, PersonId, Violation, Workspace};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::{Change, Event};

/// Fields that only make sense together: one person's value of all of them is taken, never a
/// mix (a start of one and a deadline of the other, a parent in another project).
const TOGETHER: &[&[&str]] = &[&["start", "due", "due_time"], &["project", "parent"]];

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
}

/// One own change that could not be taken.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Lost {
    /// Its number before the merge.
    pub seq: u64,
    /// Its command, e.g. `task.update`.
    pub command: String,
    /// Why.
    pub dropped: Dropped,
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
                    lost.push(Lost {
                        seq: change.seq,
                        command: change.command.clone(),
                        dropped: Dropped::Follows { on: seq },
                    });
                    continue;
                }
            },
            other => other,
        };
        let mut found = Vec::new();
        let events = match merged_events(&ws, &change.events, &foreign, &mut found) {
            Ok(events) => events,
            Err(entity) => {
                lost.push(Lost {
                    seq: change.seq,
                    command: change.command.clone(),
                    dropped: Dropped::Gone { entity },
                });
                continue;
            }
        };
        if events.is_empty() {
            continue;
        }
        let before = ws.clone();
        for event in &events {
            // Every merged event starts from what is there: applying cannot conflict.
            if event.apply(&mut ws).is_err() {
                ws = before.clone();
                break;
            }
        }
        let violations = ws.check();
        if !violations.is_empty() {
            ws = before;
            lost.push(Lost {
                seq: change.seq,
                command: change.command.clone(),
                dropped: Dropped::Invalid { violations },
            });
            continue;
        }
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
            (Event::Put { before: None, after }, None) => Some(Event::Put {
                before: None,
                after: after.clone(),
            }),
            (Event::Put { before: None, .. }, Some(_)) => return Err(key),
            (Event::Put { before: Some(_), .. }, None) => return Err(key),
            (Event::Put {
                before: Some(before),
                after,
            }, Some(current)) => {
                let (entity, found) = merge_fields(before, after, &current, foreign);
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
            .map_or_else(|| vec![name.clone()], |g| g.iter().map(|f| (*f).to_owned()).collect());
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
