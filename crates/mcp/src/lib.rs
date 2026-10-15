//! Model Context Protocol server of plnwrk (layer 3, native only). See `docs/PLAN.md` §3.7 and
//! `docs/plans/2026-10-14-phase-6a.md`.
//!
//! MCP over stdio is JSON-RPC 2.0, one message per line. The protocol part is a plain function
//! from a line to an answer ([`Server::handle`]), so it is tested without a process; [`serve`]
//! connects it to stdin and stdout. Deliberately without the `rmcp` crate: the few methods
//! needed (initialize, ping, tools/list, tools/call) do not justify an async runtime.

mod files;
mod tools;

use std::io::{self, BufRead, Read, Write};

use serde_json::{Map, Value, json};

/// Protocol versions understood, newest first.
const VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

/// Longest message read (4 MiB; a parsed message takes many times that in memory); anything
/// longer is answered with an error and skipped.
pub const MAX_LINE: usize = 4 * 1024 * 1024;

/// What the model is told when it connects.
const INSTRUCTIONS: &str = "plnwrk manages projects and cases (Vorgänge) with tasks, deadlines, \
milestones and dependencies, each workspace in one .plnwrk file. Every tool names the file by \
its absolute path. Start with overview to get project ids, then query (e.g. tasks, overdue, \
project, task, history) for more. To change something, find the command with search_commands \
and call run_command; each change is saved at once, is an undo step and is marked as made by an \
AI. Dates are YYYY-MM-DD, deadlines optionally YYYY-MM-DDTHH:MM. Changing a task's start or due \
moves the tasks that wait for it later as needed (every day counts); together with other \
fields that is two undo steps. \"undo\" takes back your own last step, as long as the file was \
not changed elsewhere since (never a change made in the app). Files \
classified VS-NfD and encrypted files are never opened.";

/// JSON-RPC error codes.
const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

/// The MCP server: answers one message at a time.
#[derive(Default)]
pub struct Server {
    /// Sessions of the files changed here, so undo reaches over calls.
    kept: files::Kept,
}

impl Server {
    pub fn new() -> Self {
        Self::default()
    }

    /// Answers one message (one line of JSON). Notifications and responses get no answer.
    pub fn handle(&mut self, line: &str) -> Option<String> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }
        let answer = match serde_json::from_str::<Value>(line) {
            Err(e) => Some(failure(
                &Value::Null,
                PARSE_ERROR,
                &format!("not JSON: {e}"),
            )),
            Ok(Value::Array(_)) => Some(failure(
                &Value::Null,
                INVALID_REQUEST,
                "batches are not supported",
            )),
            Ok(Value::Object(message)) => self.message(&message),
            Ok(_) => Some(failure(
                &Value::Null,
                INVALID_REQUEST,
                "a message is a JSON object",
            )),
        };
        answer.map(|v| v.to_string())
    }

    /// One JSON-RPC message.
    fn message(&mut self, message: &Map<String, Value>) -> Option<Value> {
        let id = message.get("id");
        let Some(method) = message.get("method") else {
            // A response to something the server never asked: nothing to say.
            return match id {
                Some(id) if !message.contains_key("result") && !message.contains_key("error") => {
                    Some(failure(id, INVALID_REQUEST, "a request needs a method"))
                }
                _ => None,
            };
        };
        let Some(method) = method.as_str() else {
            return id.map(|id| failure(id, INVALID_REQUEST, "method must be a string"));
        };
        // A notification (no id): nothing to answer, whatever it is.
        let id = id?;
        // Whole numbers only: a float would not come back as the same id.
        if !(id.is_string() || id.is_i64() || id.is_u64()) {
            return Some(failure(
                &Value::Null,
                INVALID_REQUEST,
                "id must be a string or a whole number",
            ));
        }
        let params = match message.get("params") {
            None | Some(Value::Null) => Map::new(),
            Some(Value::Object(params)) => params.clone(),
            Some(_) => return Some(failure(id, INVALID_PARAMS, "params must be an object")),
        };
        Some(match method {
            "initialize" => success(id, initialize(&params)),
            "ping" => success(id, json!({})),
            "tools/list" => success(id, json!({"tools": tools::list()})),
            "tools/call" => match call(&params, &mut self.kept) {
                Ok(result) => success(id, result),
                Err(reason) => failure(id, INVALID_PARAMS, &reason),
            },
            _ => failure(id, METHOD_NOT_FOUND, &format!("unknown method {method:?}")),
        })
    }
}

/// The answer to `initialize`: the client's protocol version if known, else the newest.
fn initialize(params: &Map<String, Value>) -> Value {
    let asked = params.get("protocolVersion").and_then(Value::as_str);
    let version = asked
        .filter(|v| VERSIONS.contains(v))
        .unwrap_or(VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": {"tools": {"listChanged": false}},
        "serverInfo": {"name": "plnwrk", "version": env!("CARGO_PKG_VERSION")},
        "instructions": INSTRUCTIONS,
    })
}

/// `tools/call`: an unknown tool or arguments of the wrong shape are protocol errors; what the
/// tool itself refuses is a result marked as an error, so the model can read why.
fn call(params: &Map<String, Value>, kept: &mut files::Kept) -> Result<Value, String> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or("tools/call needs the name of a tool")?;
    let arguments = match params.get("arguments") {
        None | Some(Value::Null) => Map::new(),
        Some(Value::Object(arguments)) => arguments.clone(),
        Some(_) => return Err("arguments must be an object".to_owned()),
    };
    if !["overview", "query", "search_commands", "run_command"].contains(&name) {
        return Err(format!("unknown tool {name:?}"));
    }
    let (text, is_error) = match tools::call(name, &arguments, kept) {
        Ok(value) => (
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string()),
            false,
        ),
        Err(reason) => (reason, true),
    };
    Ok(json!({
        "content": [{"type": "text", "text": text}],
        "isError": is_error,
    }))
}

fn success(id: &Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn failure(id: &Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

/// Serves MCP on `input` and `output` until the input ends. Messages longer than
/// [`MAX_LINE`] are skipped with an error.
pub fn serve(mut input: impl BufRead, mut output: impl Write) -> io::Result<()> {
    let mut server = Server::new();
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        let limit = u64::try_from(MAX_LINE)
            .unwrap_or(u64::MAX)
            .saturating_add(1);
        let read = Read::take(&mut input, limit).read_until(b'\n', &mut buffer)?;
        if read == 0 {
            return Ok(());
        }
        let answer = if buffer.len() > MAX_LINE {
            // Skip the rest of the message.
            let mut rest = Vec::new();
            while buffer.last() != Some(&b'\n') {
                rest.clear();
                let more = Read::take(&mut input, 64 * 1024).read_until(b'\n', &mut rest)?;
                if more == 0 || rest.last() == Some(&b'\n') {
                    break;
                }
            }
            Some(failure(&Value::Null, INVALID_REQUEST, "the message is too long").to_string())
        } else {
            match std::str::from_utf8(&buffer) {
                Ok(line) => server.handle(line),
                Err(_) => {
                    Some(failure(&Value::Null, PARSE_ERROR, "the message is not UTF-8").to_string())
                }
            }
        };
        if let Some(answer) = answer {
            writeln!(output, "{answer}")?;
            output.flush()?;
        }
    }
}

#[cfg(test)]
mod tests;
