//! The tools the MCP server offers: read a workspace (overview, any query), find commands and
//! run one. Every tool names its file; every change is saved at once, is one undo step (two,
//! when dates and other fields of a task change together) and is marked as made by an AI.

use plnwrk_core::TaskId;
use plnwrk_engine::params::{self, MOVES_MAX};
use plnwrk_engine::query::query_registry;
use plnwrk_engine::{CommandSpec, Origin, Session, registry, search};
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
            "description": "Runs one command (see search_commands) and saves the file. Each command is one undo step, marked in the history as made by an AI; \"undo\" and \"redo\" reach the steps made in this conversation, as long as the file was not changed elsewhere in between (never a person's change). Changing start or due of a task with task.update moves every task that waits for it as far later as needed (Hamburg working days), in the same step. Refused for files open in the plnwrk app, encrypted files and anything classified VS-NfD.",
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
        return Err(Refusal::Classified.message());
    }
    let path = files::path(&text(arguments, "file")?).map_err(|r| r.message())?;
    let mut file = Changing::open(&path, kept).map_err(|r| r.message())?;
    let upgraded = file.opened.upgraded;
    let session = &mut file.opened.session;
    let logged = session.log().len();
    let result = if command == "task.update" {
        update_task(session, params)
    } else {
        step(session, &command, &params).map(|outcome| vec![outcome])
    };
    let steps = match result {
        Ok(steps) => steps,
        Err(reason) => {
            // Refused as a whole: the session is as it was and keeps its undo steps. A
            // half-done change (dates taken, other fields refused) is dropped unsaved.
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
    serde_json::to_value(outcome).map_err(|e| e.to_string())
}

/// `task.update`: new dates go through `task.reschedule`, so that whatever waits for the task
/// moves along (one step); the other fields follow in a second step.
fn update_task(session: &mut Session, params: Value) -> Result<Vec<Value>, String> {
    let Value::Object(mut fields) = params else {
        return Ok(vec![step(session, "task.update", &params)?]);
    };
    let has_dates = fields.contains_key("start") || fields.contains_key("due");
    let task = fields
        .get("task")
        .and_then(|v| params::id::<TaskId>("task", v).ok());
    let (Some(task), true) = (task, has_dates) else {
        return Ok(vec![step(session, "task.update", &Value::Object(fields))?]);
    };
    let Some(current) = session.workspace().task(task).cloned() else {
        return Ok(vec![step(session, "task.update", &Value::Object(fields))?]);
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
        return Ok(vec![step(session, "task.update", &Value::Object(fields))?]);
    };
    let Some(reschedule) = Reschedule::of(
        session.workspace(),
        task,
        start,
        due.map(|(day, _)| day),
        Calendar::hamburg(),
    ) else {
        return Ok(vec![step(session, "task.update", &Value::Object(fields))?]);
    };
    if reschedule.moves.len() > MOVES_MAX {
        return Err(format!(
            "Not changed: {} tasks wait for this one and would have to move, more than one step can take ({MOVES_MAX}). Remove dependencies or move them in parts.",
            reschedule.along()
        ));
    }
    let time = fields
        .contains_key("due")
        .then(|| due.and_then(|(_, time)| time));
    let mut steps = vec![step(session, "task.reschedule", &reschedule.params(time))?];
    fields.remove("start");
    fields.remove("due");
    if fields.len() > 1 {
        steps.push(step(session, "task.update", &Value::Object(fields))?);
    }
    if let Some(first) = steps.first_mut().and_then(Value::as_object_mut) {
        first.insert("moved_along".to_owned(), Value::from(reschedule.along()));
    }
    Ok(steps)
}
