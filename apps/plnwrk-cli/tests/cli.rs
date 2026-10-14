//! The command line as a user or agent sees it.

#[cfg(test)]
mod cli {
    use std::process::Command;

    use serde_json::Value;

    fn run(args: &[&str]) -> (bool, Value) {
        let output = Command::new(env!("CARGO_BIN_EXE_plnwrk-cli"))
            .args(args)
            .output()
            .unwrap();
        let value = serde_json::from_slice(&output.stdout).unwrap_or(Value::Null);
        (output.status.success(), value)
    }

    #[test]
    fn lists_all_commands_with_schemas() {
        let (ok, value) = run(&["commands"]);
        assert!(ok);
        let commands = value.as_array().unwrap();
        assert!(commands.len() >= 20);
        assert!(
            commands
                .iter()
                .all(|c| c["input_schema"]["type"] == "object")
        );
    }

    #[test]
    fn searches_commands_and_lists_queries() {
        // Milestones are tasks now: the search finds task commands, never retired ones.
        let (_, value) = run(&["commands", "--search", "milestone"]);
        let ids: Vec<&str> = value
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["id"].as_str().unwrap())
            .collect();
        assert!(ids.contains(&"task.create"), "{ids:?}");
        assert!(
            ids.iter().all(|id| !id.starts_with("milestone.")),
            "{ids:?}"
        );
        let (ok, value) = run(&["queries"]);
        assert!(ok);
        assert!(
            value
                .as_array()
                .unwrap()
                .iter()
                .any(|q| q["name"] == "board")
        );
    }

    #[test]
    fn demo_shows_a_board_with_five_columns() {
        let (ok, value) = run(&["demo"]);
        assert!(ok);
        assert_eq!(value["board"]["columns"].as_array().map(Vec::len), Some(5));
        assert_eq!(value["case"]["project"]["reference"], "23/1234");
        assert!(value["overview"].as_array().is_some_and(|o| o.len() == 2));
    }

    #[test]
    fn version_and_unknown_subcommands() {
        let output = Command::new(env!("CARGO_BIN_EXE_plnwrk-cli"))
            .arg("--version")
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).starts_with("plnwrk-cli "));
        let (ok, _) = run(&["fly"]);
        assert!(!ok);
    }

    #[test]
    fn the_mcp_server_talks_json_rpc_over_stdio() {
        use std::io::Write as _;
        use std::process::Stdio;

        let dir = std::env::temp_dir().join(format!("plnwrk-cli-mcp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("abteilung.plnwrk");
        let file = file.to_str().unwrap();
        assert!(run(&["new", file, "--sample"]).0);

        let mut child = Command::new(env!("CARGO_BIN_EXE_plnwrk-cli"))
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let messages = [
            serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": {"protocolVersion": "2025-06-18", "capabilities": {},
                           "clientInfo": {"name": "test", "version": "1"}}}),
            serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
            serde_json::json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
                "params": {"name": "overview", "arguments": {"file": file}}}),
        ];
        {
            let mut stdin = child.stdin.take().unwrap();
            for message in &messages {
                writeln!(stdin, "{message}").unwrap();
            }
        }
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        let answers: Vec<Value> = String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        let ids: Vec<i64> = answers.iter().map(|a| a["id"].as_i64().unwrap()).collect();
        assert_eq!(
            ids,
            [1, 2, 3],
            "one answer per request, none for the notification"
        );
        assert_eq!(answers[0]["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(answers[2]["result"]["isError"], false);
        assert!(
            answers[2]["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Digitalfunk")
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
