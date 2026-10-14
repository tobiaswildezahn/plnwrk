//! Why a command or query was refused. Serializable, so the CLI, MCP and the assistant can hand
//! the reason to an agent unchanged.

use plnwrk_core::{EntityKey, Violation};
use serde::Serialize;

/// Why a command did not change anything.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum CommandError {
    /// No command with this id.
    #[error("unknown command {command:?}")]
    UnknownCommand {
        /// What was asked for.
        command: String,
    },
    /// The command exists but cannot run now (e.g. nothing to undo).
    #[error("{command} is not possible now: {reason}")]
    Disabled {
        /// The command.
        command: String,
        /// Why not.
        reason: String,
    },
    /// A parameter is missing, unknown or has a wrong value.
    #[error("parameter {param}: {reason}")]
    BadParam {
        /// The parameter.
        param: String,
        /// What is wrong.
        reason: String,
    },
    /// The entity does not exist.
    #[error("{entity} does not exist")]
    NotFound {
        /// The missing entity.
        entity: EntityKey,
    },
    /// The result would break rules of the model.
    #[error("the change would break {} rule(s): {}", violations.len(), first_message(violations))]
    Invalid {
        /// Every broken rule.
        violations: Vec<Violation>,
    },
    /// The stored data is not what the change expected (e.g. undo after outside edits).
    #[error("conflict: {reason}")]
    Conflict {
        /// What did not match.
        reason: String,
    },
}

fn first_message(violations: &[Violation]) -> String {
    violations
        .first()
        .map(|v| format!("{}: {}", v.entity, v.message))
        .unwrap_or_default()
}

impl CommandError {
    /// A [`CommandError::BadParam`].
    pub fn bad(param: &str, reason: impl Into<String>) -> Self {
        Self::BadParam {
            param: param.to_owned(),
            reason: reason.into(),
        }
    }
}
