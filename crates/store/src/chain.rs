//! The change log on disk: one change per line, each sealed with a BLAKE3 hash over the previous
//! hash and **the exact bytes of the change as stored**. Changing, adding, removing, reordering or
//! inserting anything – a field, a line, even whitespace inside a change – breaks the chain.
//! Because the stored bytes are hashed (not a re-serialisation), the chain does not depend on how
//! a JSON library formats numbers or orders keys.
//!
//! Honest limit: in an unencrypted file someone who rewrites every line can also recompute every
//! hash; that is caught only by comparing with the stored state (and later by a signature,
//! PLAN.md Phase 7). In an encrypted file, age authenticates the whole content.

use plnwrk_engine::Change;
use serde::Deserialize;
use serde_json::value::RawValue;

use crate::StoreError;

/// The `prev` of the first line.
pub const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SealedLine<'a> {
    prev: String,
    hash: String,
    #[serde(borrow)]
    change: &'a RawValue,
}

fn hash(prev: &str, change: &[u8]) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(prev.as_bytes());
    hasher.update(b"\n");
    hasher.update(change);
    hasher.finalize().to_hex().to_string()
}

/// The log as JSON lines, and the hash of the last line (`None` for an empty log).
pub fn seal(log: &[Change]) -> Result<(String, Option<String>), StoreError> {
    let mut out = String::new();
    let mut prev = GENESIS.to_owned();
    for change in log {
        let json = serde_json::to_string(change)
            .map_err(|e| StoreError::corrupt(format!("change: {e}")))?;
        let hash = hash(&prev, json.as_bytes());
        out.push_str(&format!(
            "{{\"prev\":\"{prev}\",\"hash\":\"{hash}\",\"change\":{json}}}\n"
        ));
        prev = hash;
    }
    let head = (!log.is_empty()).then_some(prev);
    Ok((out, head))
}

/// Reads and verifies the log. `head` is the hash the manifest promises for the last line.
/// Every line ends with a line feed; empty lines are not allowed.
pub fn unseal(text: &str, head: Option<&str>) -> Result<Vec<Change>, StoreError> {
    if !text.is_empty() && !text.ends_with('\n') {
        return Err(StoreError::corrupt(
            "the log does not end with a line break",
        ));
    }
    let mut log = Vec::new();
    let mut prev = GENESIS.to_owned();
    for (index, line) in text.split_terminator('\n').enumerate() {
        let position = u64::try_from(index).unwrap_or(u64::MAX).saturating_add(1);
        let sealed: SealedLine<'_> = serde_json::from_str(line)
            .map_err(|e| StoreError::corrupt(format!("log line {position}: {e}")))?;
        let raw = sealed.change.get();
        if sealed.prev != prev || sealed.hash != hash(&prev, raw.as_bytes()) {
            return Err(StoreError::ChainBroken { seq: position });
        }
        // Outside the hashed change nothing may vary either: the line must be exactly what
        // `seal` writes (no extra spaces, no reordering, no escapes, no carriage return).
        let canonical = format!(
            "{{\"prev\":\"{}\",\"hash\":\"{}\",\"change\":{raw}}}",
            sealed.prev, sealed.hash
        );
        if line != canonical {
            return Err(StoreError::corrupt(format!(
                "log line {position} is not in the form plnwrk writes"
            )));
        }
        let change: Change = serde_json::from_str(raw)
            .map_err(|e| StoreError::corrupt(format!("log line {position}: {e}")))?;
        if change.seq != position {
            return Err(StoreError::corrupt(format!(
                "log line {position} carries change number {}",
                change.seq
            )));
        }
        prev = sealed.hash;
        log.push(change);
    }
    let actual = (!log.is_empty()).then_some(prev.as_str());
    if actual != head {
        let seq = u64::try_from(log.len()).unwrap_or(u64::MAX);
        return Err(StoreError::ChainBroken { seq });
    }
    Ok(log)
}

#[cfg(test)]
mod tests {
    use super::*;
    use plnwrk_core::{FixedClock, SequentialIds};
    use plnwrk_engine::{Origin, Session};
    use proptest::prelude::*;
    use serde_json::json;

    fn log() -> Vec<Change> {
        let mut s = Session::new(
            Box::new(FixedClock::at("2026-10-14").unwrap()),
            Box::new(SequentialIds::default()),
        );
        s.run("person.create", &json!({"name": "A"}), Origin::User)
            .unwrap();
        s.run("person.create", &json!({"name": "B"}), Origin::User)
            .unwrap();
        s.log().to_vec()
    }

    #[test]
    fn round_trip() {
        let log = log();
        let (text, head) = seal(&log).unwrap();
        assert_eq!(unseal(&text, head.as_deref()).unwrap(), log);
        assert_eq!(unseal("", None).unwrap(), Vec::new());
    }

    #[test]
    fn anything_added_inside_a_change_breaks_the_chain() {
        let (text, head) = seal(&log()).unwrap();
        let smuggled = text.replacen("\"command\":", "\"smuggled\":\"x\",\"command\":", 1);
        assert_ne!(smuggled, text);
        assert!(matches!(
            unseal(&smuggled, head.as_deref()),
            Err(StoreError::ChainBroken { seq: 1 })
        ));
        let spaced = text.replacen("\"command\":", "\"command\": ", 1);
        assert!(matches!(
            unseal(&spaced, head.as_deref()),
            Err(StoreError::ChainBroken { seq: 1 })
        ));
    }

    #[test]
    fn blank_lines_and_missing_final_break_are_refused() {
        let (text, head) = seal(&log()).unwrap();
        let blank = text.replacen('\n', "\n\n", 1);
        assert!(unseal(&blank, head.as_deref()).is_err());
        assert!(unseal(text.trim_end(), head.as_deref()).is_err());
    }

    #[test]
    fn a_wrong_change_number_is_refused_even_with_a_valid_chain() {
        let mut log = log();
        log[1].seq = 7;
        let (text, head) = seal(&log).unwrap();
        assert!(matches!(
            unseal(&text, head.as_deref()),
            Err(StoreError::Corrupt { .. })
        ));
    }

    #[test]
    fn variations_outside_the_hashed_change_are_refused() {
        let (text, head) = seal(&log()).unwrap();
        let crlf = text.replace('\n', "\r\n");
        let spaced = text.replacen("{\"prev\":", "{ \"prev\":", 1);
        let reordered = {
            let first = text.lines().next().unwrap();
            let value: serde_json::Value = serde_json::from_str(first).unwrap();
            let swapped = format!(
                "{{\"hash\":{},\"prev\":{},\"change\":{}}}",
                value["hash"], value["prev"], value["change"]
            );
            text.replacen(first, &swapped, 1)
        };
        for variant in [crlf, spaced, reordered] {
            assert!(unseal(&variant, head.as_deref()).is_err(), "{variant:?}");
        }
    }

    #[test]
    fn unknown_fields_in_a_line_are_refused() {
        let (text, head) = seal(&log()).unwrap();
        let extra = text.replacen("{\"prev\":", "{\"note\":1,\"prev\":", 1);
        assert!(matches!(
            unseal(&extra, head.as_deref()),
            Err(StoreError::Corrupt { .. })
        ));
    }

    proptest! {
        #[test]
        fn numbers_of_any_shape_in_params_survive(value in any::<f64>().prop_filter("finite", |f| f.is_finite()), big in any::<u64>()) {
            let mut log = log();
            log[0].params = json!({"f": value, "big": big, "text": "ä\u{1F692}\"\\"});
            let (text, head) = seal(&log).unwrap();
            prop_assert_eq!(unseal(&text, head.as_deref()).unwrap(), log);
        }
    }
}
