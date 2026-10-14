//! Events: the smallest recorded change. Each carries the value before and after, so every event
//! can be inverted without looking anything up – that makes undo, the audit trail and later
//! synchronisation one mechanism (docs/PLAN.md §3.3).

use jiff::Timestamp;
use plnwrk_core::{Entity, EntityKey, PersonId, Workspace};
use serde::{Deserialize, Serialize};

use crate::CommandError;

/// One stored or removed entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    /// An entity was created (`before` is `None`) or replaced.
    Put {
        /// The value before, if there was one.
        before: Option<Box<Entity>>,
        /// The value after.
        after: Box<Entity>,
    },
    /// An entity was removed.
    Remove {
        /// The value it had.
        before: Box<Entity>,
    },
}

impl Event {
    /// The entity the event is about.
    pub fn key(&self) -> EntityKey {
        match self {
            Self::Put { after, .. } => after.key(),
            Self::Remove { before } => before.key(),
        }
    }

    /// The event that undoes this one.
    pub fn inverse(&self) -> Self {
        match self {
            Self::Put {
                before: None,
                after,
            } => Self::Remove {
                before: after.clone(),
            },
            Self::Put {
                before: Some(before),
                after,
            } => Self::Put {
                before: Some(after.clone()),
                after: before.clone(),
            },
            Self::Remove { before } => Self::Put {
                before: None,
                after: before.clone(),
            },
        }
    }

    /// Applies the event, if the workspace holds exactly the `before` value.
    pub fn apply(&self, ws: &mut Workspace) -> Result<(), CommandError> {
        let key = self.key();
        let current = ws.get(key);
        let expected = match self {
            Self::Put { before, .. } => before.as_deref(),
            Self::Remove { before } => Some(&**before),
        };
        if current.as_ref() != expected {
            return Err(CommandError::Conflict {
                reason: format!("{key} is not in the expected state"),
            });
        }
        match self {
            Self::Put { after, .. } => {
                ws.put((**after).clone());
            }
            Self::Remove { .. } => {
                ws.remove(key);
            }
        }
        Ok(())
    }
}

/// Who or what started a change.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    /// A person in the user interface or on the command line.
    #[default]
    User,
    /// The AI assistant or an agent over MCP.
    Assistant,
    /// A script or import.
    Script,
}

/// One successful command: an entry in the append-only log and one undo step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    /// Position in the log, starting at 1.
    pub seq: u64,
    /// When it happened.
    pub at: Timestamp,
    /// Who or what started it.
    pub origin: Origin,
    /// The person working in the session, if known.
    pub actor: Option<PersonId>,
    /// The command id, e.g. `task.create`, `undo`.
    pub command: String,
    /// The parameters as given.
    pub params: serde_json::Value,
    /// What changed, in order.
    pub events: Vec<Event>,
    /// For `undo`: the change it reverts.
    pub undoes: Option<u64>,
    /// For `redo`: the change it repeats.
    pub redoes: Option<u64>,
}
