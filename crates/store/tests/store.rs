//! Saving and loading, and every way a file can be damaged or tampered with.

#[cfg(test)]
mod store {
    use std::collections::BTreeMap;
    use std::io::{Cursor, Read, Write};

    use plnwrk_core::{Classification, FixedClock, SequentialIds, Workspace};
    use plnwrk_engine::fixture::sample;
    use plnwrk_engine::{Origin, Session};
    use plnwrk_store::{
        Document, Limits, Manifest, Protection, SaveOptions, SecretString, StoreError, chain, load,
        load_with, peek, save, save_with,
    };
    use proptest::prelude::*;
    use serde_json::{Value, json};

    fn clock() -> Box<FixedClock> {
        Box::new(FixedClock::at("2026-10-14").unwrap())
    }

    fn sample_doc() -> Document {
        let mut session = Session::new(clock(), Box::new(SequentialIds::default()));
        sample(&mut session).unwrap();
        Document {
            workspace: session.workspace().clone(),
            log: session.log().to_vec(),
        }
    }

    fn pass(text: &str) -> SecretString {
        SecretString::from(text.to_owned())
    }

    fn fast() -> SaveOptions {
        SaveOptions {
            work_factor: Some(10),
        }
    }

    fn encrypted(doc: &Document, passphrase: &str) -> Vec<u8> {
        save_with(doc, &Protection::Passphrase(pass(passphrase)), fast()).unwrap()
    }

    fn entries(bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut out = BTreeMap::new();
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).unwrap();
            let name = file.name().unwrap().into_owned();
            let mut content = Vec::new();
            file.read_to_end(&mut content).unwrap();
            out.insert(name, content);
        }
        out
    }

    fn rebuild(entries: &BTreeMap<String, Vec<u8>>) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, content) in entries {
            writer
                .start_file(name.as_str(), zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(content).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn edit(bytes: &[u8], name: &str, change: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
        let mut all = entries(bytes);
        change(all.get_mut(name).unwrap());
        rebuild(&all)
    }

    fn edit_json(bytes: &[u8], name: &str, change: impl FnOnce(&mut Value)) -> Vec<u8> {
        edit(bytes, name, |content| {
            let mut value: Value = serde_json::from_slice(content).unwrap();
            change(&mut value);
            *content = serde_json::to_vec(&value).unwrap();
        })
    }

    fn classified() -> Document {
        let mut session = Session::new(clock(), Box::new(SequentialIds::default()));
        session
            .run(
                "project.create",
                &json!({"title": "Lage Nord", "classification": "vs_nfd"}),
                Origin::User,
            )
            .unwrap();
        Document {
            workspace: session.workspace().clone(),
            log: session.log().to_vec(),
        }
    }

    // ------------------------------------------------------------ round trips

    #[test]
    fn plain_round_trip_keeps_data_and_history() {
        let doc = sample_doc();
        let bytes = save(&doc, &Protection::None).unwrap();
        assert_eq!(load(&bytes, None).unwrap(), doc);
        let manifest = peek(&bytes).unwrap();
        assert!(!manifest.encrypted);
        assert_eq!(manifest.classification, Classification::Open);
        assert_eq!(
            manifest.changes,
            Some(u64::try_from(doc.log.len()).unwrap())
        );
        assert_eq!(
            save(&doc, &Protection::None).unwrap(),
            bytes,
            "saving is reproducible"
        );
    }

    #[test]
    fn work_continues_after_loading() {
        let doc = load(&save(&sample_doc(), &Protection::None).unwrap(), None).unwrap();
        let count = doc.log.len();
        let mut session = Session::with_workspace(
            doc.workspace,
            doc.log,
            clock(),
            Box::new(SequentialIds::starting_at(10_000)),
        );
        session
            .run("person.create", &json!({"name": "Neu Hinzu"}), Origin::User)
            .unwrap();
        assert_eq!(session.log().len(), count + 1);
        let again = Document {
            workspace: session.workspace().clone(),
            log: session.log().to_vec(),
        };
        let reloaded = load(&save(&again, &Protection::None).unwrap(), None).unwrap();
        assert_eq!(reloaded, again);
        session.run("undo", &json!({}), Origin::User).unwrap();
    }

    #[test]
    fn empty_workspace_round_trips() {
        let doc = Document::default();
        assert_eq!(
            load(&save(&doc, &Protection::None).unwrap(), None).unwrap(),
            doc
        );
    }

    // ------------------------------------------------------------ tampering

    #[test]
    fn a_changed_stored_state_is_detected() {
        let bytes = save(&sample_doc(), &Protection::None).unwrap();
        let tampered = edit_json(&bytes, "workspace.json", |ws| {
            let tasks = ws["tasks"].as_object_mut().unwrap();
            let first = tasks.values_mut().next().unwrap();
            first["title"] = json!("Heimlich geändert");
        });
        assert_eq!(load(&tampered, None), Err(StoreError::SnapshotMismatch));
    }

    #[test]
    fn a_changed_log_line_breaks_the_chain() {
        let bytes = save(&sample_doc(), &Protection::None).unwrap();
        let tampered = edit(&bytes, "log.jsonl", |log| {
            let text = String::from_utf8(log.clone()).unwrap();
            *log = text.replacen("Anna Berg", "Anna Borg", 1).into_bytes();
        });
        assert!(matches!(
            load(&tampered, None),
            Err(StoreError::ChainBroken { .. })
        ));
    }

    #[test]
    fn removed_or_swapped_log_lines_are_detected() {
        let bytes = save(&sample_doc(), &Protection::None).unwrap();
        let without_last = edit(&bytes, "log.jsonl", |log| {
            let text = String::from_utf8(log.clone()).unwrap();
            let mut lines: Vec<&str> = text.lines().collect();
            lines.pop();
            *log = (lines.join("\n") + "\n").into_bytes();
        });
        assert!(matches!(
            load(&without_last, None),
            Err(StoreError::ChainBroken { .. })
        ));
        let swapped = edit(&bytes, "log.jsonl", |log| {
            let text = String::from_utf8(log.clone()).unwrap();
            let mut lines: Vec<&str> = text.lines().collect();
            lines.swap(1, 2);
            *log = (lines.join("\n") + "\n").into_bytes();
        });
        assert!(matches!(
            load(&swapped, None),
            Err(StoreError::ChainBroken { .. })
        ));
    }

    #[test]
    fn a_recomputed_chain_with_an_old_state_is_detected() {
        // Someone rewrites history consistently (new hashes, new head) but keeps the state.
        let mut doc = sample_doc();
        let bytes = save(&doc, &Protection::None).unwrap();
        doc.log.truncate(doc.log.len() - 1);
        let (log, head) = chain::seal(&doc.log).unwrap();
        let tampered = edit(&bytes, "log.jsonl", |content| *content = log.into_bytes());
        let tampered = edit_json(&tampered, "manifest.json", |m| {
            m["head"] = json!(head);
            m["changes"] = json!(doc.log.len());
        });
        assert_eq!(load(&tampered, None), Err(StoreError::SnapshotMismatch));
    }

    #[test]
    fn an_entry_filed_under_another_id_is_refused() {
        let bytes = save(&sample_doc(), &Protection::None).unwrap();
        let tampered = edit(&bytes, "workspace.json", |content| {
            let text = String::from_utf8(content.clone()).unwrap();
            // Move the first person to another key; the replay will not match either.
            let replaced = text.replacen("\"people\":{\"0", "\"people\":{\"7", 1);
            assert_ne!(replaced, text);
            *content = replaced.into_bytes();
        });
        assert!(load(&tampered, None).is_err());
    }

    #[test]
    fn a_change_that_does_not_fit_its_past_is_refused() {
        let doc = sample_doc();
        // The first change again as change 2: it creates what already exists.
        let mut repeated = doc.log[0].clone();
        repeated.seq = 2;
        let log = vec![doc.log[0].clone(), repeated];
        assert!(matches!(
            plnwrk_store::replay(&log),
            Err(StoreError::Corrupt { .. })
        ));
    }

    /// Wraps an inner (plain) container the way `save` does: padded, age-encrypted, next to a
    /// readable head – so tests can hand-craft hostile inner content.
    fn wrap_encrypted(inner: &[u8], passphrase: &str) -> Vec<u8> {
        let mut padded = u64::try_from(inner.len()).unwrap().to_le_bytes().to_vec();
        padded.extend_from_slice(inner);
        while padded.len() % 4096 != 0 {
            padded.push(0);
        }
        let mut recipient = age::scrypt::Recipient::new(pass(passphrase));
        recipient.set_work_factor(10);
        let payload = age::encrypt(&recipient, &padded).unwrap();
        let mut head: Value = serde_json::from_slice(&entries(inner)["manifest.json"]).unwrap();
        head["encrypted"] = json!(true);
        if let Some(object) = head.as_object_mut() {
            object.remove("changes");
            object.remove("head");
        }
        rebuild(&BTreeMap::from([
            (
                "manifest.json".to_owned(),
                serde_json::to_vec(&head).unwrap(),
            ),
            ("payload.age".to_owned(), payload),
        ]))
    }

    #[test]
    fn a_wrong_change_count_in_the_head_is_refused() {
        let bytes = save(&sample_doc(), &Protection::None).unwrap();
        let tampered = edit_json(&bytes, "manifest.json", |m| m["changes"] = json!(1));
        assert!(matches!(
            load(&tampered, None),
            Err(StoreError::Corrupt { .. })
        ));
    }

    #[test]
    fn a_head_claiming_a_higher_classification_is_refused() {
        let bytes = save(&sample_doc(), &Protection::None).unwrap();
        let tampered = edit_json(&bytes, "manifest.json", |m| {
            m["classification"] = json!("vs_nfd")
        });
        assert!(matches!(
            load(&tampered, None),
            Err(StoreError::Corrupt { .. })
        ));
    }

    #[test]
    fn an_inner_head_claiming_encryption_is_refused() {
        let inner = save(&sample_doc(), &Protection::None).unwrap();
        let inner = edit_json(&inner, "manifest.json", |m| m["encrypted"] = json!(true));
        let file = wrap_encrypted(&inner, "richtig");
        assert!(matches!(
            load(&file, Some(&pass("richtig"))),
            Err(StoreError::Corrupt { .. })
        ));
        // The same wrapping with an honest inner head loads – the helper itself is sound.
        let honest = wrap_encrypted(&save(&sample_doc(), &Protection::None).unwrap(), "richtig");
        assert_eq!(load(&honest, Some(&pass("richtig"))).unwrap(), sample_doc());
    }

    #[test]
    fn a_consistent_file_that_breaks_rules_is_refused() {
        // Workspace and log agree perfectly, but the content breaks a rule (empty name);
        // `save` would never write this, so the file is hand-made.
        let person = plnwrk_core::Entity::Person(plnwrk_core::Person {
            id: plnwrk_core::PersonId::from_ulid(plnwrk_core::ids::Ulid(1)),
            name: String::new(),
            email: None,
            role: None,
            initials: "X".into(),
        });
        let change = plnwrk_engine::Change {
            seq: 1,
            at: jiff::Timestamp::UNIX_EPOCH,
            origin: Origin::Script,
            actor: None,
            command: "person.create".into(),
            params: json!({}),
            events: vec![plnwrk_engine::Event::Put {
                before: None,
                after: Box::new(person),
            }],
            undoes: None,
            redoes: None,
        };
        let log = vec![change];
        let workspace = plnwrk_store::replay(&log).unwrap();
        let (text, head) = chain::seal(&log).unwrap();
        let manifest = json!({
            "format": "plnwrk", "version": 1, "classification": "open", "encrypted": false,
            "created_with": "test", "changes": 1, "head": head
        });
        let file = rebuild(&BTreeMap::from([
            (
                "manifest.json".to_owned(),
                serde_json::to_vec(&manifest).unwrap(),
            ),
            (
                "workspace.json".to_owned(),
                serde_json::to_vec(&workspace).unwrap(),
            ),
            ("log.jsonl".to_owned(), text.into_bytes()),
        ]));
        assert!(matches!(load(&file, None), Err(StoreError::Invalid { .. })));
    }

    #[test]
    fn extra_entries_are_refused() {
        let plain = save(&sample_doc(), &Protection::None).unwrap();
        let mut all = entries(&plain);
        all.insert("notes.txt".to_owned(), b"hidden".to_vec());
        assert!(matches!(
            load(&rebuild(&all), None),
            Err(StoreError::Corrupt { .. })
        ));
        let encrypted_file = encrypted(&sample_doc(), "richtig");
        let mut all = entries(&encrypted_file);
        all.insert("workspace.json".to_owned(), b"{}".to_vec());
        assert!(matches!(
            load(&rebuild(&all), Some(&pass("richtig"))),
            Err(StoreError::Corrupt { .. })
        ));
    }

    #[test]
    fn the_readable_head_of_an_encrypted_file_may_not_describe_its_content() {
        let bytes = encrypted(&sample_doc(), "richtig");
        let tampered = edit_json(&bytes, "manifest.json", |m| {
            m["changes"] = json!(999);
            m["head"] = json!("deadbeef");
        });
        assert!(matches!(
            load(&tampered, Some(&pass("richtig"))),
            Err(StoreError::Corrupt { .. })
        ));
    }

    #[test]
    fn duplicate_entry_names_are_refused() {
        // Write a fourth entry under a placeholder name, then patch the name to an existing one.
        let plain = save(&sample_doc(), &Protection::None).unwrap();
        let mut all = entries(&plain);
        all.insert("log.jsonX".to_owned(), b"other".to_vec());
        let bytes = rebuild(&all);
        let patched: Vec<u8> = {
            let needle = b"log.jsonX";
            let mut out = bytes.clone();
            let mut at = 0;
            while let Some(position) = out[at..].windows(needle.len()).position(|w| w == needle) {
                let start = at + position;
                out[start + needle.len() - 1] = b'l';
                at = start + needle.len();
            }
            out
        };
        assert_ne!(patched, bytes);
        assert!(matches!(
            load(&patched, None),
            Err(StoreError::Corrupt { .. })
        ));
    }

    #[test]
    fn an_oversized_head_is_refused_by_peek() {
        let plain = save(&sample_doc(), &Protection::None).unwrap();
        let tampered = edit_json(&plain, "manifest.json", |m| {
            m["created_with"] = json!("x".repeat(70_000))
        });
        assert!(matches!(peek(&tampered), Err(StoreError::TooLarge { .. })));
    }

    // ------------------------------------------------------------ foreign and broken files

    #[test]
    fn foreign_files_are_refused() {
        assert_eq!(load(b"", None), Err(StoreError::NotPlnwrk));
        assert_eq!(load(b"hello world", None), Err(StoreError::NotPlnwrk));
        let other_zip = rebuild(&BTreeMap::from([("readme.txt".to_owned(), b"hi".to_vec())]));
        assert_eq!(load(&other_zip, None), Err(StoreError::NotPlnwrk));
    }

    #[test]
    fn newer_versions_ask_for_an_update() {
        let bytes = save(&sample_doc(), &Protection::None).unwrap();
        let newer = edit_json(&bytes, "manifest.json", |m| m["version"] = json!(2));
        assert_eq!(
            load(&newer, None),
            Err(StoreError::NewerFormat {
                found: 2,
                supported: 1
            })
        );
        assert_eq!(
            peek(&newer),
            Err(StoreError::NewerFormat {
                found: 2,
                supported: 1
            })
        );
    }

    #[test]
    fn size_limits_hold() {
        let bytes = save(&sample_doc(), &Protection::None).unwrap();
        let small = Limits {
            max_bytes: 1_000,
            ..Limits::default()
        };
        assert!(matches!(
            load_with(&bytes, None, small),
            Err(StoreError::TooLarge { .. })
        ));
        let few = Limits {
            max_entries: 2,
            ..Limits::default()
        };
        assert!(matches!(
            load_with(&bytes, None, few),
            Err(StoreError::TooLarge { .. })
        ));
    }

    #[test]
    fn a_zip_bomb_stops_at_the_limit() {
        // 64 MiB of zeros compress to a few kilobytes.
        let zeros = vec![0u8; 64 * 1024 * 1024];
        let bomb = rebuild(&BTreeMap::from([
            ("manifest.json".to_owned(), b"{}".to_vec()),
            ("workspace.json".to_owned(), zeros),
        ]));
        assert!(bomb.len() < 1_000_000);
        let limits = Limits {
            max_bytes: 1024 * 1024,
            ..Limits::default()
        };
        assert!(matches!(
            load_with(&bomb, None, limits),
            Err(StoreError::TooLarge { .. })
        ));
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 200, failure_persistence: None, ..ProptestConfig::default() })]

        #[test]
        fn damaged_bytes_never_panic_and_never_give_other_data(
            flips in prop::collection::vec((any::<prop::sample::Index>(), any::<u8>()), 1..4),
            cut in prop::option::of(any::<prop::sample::Index>()),
        ) {
            let doc = sample_doc();
            let mut bytes = save(&doc, &Protection::None).unwrap();
            for (index, value) in flips {
                let at = index.index(bytes.len());
                bytes[at] ^= value | 1;
            }
            if let Some(cut) = cut {
                bytes.truncate(cut.index(bytes.len()));
            }
            if let Ok(loaded) = load(&bytes, None) {
                prop_assert_eq!(loaded, doc);
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 64, failure_persistence: None, ..ProptestConfig::default() })]

        #[test]
        fn damaged_encrypted_files_never_panic_and_never_give_other_data(
            flips in prop::collection::vec((any::<prop::sample::Index>(), any::<u8>()), 1..4),
        ) {
            let doc = sample_doc();
            let mut bytes = encrypted(&doc, "richtig");
            for (index, value) in flips {
                let at = index.index(bytes.len());
                bytes[at] ^= value | 1;
            }
            if let Ok(loaded) = load(&bytes, Some(&pass("richtig"))) {
                prop_assert_eq!(loaded, doc);
            }
        }
    }

    // ------------------------------------------------------------ encryption and classification

    #[test]
    fn encrypted_round_trip_and_readable_head() {
        let doc = sample_doc();
        let bytes = encrypted(&doc, "richtig");
        assert_eq!(load(&bytes, Some(&pass("richtig"))).unwrap(), doc);
        let head: Manifest = peek(&bytes).unwrap();
        assert!(head.encrypted);
        assert_eq!(
            (head.changes, head.head),
            (None, None),
            "no metadata leaks outside"
        );
        let outside = entries(&bytes);
        assert_eq!(
            outside.keys().collect::<Vec<_>>(),
            ["manifest.json", "payload.age"]
        );
        let raw = String::from_utf8_lossy(&bytes);
        assert!(!raw.contains("Digitalfunk"), "no content in the clear");
    }

    #[test]
    fn wrong_or_missing_passphrase() {
        let bytes = encrypted(&sample_doc(), "richtig");
        assert_eq!(
            load(&bytes, Some(&pass("falsch"))),
            Err(StoreError::WrongPassphrase)
        );
        assert_eq!(load(&bytes, None), Err(StoreError::PassphraseRequired));
    }

    #[test]
    fn changed_ciphertext_is_detected() {
        let bytes = encrypted(&sample_doc(), "richtig");
        let tampered = edit(&bytes, "payload.age", |payload| {
            let middle = payload.len() / 2;
            payload[middle] ^= 0x01;
        });
        assert!(load(&tampered, Some(&pass("richtig"))).is_err());
    }

    #[test]
    fn classified_content_is_never_stored_in_the_clear() {
        let doc = classified();
        assert_eq!(
            save(&doc, &Protection::None),
            Err(StoreError::ClassifiedUnencrypted)
        );
        let bytes = encrypted(&doc, "geheim");
        assert_eq!(peek(&bytes).unwrap().classification, Classification::VsNfd);
        assert_eq!(load(&bytes, Some(&pass("geheim"))).unwrap(), doc);
        // The decrypted inner container on its own is refused.
        let payload = entries(&bytes).remove("payload.age").unwrap();
        let identity = age::scrypt::Identity::new(pass("geheim"));
        let padded = age::decrypt(&identity, &payload).unwrap();
        let length = usize::try_from(u64::from_le_bytes(padded[..8].try_into().unwrap())).unwrap();
        let inner = &padded[8..8 + length];
        assert_eq!(load(inner, None), Err(StoreError::ClassifiedUnencrypted));
    }

    #[test]
    fn classified_history_keeps_the_file_classified() {
        let mut session = Session::new(clock(), Box::new(SequentialIds::default()));
        let created = session
            .run("project.create", &json!({"title": "Übung"}), Origin::User)
            .unwrap();
        let project = serde_json::to_value(created.created[0]).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let task = session
            .run(
                "task.create",
                &json!({"project": project, "title": "Bunker 7", "classification": "vs_nfd"}),
                Origin::User,
            )
            .unwrap();
        let task = serde_json::to_value(task.created[0]).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        session
            .run("task.delete", &json!({"task": task}), Origin::User)
            .unwrap();
        let doc = Document {
            workspace: session.workspace().clone(),
            log: session.log().to_vec(),
        };
        assert_eq!(doc.workspace.highest_classification(), Classification::Open);
        assert_eq!(plnwrk_store::classification_of(&doc), Classification::VsNfd);
        assert_eq!(
            save(&doc, &Protection::None),
            Err(StoreError::ClassifiedUnencrypted)
        );
        let bytes = encrypted(&doc, "richtig-und-lang");
        assert_eq!(peek(&bytes).unwrap().classification, Classification::VsNfd);
    }

    #[test]
    fn a_downgraded_head_is_detected() {
        let bytes = encrypted(&classified(), "geheim");
        let downgraded = edit_json(&bytes, "manifest.json", |m| {
            m["classification"] = json!("open")
        });
        assert!(matches!(
            load(&downgraded, Some(&pass("geheim"))),
            Err(StoreError::Corrupt { .. })
        ));
    }

    #[test]
    fn any_mismatch_between_head_and_encrypted_content_is_detected() {
        let bytes = encrypted(&sample_doc(), "richtig");
        let upgraded = edit_json(&bytes, "manifest.json", |m| {
            m["classification"] = json!("vs_nfd")
        });
        assert!(matches!(
            load(&upgraded, Some(&pass("richtig"))),
            Err(StoreError::Corrupt { .. })
        ));
        let other_program = edit_json(&bytes, "manifest.json", |m| m["created_with"] = json!("x"));
        assert!(matches!(
            load(&other_program, Some(&pass("richtig"))),
            Err(StoreError::Corrupt { .. })
        ));
    }

    #[test]
    fn broken_data_is_never_written() {
        let mut ws = Workspace::default();
        ws.put(plnwrk_core::Entity::Person(plnwrk_core::Person {
            id: plnwrk_core::PersonId::from_ulid(plnwrk_core::ids::Ulid(1)),
            name: String::new(),
            email: None,
            role: None,
            initials: "X".into(),
        }));
        let doc = Document {
            workspace: ws,
            log: Vec::new(),
        };
        assert!(matches!(
            save(&doc, &Protection::None),
            Err(StoreError::Invalid { .. })
        ));
    }

    #[test]
    fn exports_of_the_sample() {
        let doc = sample_doc();
        let markdown = plnwrk_store::export::markdown(&doc.workspace);
        assert!(
            !markdown.contains("SKA beantworten"),
            "templates are left out"
        );
        insta::assert_snapshot!("markdown", markdown);
        let csv = plnwrk_store::export::csv(&doc.workspace);
        assert_eq!(csv.lines().count(), 1 + 10, "header and one row per task");
        insta::assert_snapshot!("csv", csv);
        let json: Value =
            serde_json::from_str(&plnwrk_store::export::json(&doc.workspace).unwrap()).unwrap();
        assert!(
            json["workspace"]["tasks"]
                .as_object()
                .is_some_and(|t| t.len() == 14)
        );
    }

    #[test]
    fn classified_exports_are_marked() {
        let doc = classified();
        let mark = plnwrk_store::export::VS_NFD_MARK;
        assert!(plnwrk_store::export::csv(&doc.workspace).starts_with(&format!("\u{FEFF}{mark}")));
        let json: Value =
            serde_json::from_str(&plnwrk_store::export::json(&doc.workspace).unwrap()).unwrap();
        assert_eq!(json["marking"], mark);
        assert!(
            plnwrk_store::export::markdown(&doc.workspace).starts_with(&format!("> **{mark}**"))
        );
        let open = sample_doc();
        assert!(!plnwrk_store::export::csv(&open.workspace).contains(mark));
        let json: Value =
            serde_json::from_str(&plnwrk_store::export::json(&open.workspace).unwrap()).unwrap();
        assert_eq!(json["marking"], Value::Null);
    }

    #[test]
    fn a_single_classified_task_in_an_open_project_is_marked() {
        let mut session = Session::new(clock(), Box::new(SequentialIds::default()));
        let created = session
            .run("project.create", &json!({"title": "Offen"}), Origin::User)
            .unwrap();
        let project = serde_json::to_value(created.created[0]).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        session
            .run(
                "task.create",
                &json!({"project": project, "title": "Geheim", "classification": "vs_nfd"}),
                Origin::User,
            )
            .unwrap();
        session
            .run(
                "task.create",
                &json!({"project": project, "title": "Offen"}),
                Origin::User,
            )
            .unwrap();
        let markdown = plnwrk_store::export::markdown(session.workspace());
        assert!(markdown.contains("- [ ] Geheim — VS-NfD"));
        assert!(markdown.contains("- [ ] Offen\n"));
        let csv = plnwrk_store::export::csv(session.workspace());
        assert!(
            csv.lines()
                .any(|l| l.contains(";Geheim;") && l.ends_with(";vs_nfd"))
        );
    }

    #[test]
    fn deep_task_trees_are_exported_completely() {
        let mut session = Session::new(clock(), Box::new(SequentialIds::default()));
        let created = session
            .run("project.create", &json!({"title": "Tief"}), Origin::User)
            .unwrap();
        let project = serde_json::to_value(created.created[0]).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let mut parent: Option<String> = None;
        for level in 0..40 {
            let mut params = json!({"project": project, "title": format!("Ebene {level}")});
            if let Some(parent) = &parent {
                params["parent"] = json!(parent);
            }
            let created = session.run("task.create", &params, Origin::User).unwrap();
            parent = serde_json::to_value(created.created[0]).unwrap()["id"]
                .as_str()
                .map(str::to_owned);
        }
        let markdown = plnwrk_store::export::markdown(session.workspace());
        assert_eq!(markdown.matches("- [ ] Ebene").count(), 40);
        assert!(markdown.contains(&format!("{}- [ ] Ebene 39", "  ".repeat(39))));
    }

    #[test]
    fn errors_serialize_for_agents() {
        let json = serde_json::to_value(StoreError::NewerFormat {
            found: 2,
            supported: 1,
        })
        .unwrap();
        assert_eq!(json["error"], "newer_format");
        assert_eq!(
            StoreError::WrongPassphrase.to_string(),
            "the passphrase is wrong"
        );
    }
}
