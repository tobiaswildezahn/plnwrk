//! The history of one entity, read from the log: who changed what, when, by which command.

use jiff::Timestamp;
use plnwrk_core::{Entity, EntityKey, PersonId};
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
        .filter(|name| name.as_str() != "kind")
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
