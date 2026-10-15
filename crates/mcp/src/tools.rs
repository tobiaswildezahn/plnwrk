//! The tools the MCP server offers: read a workspace (overview, any query), find commands and
//! run one. Every tool names its file; every change is saved at once, is one undo step (two,
//! when dates and other fields of a task change together) and is marked as made by an AI.

use plnwrk_core::{Entity, TaskId};
use plnwrk_engine::params::{self, MOVES_MAX};
use plnwrk_engine::query::query_registry;
use plnwrk_engine::{CommandSpec, Event, Origin, Session, registry, search};
use plnwrk_schedule::{Calendar, Reschedule};
use serde_json::{Map, Value, json};

use crate::files::{self, Changing, Kept, Refusal};

/// A tool's answer: data for the model, or why it failed (shown to the model as an error).
pub type Answer = Result<Value, String>;

/// The tools with their descriptions and input schemas (`tools/list`).
pub fn list() -> Value {
    let file = json!({
        "type": "string",
        "description": "Absolute path of the .plnwrk file (or starting with ~/)."
    });
    let queries: Vec<Value> = query_registry()
        .iter()
        .map(|q| Value::from(q.name))
        .collect();
    let query_list: Vec<String> = query_registry()
        .iter()
        .map(|q| format!("{} ({})", q.name, q.summary))
        .collect();
    json!([
        {
            "name": "overview",
            "title": "Overview of a plnwrk file",
            "description": "Projects and cases in a plnwrk file with status, deadline and progress. Start here to get ids.",
            "inputSchema": {
                "type": "object",
                "properties": {"file": file},
                "required": ["file"],
                "additionalProperties": false
            },
            "annotations": {"readOnlyHint": true}
        },
        {
            "name": "query",
            "title": "Read from a plnwrk file",
            "description": format!(
                "Runs a read-only query. Queries: {}. Use search_commands to learn parameter names; ids come from overview or other queries.",
                query_list.join("; ")
            ),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": file,
                    "name": {"type": "string", "enum": queries},
                    "params": {"type": "object", "description": "Parameters of the query, e.g. {\"project\": \"<id>\"}."}
                },
                "required": ["file", "name"],
                "additionalProperties": false
            },
            "annotations": {"readOnlyHint": true}
        },
        {
            "name": "search_commands",
            "title": "Find plnwrk commands",
            "description": "Lists the commands that change a plnwrk file (create, update, move, delete, undo …) with their purpose and the JSON schema of their parameters. Without words: all of them.",
            "inputSchema": {
                "type": "object",
                "properties": {"words": {"type": "string", "description": "Words to look for, e.g. \"task deadline\"."}},
                "additionalProperties": false
            },
            "annotations": {"readOnlyHint": true}
        },
        {
            "name": "run_command",
            "title": "Change a plnwrk file",
            "description": "Runs one command (see search_commands) and saves the file. Each command is one undo step, marked in the history as made by an AI; \"undo\" and \"redo\" reach the steps made in this conversation, as long as the file was not changed elsewhere in between (never a person's change). Changing start or due of a task with task.update moves every task that waits for it as far later as needed (every calendar day counts, weekends and holidays too), in the same step. Refused for files open in the plnwrk app, encrypted files and anything classified VS-NfD.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": file,
                    "command": {"type": "string", "description": "Command id, e.g. task.update."},
                    "params": {"type": "object", "description": "Parameters as in the command's schema."}
                },
                "required": ["file", "command"],
                "additionalProperties": false
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": true, "idempotentHint": false}
        },
        {
            "name": "move_task",
            "title": "Move a task by days",
            "description": "Moves a task's start and deadline by a number of days (negative: earlier), keeping its length and the time of its deadline; every day counts, a week is 7 days. What waits for it moves along in the same step, as with task.update. Saved at once; one undo step.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": file,
                    "task": {"type": "string", "description": "Id of the task (from overview or a query)."},
                    "days": {"type": "integer", "minimum": -3650, "maximum": 3650, "description": "Days to move by, e.g. 7 for a week."}
                },
                "required": ["file", "task", "days"],
                "additionalProperties": false
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": false, "idempotentHint": false}
        }
    ])
}

/// Runs a tool.
pub fn call(name: &str, arguments: &Map<String, Value>, kept: &mut Kept) -> Answer {
    let allowed: &[&str] = match name {
        "overview" => &["file"],
        "query" => &["file", "name", "params"],
        "search_commands" => &["words"],
        "run_command" => &["file", "command", "params"],
        "move_task" => &["file", "task", "days"],
        _ => return Err(format!("There is no tool {name:?}.")),
    };
    if let Some(unknown) = arguments.keys().find(|k| !allowed.contains(&k.as_str())) {
        return Err(format!(
            "Unknown argument {unknown:?}; {name} takes {}.",
            allowed.join(", ")
        ));
    }
    match name {
        "overview" => read(arguments, |session| {
            session.query("overview", &json!({})).map_err(error)
        }),
        "query" => {
            let query = text(arguments, "name")?;
            let params = object(arguments, "params")?;
            read(arguments, |session| {
                session.query(&query, &params).map_err(error)
            })
        }
        "search_commands" => {
            let specs: Vec<&CommandSpec> = match arguments.get("words") {
                None | Some(Value::Null) => registry().iter().collect(),
                Some(Value::String(words)) => search(words),
                Some(_) => return Err("words must be a string.".to_owned()),
            };
            Ok(Value::from(
                specs
                    .into_iter()
                    .map(|s| json!({"id": s.id, "summary": s.summary, "input_schema": s.input_schema()}))
                    .collect::<Vec<_>>(),
            ))
        }
        "move_task" => move_task(arguments, kept),
        _ => run(arguments, kept),
    }
}

/// A string argument.
fn text(arguments: &Map<String, Value>, name: &str) -> Result<String, String> {
    match arguments.get(name) {
        Some(Value::String(value)) => Ok(value.clone()),
        Some(_) => Err(format!("{name} must be a string.")),
        None => Err(format!("{name} is missing.")),
    }
}

/// An optional object argument (`{}` if missing).
fn object(arguments: &Map<String, Value>, name: &str) -> Result<Value, String> {
    match arguments.get(name) {
        None | Some(Value::Null) => Ok(json!({})),
        Some(value @ Value::Object(_)) => Ok(value.clone()),
        Some(_) => Err(format!("{name} must be an object.")),
    }
}

/// A command or query error as JSON text for the model.
fn error(error: plnwrk_engine::CommandError) -> String {
    serde_json::to_value(&error).map_or_else(|_| error.to_string(), |v| v.to_string())
}

/// Opens the named file to read and answers from it.
fn read(arguments: &Map<String, Value>, answer: impl FnOnce(&Session) -> Answer) -> Answer {
    let path = files::path(&text(arguments, "file")?).map_err(|r| r.message())?;
    let (opened, open_elsewhere) = files::read(&path).map_err(|r| r.message())?;
    let value = answer(&opened.session)?;
    Ok(if open_elsewhere {
        json!({
            "note": "The file is open in the plnwrk app: this is the state last saved there.",
            "result": value,
        })
    } else {
        value
    })
}

/// Runs a command on the named file and saves it.
fn run(arguments: &Map<String, Value>, kept: &mut Kept) -> Answer {
    let command = text(arguments, "command")?;
    let params = object(arguments, "params")?;
    // Refused before the file is even opened: nothing classified may enter it.
    if params::asks_for_classified(&params) {
        return Err(Refusal::ClassifiedCommand.message());
    }
    change(arguments, kept, |session| {
        if command == "task.update" {
            update_task(session, params)
        } else {
            step(session, &command, &params).map(|outcome| vec![outcome])
        }
    })
}

/// Moves a task by whole days (start and deadline, the time of day kept); what waits for it
/// moves along as with any date change.
fn move_task(arguments: &Map<String, Value>, kept: &mut Kept) -> Answer {
    let task = arguments
        .get("task")
        .ok_or("task is missing.")
        .and_then(|v| params::id::<TaskId>("task", v).map_err(|_| "task is not a task id."))?;
    let days = match arguments.get("days") {
        Some(value) => params::integer("days", value, -3650, 3650)
            .map_err(|_| "days must be a whole number from -3650 to 3650.".to_owned())?,
        None => return Err("days is missing.".to_owned()),
    };
    change(arguments, kept, |session| {
        let Some(current) = session.workspace().task(task).cloned() else {
            return Err(format!("There is no task {task}."));
        };
        if current.start.is_none() && current.due.is_none() {
            return Err(format!(
                "{:?} has no dates to move; give it a start or due with task.update.",
                current.title
            ));
        }
        let shift = |date: jiff::civil::Date| {
            date.checked_add(jiff::Span::new().days(days))
                .map_err(|_| "The new date lies beyond the calendar.".to_owned())
        };
        let mut fields = Map::new();
        fields.insert("task".to_owned(), Value::String(task.to_string()));
        if let Some(start) = current.start {
            fields.insert("start".to_owned(), Value::String(shift(start)?.to_string()));
        }
        if let Some(due) = current.due {
            let day = shift(due)?.to_string();
            let due = match current.due_time {
                Some(t) => format!("{day}T{:02}:{:02}", t.hour(), t.minute()),
                None => day,
            };
            fields.insert("due".to_owned(), Value::String(due));
        }
        update_task(session, Value::Object(fields))
    })
}

/// Opens the named file for a change, runs `act` on its session and saves it if anything
/// changed. A refusal leaves the file as it was and keeps what can be undone.
fn change(
    arguments: &Map<String, Value>,
    kept: &mut Kept,
    act: impl FnOnce(&mut Session) -> Result<Vec<Value>, String>,
) -> Answer {
    let path = files::path(&text(arguments, "file")?).map_err(|r| r.message())?;
    let mut file = Changing::open(&path, kept).map_err(|r| r.message())?;
    let upgraded = file.opened.upgraded;
    let logged = file.opened.session.log().len();
    let steps = match act(&mut file.opened.session) {
        Ok(steps) => steps,
        Err(reason) => {
            // Refused as a whole: the session is as it was and keeps its undo steps. A
            // half-done change (in practice impossible: limits are checked first) is dropped
            // unsaved.
            if file.opened.session.log().len() == logged {
                file.keep(kept);
            }
            return Err(reason);
        }
    };
    let changed = steps
        .iter()
        .any(|s| s.get("change").is_some_and(|c| !c.is_null()));
    if changed || upgraded {
        file.save(kept).map_err(|r| {
            format!(
                "The change was NOT saved; the file is unchanged. {}",
                r.message()
            )
        })?;
    } else {
        file.keep(kept);
    }
    Ok(json!({"steps": steps, "saved": changed || upgraded}))
}

/// Runs one command as made by an AI.
fn step(session: &mut Session, command: &str, params: &Value) -> Answer {
    let outcome = session
        .run(command, params, Origin::Assistant)
        .map_err(error)?;
    let mut answer = serde_json::to_value(&outcome).map_err(|e| e.to_string())?;
    if let (Some(seq), Some(fields)) = (outcome.change, answer.as_object_mut()) {
        describe(session, seq, fields);
    }
    Ok(answer)
}

/// Adds to an answer what change `seq` did, so the model can tell the person exactly: every
/// entity it touched (as it is afterwards, or as it was if removed) and, for undo and redo,
/// which step it took back or repeated.
fn describe(session: &Session, seq: u64, answer: &mut Map<String, Value>) {
    let log = session.log();
    let Some(change) = log.iter().rev().find(|c| c.seq == seq) else {
        return;
    };
    let touched: Vec<Value> = change
        .events
        .iter()
        .map(|event| match event {
            Event::Put { before, after } => entity(
                after,
                if before.is_none() {
                    "created"
                } else {
                    "changed"
                },
            ),
            Event::Remove { before } => entity(before, "removed"),
        })
        .collect();
    answer.insert("touched".to_owned(), Value::from(touched));
    let other = |n: u64| {
        log.iter()
            .find(|c| c.seq == n)
            .map(|c| json!({"change": n, "command": c.command}))
    };
    if let Some(back) = change.undoes.and_then(other) {
        answer.insert("takes_back".to_owned(), back);
    }
    if let Some(again) = change.redoes.and_then(other) {
        answer.insert("repeats".to_owned(), again);
    }
}

/// One touched entity, briefly.
fn entity(entity: &Entity, what: &str) -> Value {
    match entity {
        Entity::Task(t) => json!({
            what: "task", "id": t.id, "title": t.title, "start": t.start, "due": t.due,
            "due_time": t.due_time, "status": t.status, "milestone": t.milestone,
        }),
        Entity::Project(p) => json!({what: "project", "id": p.id, "title": p.title}),
        Entity::Milestone(m) => json!({what: "milestone", "id": m.id, "title": m.title}),
        Entity::Dependency(d) => {
            json!({what: "dependency", "id": d.id, "from": d.from, "to": d.to})
        }
        Entity::Person(p) => json!({what: "person", "id": p.id, "name": p.name}),
        Entity::Comment(c) => json!({what: "comment", "id": c.id, "task": c.task}),
    }
}

/// `task.update` with new dates: whatever waits for the task moves along, as in the app.
/// Dates alone are one `task.reschedule` (one step). Together with other fields, the whole
/// update runs first as it was asked (taken as one, or refused as a whole), then the
/// successors move in a second step. Dates given as they are move nothing.
fn update_task(session: &mut Session, params: Value) -> Result<Vec<Value>, String> {
    let plain = |session: &mut Session, params: &Value| {
        step(session, "task.update", params).map(|outcome| vec![outcome])
    };
    let Value::Object(fields) = &params else {
        return plain(session, &params);
    };
    let has_dates = fields.contains_key("start") || fields.contains_key("due");
    let task = fields
        .get("task")
        .and_then(|v| params::id::<TaskId>("task", v).ok());
    let (Some(task), true) = (task, has_dates) else {
        return plain(session, &params);
    };
    let Some(current) = session.workspace().task(task).cloned() else {
        return plain(session, &params);
    };
    // Dates that do not read are left to task.update, which says what is wrong.
    let start = match fields.get("start") {
        None => Ok(current.start),
        Some(Value::Null) => Ok(None),
        Some(value) => params::date("start", value).map(Some),
    };
    let due = match fields.get("due") {
        None => Ok(current.due.map(|day| (day, current.due_time))),
        Some(Value::Null) => Ok(None),
        Some(value) => params::deadline("due", value).map(Some),
    };
    let (Ok(start), Ok(due)) = (start, due) else {
        return plain(session, &params);
    };
    let day = due.map(|(day, _)| day);
    if start == current.start && day == current.due {
        return plain(session, &params).map(|steps| with_along(steps, 0));
    }
    let Some(reschedule) =
        Reschedule::of(session.workspace(), task, start, day, Calendar::standard())
    else {
        return plain(session, &params);
    };
    // Refused before anything runs.
    too_many(&reschedule)?;
    let others = fields
        .keys()
        .any(|k| !["task", "start", "due"].contains(&k.as_str()));
    if !others {
        let time = fields
            .contains_key("due")
            .then(|| due.and_then(|(_, time)| time));
        let moved = step(session, "task.reschedule", &reschedule.params(time))?;
        return Ok(with_along(vec![moved], reschedule.along()));
    }
    let mut steps = plain(session, &params)?;
    // From the state as it is now (another field may matter, e.g. done or milestone).
    let Some(now) = session.workspace().task(task).cloned() else {
        return Ok(with_along(steps, 0));
    };
    let Some(after) = Reschedule::of(
        session.workspace(),
        task,
        now.start,
        now.due,
        Calendar::standard(),
    ) else {
        return Ok(with_along(steps, 0));
    };
    if after.along() > 0 {
        too_many(&after)?;
        steps.push(step(session, "task.reschedule", &after.params(None))?);
    }
    Ok(with_along(steps, after.along()))
}

/// Notes on the last step how many other tasks moved along.
fn with_along(mut steps: Vec<Value>, along: usize) -> Vec<Value> {
    if let Some(last) = steps.last_mut().and_then(Value::as_object_mut) {
        last.insert("moved_along".to_owned(), Value::from(along));
    }
    steps
}

/// Refuses a push larger than one command can take.
fn too_many(reschedule: &Reschedule) -> Result<(), String> {
    if reschedule.moves.len() > MOVES_MAX {
        return Err(format!(
            "Not changed: {} tasks wait for this one and would have to move, more than one step can take ({MOVES_MAX}). Remove dependencies or move them in parts.",
            reschedule.along()
        ));
    }
    Ok(())
}
