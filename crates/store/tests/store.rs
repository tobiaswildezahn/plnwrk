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
        let inner = age::decrypt(&identity, &payload).unwrap();
        assert_eq!(load(&inner, None), Err(StoreError::ClassifiedUnencrypted));
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
