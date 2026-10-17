use std::path::{Path, PathBuf};

use plnwrk_core::{Classification, TaskId};
use plnwrk_engine::{Origin, Session, fixture};
use plnwrk_platform::{RandomIds, SystemClock};
use plnwrk_store::{Document, Protection, SaveOptions, SecretString};
use serde_json::{Value, json};

use super::*;

/// A fresh folder for one test.
fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("plnwrk-mcp-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn session() -> Session {
    Document::default().into_session(Box::new(SystemClock), Box::new(RandomIds::default()))
}

fn write(path: &Path, session: &Session, protection: &Protection) {
    let bytes = plnwrk_store::save_with(
        &Document::from_session(session),
        protection,
        SaveOptions {
            work_factor: Some(1),
        },
    )
    .unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// The sample workspace plus two dated tasks, the second waiting for the first.
fn sample(name: &str) -> (PathBuf, TaskId, TaskId) {
    let path = folder(name).join("abteilung.plnwrk");
    let mut s = session();
    let sample = fixture::sample(&mut s).unwrap();
    let mut task = |title: &str, start: &str, due: &str| {
        let outcome = s
            .run(
                "task.create",
                &json!({"project": sample.project.to_string(), "title": title, "start": start, "due": due}),
                Origin::User,
            )
            .unwrap();
        match outcome.created.first() {
            Some(plnwrk_core::EntityKey::Task(id)) => *id,
            _ => panic!("no task"),
        }
    };
    let first = task("Leitstelle anbinden", "2026-11-02", "2026-11-04");
    let second = task("Probebetrieb", "2026-11-05", "2026-11-06");
    s.run(
        "dependency.add",
        &json!({"from": first.to_string(), "to": second.to_string()}),
        Origin::User,
    )
    .unwrap();
    write(&path, &s, &Protection::None);
    (path, first, second)
}

fn open(path: &Path) -> Session {
    let bytes = std::fs::read(path).unwrap();
    plnwrk_store::open(&bytes, None)
        .unwrap()
        .document
        .into_session(Box::new(SystemClock), Box::new(RandomIds::default()))
}

/// Sends one request and returns the answer.
fn ask(server: &mut Server, method: &str, params: Value) -> Value {
    let line = json!({"jsonrpc": "2.0", "id": 7, "method": method, "params": params}).to_string();
    serde_json::from_str(&server.handle(&line).unwrap()).unwrap()
}

/// Calls a tool on a fresh server (a new conversation); returns its text (parsed as JSON if it
/// is) and whether it is an error.
fn tool(name: &str, arguments: Value) -> (Value, bool) {
    tool_on(&mut Server::new(), name, arguments)
}

/// Calls a tool on `server`.
fn tool_on(server: &mut Server, name: &str, arguments: Value) -> (Value, bool) {
    let answer = ask(
        server,
        "tools/call",
        json!({"name": name, "arguments": arguments}),
    );
    let result = &answer["result"];
    let text = result["content"][0]["text"].as_str().unwrap().to_owned();
    let value = serde_json::from_str(&text).unwrap_or(Value::String(text));
    (value, result["isError"].as_bool().unwrap())
}

fn s(path: &Path) -> String {
    path.display().to_string()
}

#[test]
fn initialize_answers_the_version_asked_for_or_the_newest() {
    let mut server = Server::new();
    let answer = ask(
        &mut server,
        "initialize",
        json!({"protocolVersion": "2025-03-26"}),
    );
    assert_eq!(answer["id"], 7);
    assert_eq!(answer["result"]["protocolVersion"], "2025-03-26");
    assert_eq!(answer["result"]["serverInfo"]["name"], "plnwrk");
    assert!(
        answer["result"]["instructions"]
            .as_str()
            .unwrap()
            .contains("VS-NfD")
    );
    let answer = ask(
        &mut server,
        "initialize",
        json!({"protocolVersion": "1999-01-01"}),
    );
    assert_eq!(answer["result"]["protocolVersion"], VERSIONS[0]);
    assert_eq!(ask(&mut server, "ping", json!({}))["result"], json!({}));
}

#[test]
fn notifications_and_responses_get_no_answer() {
    let mut server = Server::new();
    assert_eq!(
        server.handle(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#),
        None
    );
    assert_eq!(
        server.handle(r#"{"jsonrpc":"2.0","id":3,"result":{}}"#),
        None
    );
    assert_eq!(server.handle("   "), None);
}

#[test]
fn broken_messages_get_json_rpc_errors() {
    let mut server = Server::new();
    let code = |server: &mut Server, line: &str| -> i64 {
        let answer: Value = serde_json::from_str(&server.handle(line).unwrap()).unwrap();
        answer["error"]["code"].as_i64().unwrap()
    };
    assert_eq!(code(&mut server, "{not json"), PARSE_ERROR);
    assert_eq!(code(&mut server, "[]"), INVALID_REQUEST);
    assert_eq!(code(&mut server, "42"), INVALID_REQUEST);
    assert_eq!(code(&mut server, r#"{"id":1}"#), INVALID_REQUEST);
    assert_eq!(code(&mut server, r#"{"id":1,"method":5}"#), INVALID_REQUEST);
    assert_eq!(
        code(&mut server, r#"{"id":true,"method":"ping"}"#),
        INVALID_REQUEST
    );
    assert_eq!(
        code(&mut server, r#"{"id":1.5,"method":"ping"}"#),
        INVALID_REQUEST
    );
    assert_eq!(
        code(
            &mut server,
            r#"{"id":123456789012345678901234567890,"method":"ping"}"#
        ),
        INVALID_REQUEST
    );
    assert_eq!(
        code(&mut server, r#"{"id":1,"method":"resources/list"}"#),
        METHOD_NOT_FOUND
    );
    assert_eq!(
        code(&mut server, r#"{"id":1,"method":"ping","params":[1]}"#),
        INVALID_PARAMS
    );
    assert_eq!(
        code(
            &mut server,
            r#"{"id":1,"method":"tools/call","params":{"name":"rm"}}"#
        ),
        INVALID_PARAMS
    );
    assert_eq!(
        code(
            &mut server,
            r#"{"id":1,"method":"tools/call","params":{"name":"overview","arguments":"x"}}"#
        ),
        INVALID_PARAMS
    );
}

#[test]
fn the_tools_are_listed_with_schemas() {
    let answer = ask(&mut Server::new(), "tools/list", json!({}));
    let tools = answer["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(
        names,
        [
            "overview",
            "query",
            "search_commands",
            "run_command",
            "move_task"
        ]
    );
    for tool in tools {
        assert_eq!(tool["inputSchema"]["type"], "object", "{tool}");
    }
    assert!(
        tools[1]["inputSchema"]["properties"]["name"]["enum"]
            .as_array()
            .unwrap()
            .contains(&json!("overdue"))
    );
}

#[test]
fn a_file_is_read_by_its_absolute_path() {
    let (path, _, _) = sample("read");
    let (overview, error) = tool("overview", json!({"file": s(&path)}));
    assert!(!error, "{overview}");
    assert!(
        overview
            .to_string()
            .contains("Digitalfunk Umrüstung Wachen")
    );
    let (tasks, error) = tool("query", json!({"file": s(&path), "name": "overdue"}));
    assert!(!error, "{tasks}");
    // A misspelt argument is named, not ignored.
    let (reason, error) = tool("overview", json!({"file": s(&path), "projekt": "x"}));
    assert!(
        error && reason.as_str().unwrap().contains("Unknown argument"),
        "{reason}"
    );
    // Relative paths, missing files and other files are refused with a reason.
    let (reason, error) = tool("overview", json!({"file": "abteilung.plnwrk"}));
    assert!(error && reason.as_str().unwrap().contains("absolute"));
    let (_, error) = tool(
        "overview",
        json!({"file": s(&path.with_file_name("fehlt.plnwrk"))}),
    );
    assert!(error);
    let other = path.with_file_name("notizen.txt");
    std::fs::write(&other, "geheim: nicht für die KI").unwrap();
    let (reason, error) = tool("overview", json!({"file": s(&other)}));
    assert!(error);
    assert!(!reason.to_string().contains("geheim"), "{reason}");
    assert_eq!(
        std::fs::read(&other).unwrap(),
        b"geheim: nicht f\xc3\xbcr die KI"
    );
}

#[test]
fn a_command_is_saved_as_one_step_by_an_ai() {
    let (path, _, _) = sample("create");
    let project = open(&path).workspace().projects().next().unwrap().id;
    let (answer, error) = tool(
        "run_command",
        json!({"file": s(&path), "command": "task.create",
               "params": {"project": project.to_string(), "title": "Funkprobe"}}),
    );
    assert!(!error, "{answer}");
    let stored = open(&path);
    assert!(stored.workspace().tasks().any(|t| t.title == "Funkprobe"));
    let last = stored.log().last().unwrap();
    assert_eq!(last.origin, Origin::Assistant);
    assert_eq!(last.command, "task.create");
    // A refused command leaves the file as it was.
    let before = std::fs::read(&path).unwrap();
    let (reason, error) = tool(
        "run_command",
        json!({"file": s(&path), "command": "task.create", "params": {"title": "ohne Projekt"}}),
    );
    assert!(error, "{reason}");
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn a_later_deadline_moves_what_waits_and_one_undo_takes_it_back() {
    let (path, first, second) = sample("push");
    let mut server = Server::new();
    let (answer, error) = tool_on(
        &mut server,
        "run_command",
        json!({"file": s(&path), "command": "task.update",
               "params": {"task": first.to_string(), "due": "2026-11-06T12:00", "title": "Leitstelle anbinden (verlängert)"}}),
    );
    assert!(!error, "{answer}");
    // Dates together with the title: first the update as asked, then the successor.
    assert_eq!(answer["steps"][1]["moved_along"], 1);
    let stored = open(&path);
    let dates = |s: &Session, id: TaskId| {
        let t = s.workspace().task(id).unwrap();
        (
            t.start.unwrap().to_string(),
            t.due.unwrap().to_string(),
            t.due_time,
        )
    };
    assert_eq!(
        dates(&stored, first),
        (
            "2026-11-02".into(),
            "2026-11-06".into(),
            "12:00".parse().ok()
        )
    );
    assert_eq!(
        dates(&stored, second).0,
        "2026-11-07",
        "the day after, a Saturday: every day counts"
    );
    assert_eq!(
        stored.workspace().task(first).unwrap().title,
        "Leitstelle anbinden (verlängert)"
    );
    // The dates are one step: undo the title, then undo the move of both.
    for _ in 0..2 {
        let (answer, error) = tool_on(
            &mut server,
            "run_command",
            json!({"file": s(&path), "command": "undo"}),
        );
        assert!(!error, "{answer}");
    }
    let stored = open(&path);
    assert_eq!(dates(&stored, first).1, "2026-11-04");
    assert_eq!(dates(&stored, second).0, "2026-11-05");
    // A refused command in between does not lose what can be undone.
    let (_, error) = tool_on(
        &mut server,
        "run_command",
        json!({"file": s(&path), "command": "task.update", "params": {"task": "nope"}}),
    );
    assert!(error);
    let (answer, error) = tool_on(
        &mut server,
        "run_command",
        json!({"file": s(&path), "command": "redo"}),
    );
    assert!(!error, "{answer}");
    let (_, error) = tool_on(
        &mut server,
        "run_command",
        json!({"file": s(&path), "command": "undo"}),
    );
    assert!(!error);
    // Nothing left that was done here: the steps before (made by a person) stay.
    let (answer, error) = tool_on(
        &mut server,
        "run_command",
        json!({"file": s(&path), "command": "undo"}),
    );
    assert!(error, "{answer}");
    assert!(open(&path).workspace().task(second).is_some());
}

#[test]
fn new_dates_alone_are_one_step() {
    let (path, first, second) = sample("dates-only");
    let mut server = Server::new();
    let call = |server: &mut Server, command: &str, params: Value| {
        tool_on(
            server,
            "run_command",
            json!({"file": s(&path), "command": command, "params": params}),
        )
    };
    let (answer, error) = call(
        &mut server,
        "task.update",
        json!({"task": first.to_string(), "start": "2026-11-03", "due": "2026-11-05"}),
    );
    assert!(!error, "{answer}");
    assert_eq!(answer["steps"].as_array().unwrap().len(), 1);
    assert_eq!(answer["steps"][0]["moved_along"], 1);
    assert_eq!(
        open(&path)
            .workspace()
            .task(second)
            .unwrap()
            .start
            .unwrap()
            .to_string(),
        "2026-11-06"
    );
    // A refused change with dates and a wrong field: nothing taken, undo still there.
    let before = std::fs::read(&path).unwrap();
    let (_, error) = call(
        &mut server,
        "task.update",
        json!({"task": first.to_string(), "due": "2026-11-20", "status": "bogus"}),
    );
    assert!(error);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let (answer, error) = call(&mut server, "undo", json!({}));
    assert!(!error, "{answer}");
    assert_eq!(
        open(&path)
            .workspace()
            .task(second)
            .unwrap()
            .start
            .unwrap()
            .to_string(),
        "2026-11-05"
    );
}

#[test]
fn write_protected_files_and_odd_paths_are_left_alone() {
    let (path, first, _) = sample("protected");
    let dir = path.parent().unwrap().to_path_buf();
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&path, permissions.clone()).unwrap();
    let before = std::fs::read(&path).unwrap();
    let (reason, error) = tool(
        "run_command",
        json!({"file": s(&path), "command": "task.update",
               "params": {"task": first.to_string(), "title": "trotzdem"}}),
    );
    assert!(
        error && reason.as_str().unwrap().contains("write-protected"),
        "{reason}"
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    #[expect(
        clippy::permissions_set_readonly_false,
        reason = "the test made it read-only"
    )]
    permissions.set_readonly(false);
    std::fs::set_permissions(&path, permissions.clone()).unwrap();
    // Not writable for this user, though others may (group): refused too.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o464)).unwrap();
        let (reason, error) = tool(
            "run_command",
            json!({"file": s(&path), "command": "task.update",
                   "params": {"task": first.to_string(), "title": "trotzdem"}}),
        );
        assert!(
            error && reason.as_str().unwrap().contains("write-protected"),
            "{reason}"
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
        std::fs::set_permissions(&path, permissions).unwrap();
    }
    // Neither reading nor a refused change leaves anything next to what is not a file.
    let lock_files = || {
        std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().ends_with(".lock"))
            .count()
    };
    for target in [dir.join("fehlt.plnwrk"), dir.join("unterordner")] {
        let _ = std::fs::create_dir_all(dir.join("unterordner"));
        let (_, error) = tool("overview", json!({"file": s(&target)}));
        assert!(error);
        let (_, error) = tool(
            "run_command",
            json!({"file": s(&target), "command": "undo"}),
        );
        assert!(error);
    }
    // Refused changes of files that are not ours to change: no lock file either.
    let other = dir.join("notizen.txt");
    std::fs::write(&other, "kein plnwrk").unwrap();
    let mut enc = session();
    fixture::sample(&mut enc).unwrap();
    let encrypted = dir.join("verschluesselt.plnwrk");
    write(
        &encrypted,
        &enc,
        &Protection::Passphrase(SecretString::from("Lagezentrum-2026".to_owned())),
    );
    for target in [&other, &encrypted] {
        let (_, error) = tool("run_command", json!({"file": s(target), "command": "undo"}));
        assert!(error);
    }
    let (_, error) = tool("overview", json!({"file": s(&path)}));
    assert!(!error);
    assert_eq!(lock_files(), 0);
    let (reason, error) = tool("overview", json!({"file": "/dev/null"}));
    assert!(error, "{reason}");
}

#[test]
fn dates_given_as_they_are_move_nothing() {
    let (path, first, second) = sample("same-dates");
    // The second already starts too early (as can happen with old data).
    let mut s_ = open(&path);
    s_.run(
        "task.update",
        &json!({"task": second.to_string(), "start": "2026-11-03"}),
        Origin::User,
    )
    .unwrap();
    write(&path, &s_, &Protection::None);
    for params in [
        json!({"task": first.to_string(), "title": "neu", "due": "2026-11-04"}),
        json!({"task": first.to_string(), "start": "2026-11-02", "due": "2026-11-04"}),
    ] {
        let (answer, error) = tool(
            "run_command",
            json!({"file": s(&path), "command": "task.update", "params": params}),
        );
        assert!(!error, "{answer}");
        assert_eq!(
            answer["steps"].as_array().unwrap().last().unwrap()["moved_along"],
            0
        );
        assert_eq!(
            open(&path)
                .workspace()
                .task(second)
                .unwrap()
                .start
                .unwrap()
                .to_string(),
            "2026-11-03"
        );
    }
}

#[test]
fn a_push_too_big_is_refused_before_anything_runs() {
    let path = folder("too-big").join("gross.plnwrk");
    let mut s_ = session();
    let sample = fixture::sample(&mut s_).unwrap();
    let project = sample.project.to_string();
    let mut create = |title: String| {
        match s_
            .run(
                "task.create",
                &json!({"project": project, "title": title, "start": "2026-11-05", "due": "2026-11-05"}),
                Origin::User,
            )
            .unwrap()
            .created
            .first()
        {
            Some(plnwrk_core::EntityKey::Task(id)) => *id,
            _ => panic!("no task"),
        }
    };
    let first = create("Anfang".to_owned());
    let after: Vec<TaskId> = (0..=plnwrk_engine::params::MOVES_MAX)
        .map(|n| create(format!("Folge {n}")))
        .collect();
    for id in &after {
        s_.run(
            "dependency.add",
            &json!({"from": first.to_string(), "to": id.to_string()}),
            Origin::User,
        )
        .unwrap();
    }
    write(&path, &s_, &Protection::None);
    let mut server = Server::new();
    let call = |server: &mut Server, command: &str, params: Value| {
        tool_on(
            server,
            "run_command",
            json!({"file": s(&path), "command": command, "params": params}),
        )
    };
    let (_, error) = call(
        &mut server,
        "task.update",
        json!({"task": first.to_string(), "title": "Anfang (KI)"}),
    );
    assert!(!error);
    let before = std::fs::read(&path).unwrap();
    let (reason, error) = call(
        &mut server,
        "task.update",
        json!({"task": first.to_string(), "due": "2026-11-20", "title": "zu viel"}),
    );
    assert!(
        error && reason.as_str().unwrap().starts_with("Not changed"),
        "{reason}"
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    // Nothing ran, so the step before can still be taken back.
    let (answer, error) = call(&mut server, "undo", json!({}));
    assert!(!error, "{answer}");
    assert_eq!(open(&path).workspace().task(first).unwrap().title, "Anfang");
}

#[test]
fn answers_say_what_changed_and_what_undo_took_back() {
    let (path, _, _) = sample("answers");
    let project = open(&path).workspace().projects().next().unwrap().id;
    let mut server = Server::new();
    let (answer, error) = tool_on(
        &mut server,
        "run_command",
        json!({"file": s(&path), "command": "task.create",
               "params": {"project": project.to_string(), "title": "Funkprobe Wache 12", "due": "2026-11-20"}}),
    );
    assert!(!error, "{answer}");
    let touched = &answer["steps"][0]["touched"][0];
    assert_eq!(touched["created"], "task");
    assert_eq!(touched["title"], "Funkprobe Wache 12");
    assert_eq!(touched["due"], "2026-11-20");
    let (answer, error) = tool_on(
        &mut server,
        "run_command",
        json!({"file": s(&path), "command": "undo"}),
    );
    assert!(!error, "{answer}");
    assert_eq!(answer["steps"][0]["takes_back"]["command"], "task.create");
    assert_eq!(answer["steps"][0]["touched"][0]["removed"], "task");
    assert_eq!(
        answer["steps"][0]["touched"][0]["title"],
        "Funkprobe Wache 12"
    );
    let (answer, _) = tool_on(
        &mut server,
        "run_command",
        json!({"file": s(&path), "command": "redo"}),
    );
    assert_eq!(answer["steps"][0]["repeats"]["command"], "task.create");
}

#[test]
fn a_task_moves_by_days_and_takes_what_waits_along() {
    let (path, first, second) = sample("move");
    // A deadline with a time keeps it.
    let mut s_ = open(&path);
    s_.run(
        "task.update",
        &json!({"task": first.to_string(), "due": "2026-11-04T14:00"}),
        Origin::User,
    )
    .unwrap();
    write(&path, &s_, &Protection::None);
    let mut server = Server::new();
    let (answer, error) = tool_on(
        &mut server,
        "move_task",
        json!({"file": s(&path), "task": first.to_string(), "days": 7}),
    );
    assert!(!error, "{answer}");
    assert_eq!(answer["steps"].as_array().unwrap().len(), 1, "one step");
    assert_eq!(answer["steps"][0]["moved_along"], 1);
    let stored = open(&path);
    let t = stored.workspace().task(first).unwrap();
    assert_eq!(
        t.start.unwrap().to_string(),
        "2026-11-09",
        "a week is seven days"
    );
    assert_eq!(t.due.unwrap().to_string(), "2026-11-11");
    assert_eq!(t.due_time, "14:00".parse().ok());
    assert_eq!(
        stored
            .workspace()
            .task(second)
            .unwrap()
            .start
            .unwrap()
            .to_string(),
        "2026-11-12"
    );
    // Back by a week: the successor stays (nothing is pulled earlier).
    let (_, error) = tool_on(
        &mut server,
        "move_task",
        json!({"file": s(&path), "task": first.to_string(), "days": -7}),
    );
    assert!(!error);
    let stored = open(&path);
    assert_eq!(
        stored
            .workspace()
            .task(first)
            .unwrap()
            .start
            .unwrap()
            .to_string(),
        "2026-11-02"
    );
    assert_eq!(
        stored
            .workspace()
            .task(second)
            .unwrap()
            .start
            .unwrap()
            .to_string(),
        "2026-11-12"
    );
    // A task without dates, a bad number: said plainly, nothing changed.
    let undated = open(&path)
        .workspace()
        .tasks()
        .find(|t| t.start.is_none() && t.due.is_none())
        .unwrap()
        .id;
    let before = std::fs::read(&path).unwrap();
    let (reason, error) = tool_on(
        &mut server,
        "move_task",
        json!({"file": s(&path), "task": undated.to_string(), "days": 7}),
    );
    assert!(
        error && reason.as_str().unwrap().contains("no dates"),
        "{reason}"
    );
    let (reason, error) = tool_on(
        &mut server,
        "move_task",
        json!({"file": s(&path), "task": first.to_string(), "days": 99999}),
    );
    assert!(
        error && reason.as_str().unwrap().contains("days"),
        "{reason}"
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn undo_does_not_reach_over_a_change_made_elsewhere() {
    let (path, first, _) = sample("elsewhere");
    let mut server = Server::new();
    let (_, error) = tool_on(
        &mut server,
        "run_command",
        json!({"file": s(&path), "command": "task.update",
               "params": {"task": first.to_string(), "title": "durch KI"}}),
    );
    assert!(!error);
    // Meanwhile the app saves a change of its own.
    let mut app = open(&path);
    app.run(
        "task.update",
        &json!({"task": first.to_string(), "title": "durch Tobi"}),
        Origin::User,
    )
    .unwrap();
    write(&path, &app, &Protection::None);
    let (answer, error) = tool_on(
        &mut server,
        "run_command",
        json!({"file": s(&path), "command": "undo"}),
    );
    assert!(error, "{answer}");
    assert_eq!(
        open(&path).workspace().task(first).unwrap().title,
        "durch Tobi"
    );
}

#[test]
fn nothing_classified_comes_in_or_goes_out() {
    let (path, _, _) = sample("classified");
    let before = std::fs::read(&path).unwrap();
    let project = open(&path).workspace().projects().next().unwrap().id;
    let (reason, error) = tool(
        "run_command",
        json!({"file": s(&path), "command": "project.update",
               "params": {"project": project.to_string(), "classification": "vs_nfd"}}),
    );
    assert!(error);
    assert!(reason.as_str().unwrap().contains("VS-NfD"));
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "not even opened for it"
    );
    // Refused before the file is even opened: also while the app has it open.
    let held = plnwrk_store::file::lock(&plnwrk_store::file::resolve(&path).unwrap()).unwrap();
    let (reason, _) = tool(
        "run_command",
        json!({"file": s(&path), "command": "project.update",
               "params": {"project": project.to_string(), "classification": "vs_nfd"}}),
    );
    assert!(reason.as_str().unwrap().contains("VS-NfD"), "{reason}");
    drop(held);

    // A classified file: refused by its head, before any passphrase.
    let mut s_ = session();
    let sample = fixture::sample(&mut s_).unwrap();
    s_.run(
        "project.update",
        &json!({"project": sample.project.to_string(), "classification": "vs_nfd"}),
        Origin::User,
    )
    .unwrap();
    let secret = path.with_file_name("geheim.plnwrk");
    write(
        &secret,
        &s_,
        &Protection::Passphrase(SecretString::from("Lagezentrum-2026".to_owned())),
    );
    let (reason, error) = tool("overview", json!({"file": s(&secret)}));
    assert!(error);
    assert!(reason.as_str().unwrap().contains("VS-NfD"), "{reason}");
    assert!(!reason.to_string().contains("Digitalfunk"));
    let (reason, _) = tool(
        "run_command",
        json!({"file": s(&secret), "command": "undo"}),
    );
    assert!(reason.as_str().unwrap().contains("VS-NfD"), "{reason}");
}

#[test]
fn a_session_that_became_classified_is_never_saved() {
    let (path, _, _) = sample("last-line");
    let before = std::fs::read(&path).unwrap();
    let mut kept = files::Kept::default();
    let mut file = files::Changing::open(&path, &mut kept).unwrap();
    let project = file
        .opened
        .session
        .workspace()
        .projects()
        .next()
        .unwrap()
        .id;
    file.opened
        .session
        .run(
            "project.update",
            &json!({"project": project.to_string(), "classification": "vs_nfd"}),
            Origin::Assistant,
        )
        .unwrap();
    assert!(matches!(
        file.save(&mut kept),
        Err(files::Refusal::Classified)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn encrypted_files_are_not_opened() {
    let path = folder("encrypted").join("verschluesselt.plnwrk");
    let mut s_ = session();
    fixture::sample(&mut s_).unwrap();
    write(
        &path,
        &s_,
        &Protection::Passphrase(SecretString::from("Lagezentrum-2026".to_owned())),
    );
    let (reason, error) = tool("overview", json!({"file": s(&path)}));
    assert!(error);
    assert!(reason.as_str().unwrap().contains("encrypted"), "{reason}");
    assert_ne!(
        plnwrk_store::peek(&std::fs::read(&path).unwrap())
            .unwrap()
            .classification,
        Classification::VsNfd
    );
}

#[test]
fn a_file_open_in_the_app_is_read_but_not_changed() {
    let (path, first, _) = sample("in-use");
    let before = std::fs::read(&path).unwrap();
    let held = plnwrk_store::file::lock(&plnwrk_store::file::resolve(&path).unwrap()).unwrap();
    let (reason, error) = tool(
        "run_command",
        json!({"file": s(&path), "command": "task.update",
               "params": {"task": first.to_string(), "title": "parallel"}}),
    );
    assert!(error);
    assert!(
        reason.as_str().unwrap().contains("open in the plnwrk app"),
        "{reason}"
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let (answer, error) = tool("overview", json!({"file": s(&path)}));
    assert!(!error);
    assert!(answer["note"].as_str().unwrap().contains("last saved"));
    drop(held);
    let (answer, _) = tool("overview", json!({"file": s(&path)}));
    assert!(answer.get("note").is_none());
}

#[test]
fn junk_arguments_never_break_the_server() {
    let (path, first, _) = sample("junk");
    let file = s(&path);
    let junk = [
        json!(null),
        json!(true),
        json!(-1),
        json!(1e308),
        json!("x"),
        json!([]),
        json!({}),
        json!({"task": "nope"}),
        json!({"classification": {"classification": "vs_nfd"}}),
        json!("\u{0}"),
        json!("../../etc/passwd"),
        json!("~/"),
        json!("/"),
    ];
    let before = std::fs::read(&path).unwrap();
    for tool_name in [
        "overview",
        "query",
        "search_commands",
        "run_command",
        "move_task",
        "nope",
    ] {
        for a in &junk {
            for b in &junk {
                let mut calls = vec![
                    json!({"file": a}),
                    json!({"file": file, "name": a, "params": b}),
                    json!({"file": file, "command": a, "params": b}),
                    json!({"words": a}),
                    json!({"file": file, "task": a, "days": b}),
                    json!({"file": file, "extra": a}),
                ];
                // A whole number of days is a real move, not junk.
                if !a.is_i64() {
                    calls.push(json!({"file": file, "task": first.to_string(), "days": a}));
                }
                // null would clear the dates: a real change, not junk.
                if !a.is_null() && !b.is_null() {
                    calls.push(json!({"file": file, "command": "task.update",
                                      "params": {"task": first.to_string(), "due": a, "start": b}}));
                }
                for arguments in calls {
                    let line = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
                                      "params": {"name": tool_name, "arguments": arguments}})
                    .to_string();
                    let answer: Value =
                        serde_json::from_str(&Server::new().handle(&line).unwrap()).unwrap();
                    assert!(answer.get("result").is_some() || answer.get("error").is_some());
                }
            }
        }
    }
    // Only well-formed date changes could have changed it; none of the junk did.
    let stored = open(&path);
    let original = plnwrk_store::open(&before, None).unwrap().document;
    assert_eq!(stored.workspace(), &original.workspace);
}

#[test]
fn serve_answers_line_by_line_and_skips_overlong_messages() {
    let mut input = Vec::new();
    input.extend_from_slice(br#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#);
    input.push(b'\n');
    input.extend_from_slice(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
    input.push(b'\n');
    input.extend(std::iter::repeat_n(b'x', MAX_LINE + 10));
    input.push(b'\n');
    input.extend_from_slice(&[0xff, 0xfe, b'\n']);
    input.extend_from_slice(br#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#);
    let mut output = Vec::new();
    serve(std::io::Cursor::new(input), &mut output).unwrap();
    let lines: Vec<Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 4, "{lines:?}");
    assert_eq!(lines[0]["id"], 1);
    assert_eq!(lines[1]["error"]["code"], INVALID_REQUEST);
    assert_eq!(lines[2]["error"]["code"], PARSE_ERROR);
    assert_eq!(lines[3]["id"], 2);
}

#[test]
fn search_commands_says_where_a_command_is_in_the_app() {
    let (found, error) = tool("search_commands", json!({"words": "task.delete"}));
    assert!(!error, "{found}");
    let delete = found
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "task.delete")
        .unwrap();
    assert_eq!(delete["in_the_app"]["de"], "Aufgabe › Ändern");
    assert_eq!(delete["in_the_app"]["en"], "Task › Change");
    // Without a button in the ribbon, no place.
    let (found, _) = tool("search_commands", json!({"words": "comment.add"}));
    let comment = found
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "comment.add")
        .unwrap();
    assert!(comment.get("in_the_app").is_none());
}
