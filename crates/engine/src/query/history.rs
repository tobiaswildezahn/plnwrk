//! The history of one entity, read from the log: who changed what, when, by which command.

use jiff::Timestamp;
use plnwrk_core::{Entity, EntityKey, PersonId, TaskId, TaskStatus};
use serde::Serialize;
use serde_json::Value;

use crate::{Change, Event, Origin};

/// What happened to the entity in one change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// It came into existence.
    Created,
    /// Fields changed.
    Changed,
    /// It was removed.
    Removed,
}

/// One changed field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FieldChange {
    /// Field name as in JSON.
    pub field: String,
    /// Value before (`null` if none).
    pub before: Value,
    /// Value after (`null` if none).
    pub after: Value,
}

/// One entry of the history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HistoryEntry {
    /// The log entry.
    pub seq: u64,
    /// When.
    pub at: Timestamp,
    /// Person, assistant or script.
    pub origin: Origin,
    /// Who was working.
    pub actor: Option<PersonId>,
    /// The command.
    pub command: String,
    /// What happened.
    pub action: Action,
    /// Changed fields (only for `changed`).
    pub changes: Vec<FieldChange>,
}

fn fields(entity: &Entity) -> serde_json::Map<String, Value> {
    match serde_json::to_value(entity) {
        Ok(Value::Object(map)) => map,
        _ => serde_json::Map::new(),
    }
}

fn diff(before: &Entity, after: &Entity) -> Vec<FieldChange> {
    let old = fields(before);
    let new = fields(after);
    let mut names: Vec<&String> = old.keys().chain(new.keys()).collect();
    names.sort();
    names.dedup();
    names
        .into_iter()
        .filter(|name| name.as_str() != "entity")
        .filter_map(|name| {
            let before = old.get(name).cloned().unwrap_or(Value::Null);
            let after = new.get(name).cloned().unwrap_or(Value::Null);
            (before != after).then(|| FieldChange {
                field: name.clone(),
                before,
                after,
            })
        })
        .collect()
}

/// Every change that touched `key`, oldest first.
pub fn history(log: &[Change], key: EntityKey) -> Vec<HistoryEntry> {
    let mut entries = Vec::new();
    for change in log {
        for event in change.events.iter().filter(|e| e.key() == key) {
            let (action, changes) = match event {
                Event::Put { before: None, .. } => (Action::Created, Vec::new()),
                Event::Put {
                    before: Some(before),
                    after,
                } => (Action::Changed, diff(before, after)),
                Event::Remove { .. } => (Action::Removed, Vec::new()),
            };
            entries.push(HistoryEntry {
                seq: change.seq,
                at: change.at,
                origin: change.origin,
                actor: change.actor,
                command: change.command.clone(),
                action,
                changes,
            });
        }
    }
    entries
}

/// When `task` last became done (or was created done), as the log says; `None` if it never
/// did there, or if it came done out of converting an older file (`workspace.upgrade`) – that
/// day is not when it was reached. Asked only of a task that is done now.
pub fn done_at(log: &[Change], task: TaskId) -> Option<Timestamp> {
    let done = |entity: &Entity| matches!(entity, Entity::Task(t) if t.status == TaskStatus::Done);
    let key = EntityKey::Task(task);
    log.iter()
        .rev()
        .find_map(|change| {
            let became =
                change
                    .events
                    .iter()
                    .filter(|e| e.key() == key)
                    .find(|event| match event {
                        Event::Put { before, after } => {
                            done(after) && !before.as_deref().is_some_and(done)
                        }
                        Event::Remove { .. } => false,
                    })?;
            let converted = matches!(became, Event::Put { before: None, .. })
                && change.command == crate::commands::workspace::UPGRADE.id;
            Some((!converted).then_some(change.at))
        })
        .flatten()
}
