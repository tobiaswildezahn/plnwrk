//! `undo` and `redo`.

use crate::Session;
use crate::command::{CommandSpec, Runner};

fn can_undo(session: &Session) -> Result<(), String> {
    if session.can_undo() {
        Ok(())
    } else {
        Err("nothing to undo".to_owned())
    }
}

fn can_redo(session: &Session) -> Result<(), String> {
    if session.can_redo() {
        Ok(())
    } else {
        Err("nothing to redo".to_owned())
    }
}

/// `undo`
pub const UNDO: CommandSpec = CommandSpec {
    id: "undo",
    summary: "Reverts the latest change; the log keeps both.",
    params: &[],
    enabled: can_undo,
    run: Runner::Undo,
};

/// `redo`
pub const REDO: CommandSpec = CommandSpec {
    id: "redo",
    summary: "Repeats the latest reverted change.",
    params: &[],
    enabled: can_redo,
    run: Runner::Redo,
};
