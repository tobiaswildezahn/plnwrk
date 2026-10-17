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

    fn project_id_any(file: &str, passphrase: Option<&str>) -> String {
        let overview = ok(&["query", file, "overview"], passphrase);
        overview[0]["id"].as_str().unwrap().to_owned()
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
        ok(&["encrypt", s(&file)], Some("Lagezentrum-2026"));
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
        assert_eq!(
            ok(&["verify", s(&file)], Some("Lagezentrum-2026"))["ok"],
            true
        );
        let raw = std::fs::read(&file).unwrap();
        assert!(!String::from_utf8_lossy(&raw).contains("Digitalfunk"));
        ok(&["decrypt", s(&file)], Some("Lagezentrum-2026"));
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
        assert_eq!(error["error"], "not_saved");
        assert_eq!(error["cause"]["error"], "classified_unencrypted");
        assert_eq!(std::fs::read(&file).unwrap(), before);
        ok(&["encrypt", s(&file)], Some("geheim-und-lang"));
        ok(
            &["run", s(&file), "project.create", &classified],
            Some("geheim-und-lang"),
        );
        assert_eq!(ok(&["info", s(&file)], None)["classification"], "vs_nfd");
        assert_eq!(
            fails(&["decrypt", s(&file)], Some("geheim-und-lang"))["error"],
            "classified_unencrypted"
        );
        let markdown = cli(
            &["export", s(&file), "--format", "markdown"],
            Some("geheim-und-lang"),
        );
        assert!(
            String::from_utf8_lossy(&markdown.stdout)
                .starts_with("> **VS – NUR FÜR DEN DIENSTGEBRAUCH**")
        );
    }

    #[test]
    fn classified_history_keeps_the_file_classified() {
        // Review finding B1: a VS-NfD task that is later deleted (or a project that is later
        // declassified) still sits in the change log; the file must stay VS-NfD.
        for undo_it in ["task.delete", "project.update"] {
            let dir = Dir::new(&format!("sticky-{undo_it}"));
            let file = dir.file();
            ok(&["new", s(&file), "--encrypt"], Some("Lagezentrum-2026"));
            let project = json!({"title": "Übung", "classification": "open"}).to_string();
            ok(
                &["run", s(&file), "project.create", &project],
                Some("Lagezentrum-2026"),
            );
            let p = project_id_any(s(&file), Some("Lagezentrum-2026"));
            if undo_it == "task.delete" {
                let secret = json!({"project": p, "title": "Standort Gefechtsstand Bunker 7", "classification": "vs_nfd"}).to_string();
                let created = ok(
                    &["run", s(&file), "task.create", &secret],
                    Some("Lagezentrum-2026"),
                );
                let task = created["outcome"]["created"][0]["id"]
                    .as_str()
                    .unwrap()
                    .to_owned();
                ok(
                    &[
                        "run",
                        s(&file),
                        "task.delete",
                        &json!({"task": task}).to_string(),
                    ],
                    Some("Lagezentrum-2026"),
                );
            } else {
                let up = json!({"project": p, "classification": "vs_nfd", "description": "Standort Gefechtsstand Bunker 7"}).to_string();
                ok(
                    &["run", s(&file), "project.update", &up],
                    Some("Lagezentrum-2026"),
                );
                let down =
                    json!({"project": p, "classification": "open", "description": ""}).to_string();
                ok(
                    &["run", s(&file), "project.update", &down],
                    Some("Lagezentrum-2026"),
                );
            }
            assert_eq!(
                ok(&["info", s(&file)], None)["classification"],
                "vs_nfd",
                "{undo_it}"
            );
            assert_eq!(
                fails(&["decrypt", s(&file)], Some("Lagezentrum-2026"))["error"],
                "classified_unencrypted",
                "{undo_it}"
            );
        }
    }

    #[test]
    fn passphrase_rules_and_guards() {
        let dir = Dir::new("rules");
        let file = dir.file();
        ok(&["new", s(&file), "--sample"], None);
        let short = fails(&["encrypt", s(&file)], Some("zu-kurz"));
        assert!(short["message"].as_str().unwrap().contains("12"));
        assert_eq!(ok(&["info", s(&file)], None)["encrypted"], false);
        assert!(
            fails(&["decrypt", s(&file)], None)["message"]
                .as_str()
                .unwrap()
                .contains("not encrypted")
        );
        ok(&["encrypt", s(&file)], Some("Lagezentrum-2026"));
        assert!(
            fails(&["encrypt", s(&file)], Some("Lagezentrum-2026"))["message"]
                .as_str()
                .unwrap()
                .contains("already encrypted")
        );
        // verify reports the verified inner head, including the change count.
        let verified = ok(&["verify", s(&file)], Some("Lagezentrum-2026"));
        assert_eq!(verified["manifest"]["encrypted"], true);
        assert!(
            verified["manifest"]["changes"]
                .as_u64()
                .is_some_and(|n| n > 0)
        );
        // The readable head alone says nothing about the content.
        assert!(ok(&["info", s(&file)], None).get("changes").is_none());
    }

    #[test]
    fn passphrase_length_is_counted_in_characters_after_normalisation() {
        let dir = Dir::new("length");
        let file = dir.file();
        ok(&["new", s(&file)], None);
        fails(&["encrypt", s(&file)], Some("12345678901"));
        // Eleven umlauts are 22 bytes but only 11 characters; decomposed they are 22 code points.
        fails(&["encrypt", s(&file)], Some(&"ä".repeat(11)));
        fails(&["encrypt", s(&file)], Some(&"a\u{0308}".repeat(11)));
        ok(&["encrypt", s(&file)], Some("123456789012"));
    }

    #[test]
    fn a_locked_file_is_not_changed_by_a_second_program() {
        let dir = Dir::new("locked");
        let file = dir.file();
        ok(&["new", s(&file), "--sample"], None);
        let before = std::fs::read(&file).unwrap();
        let held = plnwrk_store::file::lock(&plnwrk_store::file::resolve(&file).unwrap()).unwrap();
        let project = project_id(s(&file), None);
        let params = json!({"project": project, "title": "parallel"}).to_string();
        let busy = fails(&["run", s(&file), "task.create", &params], None);
        assert_eq!(busy["error"], "in_use", "{busy}");
        assert_eq!(
            fails(&["encrypt", s(&file)], Some("Lagezentrum-2026"))["error"],
            "in_use"
        );
        assert_eq!(std::fs::read(&file).unwrap(), before);
        drop(held);
        ok(&["run", s(&file), "task.create", &params], None);
    }

    #[test]
    fn new_never_replaces_a_file_that_appears_meanwhile() {
        // Without any early check in the CLI, only the never-overwrite write protects the file.
        let dir = Dir::new("appears");
        let file = dir.file();
        std::fs::write(&file, b"fremde Datei").unwrap();
        fails(&["new", s(&file), "--sample"], None);
        assert_eq!(std::fs::read(&file).unwrap(), b"fremde Datei");
    }

    #[cfg(unix)]
    #[test]
    fn a_link_to_a_file_is_followed_not_replaced() {
        let dir = Dir::new("link");
        let real = dir.0.join("real.plnwrk");
        ok(&["new", s(&real), "--sample"], None);
        let link = dir.0.join("link.plnwrk");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let project = project_id(s(&link), None);
        ok(
            &[
                "run",
                s(&link),
                "task.create",
                &json!({"project": project, "title": "über Link"}).to_string(),
            ],
            None,
        );
        assert!(
            std::fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let board = ok(
            &[
                "query",
                s(&real),
                "board",
                &json!({"project": project}).to_string(),
            ],
            None,
        );
        assert!(board.to_string().contains("über Link"));
    }

    #[test]
    fn a_passphrase_for_an_unencrypted_file_triggers_a_warning() {
        let dir = Dir::new("warn");
        let file = dir.file();
        ok(&["new", s(&file), "--sample"], None);
        let project = project_id(s(&file), None);
        let params = json!({"project": project, "title": "x"}).to_string();
        let output = cli(
            &["run", s(&file), "task.create", &params],
            Some("Lagezentrum-2026"),
        );
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("saved unencrypted"));
        // Encrypting with the variable set is exactly what it is for: no warning.
        let output = cli(&["encrypt", s(&file)], Some("Lagezentrum-2026"));
        assert!(output.status.success());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("warning"));
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
        assert!(
            String::from_utf8_lossy(&csv.stdout)
                .starts_with("\u{FEFF}Projekt;Aktenzeichen;Aufgabe")
        );
        let json = ok(&["export", s(&file), "--format", "json"], None);
        assert!(json["workspace"]["projects"].is_object());
        assert_eq!(json["marking"], Value::Null);
    }

    #[test]
    fn changing_a_version_1_file_upgrades_it() {
        let dir = Dir::new("v1");
        let file = dir.file();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/store/tests/fixtures/v1-sample.plnwrk");
        std::fs::copy(&fixture, &file).unwrap();
        assert_eq!(ok(&["info", s(&file)], None)["version"], 1);
        // Reading leaves it as it is.
        ok(&["verify", s(&file)], None);
        assert_eq!(ok(&["info", s(&file)], None)["version"], 1);
        // Changing it converts the old milestones in the same save.
        let project = project_id_any(s(&file), None);
        let params = json!({"project": project, "title": "Neu"}).to_string();
        let result = ok(&["run", s(&file), "task.create", &params], None);
        assert_eq!(result["upgraded"], true);
        assert_eq!(result["saved"], true);
        assert_eq!(ok(&["info", s(&file)], None)["version"], 2);
        let tasks = ok(
            &["query", s(&file), "tasks", r#"{"include_done": true}"#],
            None,
        );
        let milestones = tasks[0]["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|t| t["milestone"] == true)
            .count();
        assert_eq!(milestones, 3, "two in the project, one in the case");
        // A second change has nothing more to convert.
        let params = json!({"project": project, "title": "Noch eins"}).to_string();
        assert_eq!(
            ok(&["run", s(&file), "task.create", &params], None)["upgraded"],
            false
        );
    }

    fn v1_copy(dir: &Dir) -> PathBuf {
        let file = dir.file();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/store/tests/fixtures/v1-sample.plnwrk");
        std::fs::copy(&fixture, &file).unwrap();
        file
    }

    #[test]
    fn queries_and_exports_of_a_version_1_file_keep_its_milestones() {
        let dir = Dir::new("v1-read");
        let file = v1_copy(&dir);
        let overview = ok(&["query", s(&file), "overview"], None);
        assert!(
            overview
                .as_array()
                .unwrap()
                .iter()
                .any(|card| !card["next_milestone"].is_null())
        );
        let markdown = cli(&["export", s(&file), "--format", "markdown"], None);
        assert!(String::from_utf8_lossy(&markdown.stdout).contains("### Meilensteine"));
        // Reading never writes.
        assert_eq!(ok(&["info", s(&file)], None)["version"], 1);
    }

    #[test]
    fn undo_cannot_reach_past_the_upgrade_and_an_explicit_upgrade_saves() {
        let dir = Dir::new("v1-undo");
        let file = v1_copy(&dir);
        let before = std::fs::read(&file).unwrap();
        fails(&["run", s(&file), "undo"], None);
        assert_eq!(std::fs::read(&file).unwrap(), before, "file untouched");
        let result = ok(&["run", s(&file), "workspace.upgrade"], None);
        assert_eq!(result["upgraded"], true);
        assert_eq!(result["saved"], true);
        assert_eq!(ok(&["info", s(&file)], None)["version"], 2);
        ok(&["verify", s(&file)], None);
    }

    fn milestone_ids(file: &str) -> Vec<String> {
        let tasks = ok(&["query", file, "tasks", r#"{"include_done": true}"#], None);
        let mut ids: Vec<String> = tasks[0]["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|t| t["milestone"] == true)
            .map(|t| t["id"].as_str().unwrap().to_owned())
            .collect();
        ids.sort();
        ids
    }

    #[test]
    fn reading_a_version_1_file_twice_gives_the_same_ids() {
        let dir = Dir::new("v1-ids");
        let file = v1_copy(&dir);
        let first = milestone_ids(s(&file));
        assert_eq!(first.len(), 3);
        assert_eq!(milestone_ids(s(&file)), first, "stable between calls");
        // An id from a query can be used to change the file.
        let params = json!({"task": first[0], "title": "Zuschlag (neu)"}).to_string();
        ok(&["run", s(&file), "task.update", &params], None);
        assert_eq!(milestone_ids(s(&file)), first, "the same ids after saving");
        let markdown = |f: &Path| {
            String::from_utf8(cli(&["export", s(f), "--format", "json"], None).stdout).unwrap()
        };
        let other = Dir::new("v1-ids-2");
        let copy = v1_copy(&other);
        assert_eq!(markdown(&copy), markdown(&copy), "exports are repeatable");
    }

    #[test]
    fn encrypting_a_version_1_file_writes_it_in_the_current_format() {
        let dir = Dir::new("v1-encrypt");
        let file = v1_copy(&dir);
        let pass = "lange-passphrase-123";
        ok(&["encrypt", s(&file)], Some(pass));
        let bytes = std::fs::read(&file).unwrap();
        let secret = plnwrk_store::SecretString::from(pass.to_owned());
        let opened = plnwrk_store::open(&bytes, Some(&secret)).unwrap();
        assert_eq!(opened.manifest.version, plnwrk_store::manifest::VERSION);
        assert_eq!(opened.document.workspace.milestones().count(), 0);
        assert_eq!(
            opened
                .document
                .workspace
                .tasks()
                .filter(|t| t.milestone)
                .count(),
            4
        );
    }
    /// Phase 9d: a milestone changes only with --confirmed, which the AI sets after asking the
    /// person in the chat; without it nothing is saved and the answer names the milestone.
    #[test]
    fn a_milestone_changes_only_when_confirmed() {
        let dir = Dir::new("milestone");
        let file = dir.file();
        ok(&["new", s(&file), "--sample"], None);
        let project = project_id(s(&file), None);
        // Creating a milestone asks nothing.
        let params = json!({"project": project, "title": "Abnahme", "milestone": true,
                            "due": "2026-11-20"})
        .to_string();
        let created = ok(&["run", s(&file), "task.create", &params], None);
        let task = created["outcome"]["created"][0]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let before = std::fs::read(&file).unwrap();
        let change = json!({"task": task, "due": "2026-11-27"}).to_string();
        let refused = fails(&["run", s(&file), "task.update", &change], None);
        assert_eq!(refused["error"], "milestone_not_confirmed");
        assert_eq!(refused["milestones"][0]["title"], "Abnahme");
        assert_eq!(
            std::fs::read(&file).unwrap(),
            before,
            "nothing may be saved"
        );
        let done = ok(
            &["run", s(&file), "task.update", &change, "--confirmed"],
            None,
        );
        assert_eq!(done["saved"], true);
        assert_ne!(std::fs::read(&file).unwrap(), before);
        // A normal task needs no confirmation.
        let params = json!({"project": project, "title": "Funk prüfen"}).to_string();
        let normal = ok(&["run", s(&file), "task.create", &params], None);
        let normal = normal["outcome"]["created"][0]["id"].as_str().unwrap();
        let change = json!({"task": normal, "due": "2026-11-27"}).to_string();
        ok(&["run", s(&file), "task.update", &change], None);
    }
}
