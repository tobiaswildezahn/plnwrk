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
        assert!(commands.len() >= 23);
        assert!(
            commands
                .iter()
                .all(|c| c["input_schema"]["type"] == "object")
        );
    }

    #[test]
    fn searches_commands_and_lists_queries() {
        let (_, value) = run(&["commands", "--search", "milestone"]);
        assert!(value.as_array().is_some_and(|found| found.len() >= 4));
        assert!(value.as_array().unwrap().iter().all(|c| {
            c["id"].as_str().unwrap().contains("milestone")
                || c["summary"]
                    .as_str()
                    .unwrap()
                    .to_lowercase()
                    .contains("milestone")
        }));
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
}
