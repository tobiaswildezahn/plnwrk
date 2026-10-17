//! What a command would change at milestones, found by running it on a copy (Phase 9d): every
//! change of a milestone is confirmed first – in the app by a question, over MCP by
//! `"confirmed": true` after asking the person in the chat. One rule for the app, MCP and CLI.
//!
//! Not asked for (decision 2026-10-17, plan §6.3): creating a milestone, undo and redo, and
//! what does not touch a milestone's own data (assignee, description, comments, links).

use jiff::civil::{Date, Time};
use plnwrk_core::{TaskId, TaskStatus, Workspace};
use serde::Serialize;

/// The commands that can change an existing milestone; for every other one nothing is run.
pub const TOUCHING: [&str; 5] = [
    "task.update",
    "task.set_status",
    "task.move",
    "task.delete",
    "task.reschedule",
];

/// One field of a milestone, before and after.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "field", rename_all = "snake_case")]
pub enum Field {
    Title {
        old: String,
        new: String,
    },
    Start {
        old: Option<Date>,
        new: Option<Date>,
    },
    Due {
        old: Option<(Date, Option<Time>)>,
        new: Option<(Date, Option<Time>)>,
    },
    Status {
        old: TaskStatus,
        new: TaskStatus,
    },
    /// It is no longer a milestone.
    Unmarked,
    /// It becomes a milestone.
    Marked,
    Deleted,
}

/// What a command would do to one milestone, and what follows from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MilestoneChange {
    pub task: TaskId,
    /// Its title before the change.
    pub title: String,
    pub fields: Vec<Field>,
    /// Tasks it waits for whose deadline would lie after its new date.
    pub late_predecessors: usize,
    /// Its new date would lie after the project's deadline.
    pub after_project_due: bool,
}

/// Compares the workspace before and after a command: every milestone (before or after) whose
/// own data differs.
pub fn compare(before: &Workspace, after: &Workspace) -> Vec<MilestoneChange> {
    let mut changes = Vec::new();
    for old in before.tasks() {
        let new = after.task(old.id);
        if !old.milestone && !new.is_some_and(|t| t.milestone) {
            continue;
        }
        let mut fields = Vec::new();
        match new {
            None => fields.push(Field::Deleted),
            Some(new) => {
                if old.milestone && !new.milestone {
                    fields.push(Field::Unmarked);
                }
                if !old.milestone && new.milestone {
                    fields.push(Field::Marked);
                }
                if old.title != new.title {
                    fields.push(Field::Title {
                        old: old.title.clone(),
                        new: new.title.clone(),
                    });
                }
                if old.start != new.start {
                    fields.push(Field::Start {
                        old: old.start,
                        new: new.start,
                    });
                }
                let due = |t: &plnwrk_core::Task| t.due.map(|d| (d, t.due_time));
                if due(old) != due(new) {
                    fields.push(Field::Due {
                        old: due(old),
                        new: due(new),
                    });
                }
                if old.status != new.status {
                    fields.push(Field::Status {
                        old: old.status,
                        new: new.status,
                    });
                }
            }
        }
        if fields.is_empty() {
            continue;
        }
        let new_due = new.and_then(|t| t.due);
        let late_predecessors = new_due.map_or(0, |due| {
            after
                .dependencies()
                .filter(|d| d.to == old.id)
                .filter_map(|d| after.task(d.from))
                .filter(|p| p.status != TaskStatus::Done && p.due.is_some_and(|pd| pd > due))
                .count()
        });
        let after_project_due = match (new_due, after.project(old.project).and_then(|p| p.due)) {
            (Some(due), Some(project_due)) => due > project_due,
            _ => false,
        };
        changes.push(MilestoneChange {
            task: old.id,
            title: old.title.clone(),
            fields,
            late_predecessors,
            after_project_due,
        });
    }
    changes
}
