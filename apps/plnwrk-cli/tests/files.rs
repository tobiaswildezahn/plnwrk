//! The command line on real files: create, change, query, protect, verify, export.

#[cfg(test)]
mod files {
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output, Stdio};

    use serde_json::{Value, json};

    struct Dir(PathBuf);

    impl Dir {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("plnwrk-cli-{}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn file(&self) -> PathBuf {
            self.0.join("arbeitsbereich.plnwrk")
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn cli(args: &[&str], passphrase: Option<&str>) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_plnwrk-cli"));
        command
            .args(args)
            .stdin(Stdio::null())
            .env_remove("PLNWRK_PASSPHRASE");
        if let Some(passphrase) = passphrase {
            command.env("PLNWRK_PASSPHRASE", passphrase);
        }
        command.output().unwrap()
    }

    fn ok(args: &[&str], passphrase: Option<&str>) -> Value {
        let output = cli(args, passphrase);
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        serde_json::from_slice(&output.stdout).unwrap_or(Value::Null)
    }

    fn fails(args: &[&str], passphrase: Option<&str>) -> Value {
        let output = cli(args, passphrase);
        assert!(!output.status.success(), "{args:?} should fail");
        serde_json::from_slice(&output.stdout).unwrap()
    }

    fn s(path: &Path) -> &str {
        path.to_str().unwrap()
    }

    fn project_id(file: &str, passphrase: Option<&str>) -> String {
        let overview = ok(&["query", file, "overview"], passphrase);
        overview[1]["id"].as_str().unwrap().to_owned()
    }

    #[test]
    fn create_change_and_query_a_file() {
        let dir = Dir::new("flow");
        let file = dir.file();
        ok(&["new", s(&file), "--sample"], None);
        let project = project_id(s(&file), None);
        let params = json!({"project": project, "title": "Funkgeräte Wache 23 prüfen"}).to_string();
        let result = ok(&["run", s(&file), "task.create", &params], None);
        assert_eq!(result["saved"], true);
        let board = ok(
            &[
                "query",
                s(&file),
                "board",
                &json!({"project": project}).to_string(),
            ],
            None,
        );
        let titles: Vec<&str> = board["columns"][0]["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|t| t["title"].as_str())
            .collect();
        assert!(titles.contains(&"Funkgeräte Wache 23 prüfen"));
        let verified = ok(&["verify", s(&file)], None);
        assert_eq!(verified["ok"], true);
    }

    #[test]
    fn existing_files_are_never_overwritten_by_new() {
        let dir = Dir::new("exists");
        let file = dir.file();
        ok(&["new", s(&file)], None);
        let before = std::fs::read(&file).unwrap();
        fails(&["new", s(&file), "--sample"], None);
        assert_eq!(std::fs::read(&file).unwrap(), before);
    }

    #[test]
    fn a_failing_command_leaves_the_file_untouched() {
        let dir = Dir::new("failing");
        let file = dir.file();
        ok(&["new", s(&file), "--sample"], None);
        let before = std::fs::read(&file).unwrap();
        let error = fails(&["run", s(&file), "task.create", r#"{"title": ""}"#], None);
        assert_eq!(error["error"], "bad_param");
        fails(&["run", s(&file), "task.create", "not json"], None);
        assert_eq!(std::fs::read(&file).unwrap(), before);
    }

    #[test]
    fn encryption_end_to_end() {
        let dir = Dir::new("crypt");
        let file = dir.file();
        ok(&["new", s(&file), "--sample"], None);
        ok(&["encrypt", s(&file)], Some("Lagezentrum"));
        let info = ok(&["info", s(&file)], None);
        assert_eq!(info["encrypted"], true);
        assert_eq!(
            fails(&["verify", s(&file)], None)["error"],
            "passphrase_required"
        );
        assert_eq!(
            fails(&["verify", s(&file)], Some("falsch"))["error"],
            "wrong_passphrase"
        );
        assert_eq!(ok(&["verify", s(&file)], Some("Lagezentrum"))["ok"], true);
        let raw = std::fs::read(&file).unwrap();
        assert!(!String::from_utf8_lossy(&raw).contains("Digitalfunk"));
        ok(&["decrypt", s(&file)], Some("Lagezentrum"));
        assert_eq!(ok(&["info", s(&file)], None)["encrypted"], false);
    }

    #[test]
    fn classified_content_requires_encryption() {
        let dir = Dir::new("vs");
        let file = dir.file();
        ok(&["new", s(&file)], None);
        let before = std::fs::read(&file).unwrap();
        let classified = json!({"title": "Lage Nord", "classification": "vs_nfd"}).to_string();
        let error = fails(&["run", s(&file), "project.create", &classified], None);
        assert_eq!(error["error"], "classified_unencrypted");
        assert_eq!(std::fs::read(&file).unwrap(), before);
        ok(&["encrypt", s(&file)], Some("geheim"));
        ok(
            &["run", s(&file), "project.create", &classified],
            Some("geheim"),
        );
        assert_eq!(ok(&["info", s(&file)], None)["classification"], "vs_nfd");
        assert_eq!(
            fails(&["decrypt", s(&file)], Some("geheim"))["error"],
            "classified_unencrypted"
        );
        let markdown = cli(
            &["export", s(&file), "--format", "markdown"],
            Some("geheim"),
        );
        assert!(
            String::from_utf8_lossy(&markdown.stdout)
                .starts_with("> **VS – NUR FÜR DEN DIENSTGEBRAUCH**")
        );
    }

    #[test]
    fn a_damaged_file_fails_verification() {
        let dir = Dir::new("damaged");
        let file = dir.file();
        ok(&["new", s(&file), "--sample"], None);
        let mut bytes = std::fs::read(&file).unwrap();
        let middle = bytes.len() / 2;
        bytes[middle] ^= 0xff;
        std::fs::write(&file, bytes).unwrap();
        let error = fails(&["verify", s(&file)], None);
        assert!(error["error"].is_string());
    }

    #[test]
    fn exports() {
        let dir = Dir::new("export");
        let file = dir.file();
        ok(&["new", s(&file), "--sample"], None);
        let csv = cli(&["export", s(&file), "--format", "csv"], None);
        assert!(String::from_utf8_lossy(&csv.stdout).starts_with("Projekt;Aktenzeichen;Aufgabe"));
        let json = ok(&["export", s(&file), "--format", "json"], None);
        assert!(json["projects"].is_object());
    }
}
