//! New dates for one task, with everything that waits for it moving along, as the parameters
//! of one `task.reschedule` (one undo step). Shared by the user interface and the MCP server.

use jiff::civil::{Date, Time};
use plnwrk_core::{TaskId, Workspace};
use serde_json::{Map, Value};

use crate::{Calendar, Move, Plan};

/// A task getting new dates: every task that moves, the task itself first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reschedule {
    pub task: TaskId,
    pub moves: Vec<Move>,
}

impl Reschedule {
    /// `task` gets `start` and `due` in `workspace`; whatever waits for it moves later as far
    /// as needed (see [`Plan::push`]). `None` if there is no such task.
    pub fn of(
        workspace: &Workspace,
        task: TaskId,
        start: Option<Date>,
        due: Option<Date>,
        calendar: Calendar,
    ) -> Option<Self> {
        let project = workspace.task(task)?.project;
        let own = Move { task, start, due };
        let mut moves = Plan::of(workspace, project, calendar).push(own);
        if !moves.iter().any(|m| m.task == task) {
            moves.insert(0, own);
        }
        Some(Self { task, moves })
    }

    /// How many other tasks move along.
    pub fn along(&self) -> usize {
        self.moves.iter().filter(|m| m.task != self.task).count()
    }

    /// The parameters of `task.reschedule`. `due_time`, if given, sets (or with `None` clears)
    /// the time of the task's own deadline; the others keep theirs.
    pub fn params(&self, due_time: Option<Option<Time>>) -> Value {
        let date = |d: Option<Date>| d.map_or(Value::Null, |d| Value::String(d.to_string()));
        let moves = self
            .moves
            .iter()
            .map(|m| {
                let mut entry = Map::new();
                entry.insert("task".to_owned(), Value::String(m.task.to_string()));
                entry.insert("start".to_owned(), date(m.start));
                entry.insert("due".to_owned(), date(m.due));
                if m.task == self.task
                    && let Some(time) = due_time
                {
                    let time = time.map_or(Value::Null, |t| {
                        Value::String(format!("{:02}:{:02}", t.hour(), t.minute()))
                    });
                    entry.insert("due_time".to_owned(), time);
                }
                Value::Object(entry)
            })
            .collect();
        let mut params = Map::new();
        params.insert("moves".to_owned(), Value::Array(moves));
        Value::Object(params)
    }
}
