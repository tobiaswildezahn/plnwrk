//! The change log on disk: one change per line, each sealed with a BLAKE3 hash over the previous
//! hash and the change. Altering, removing, reordering or inserting a line breaks the chain.
//!
//! Honest limit: in an unencrypted file someone who rewrites every line can also recompute every
//! hash; that is caught only by comparing with the stored state (and later by a signature,
//! PLAN.md Phase 7). In an encrypted file, age authenticates the whole content.

use plnwrk_engine::Change;
use serde::{Deserialize, Serialize};

use crate::StoreError;

/// The `prev` of the first line.
pub const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SealedLine {
    prev: String,
    hash: String,
    change: Change,
}

fn hash(prev: &str, change: &Change) -> Result<String, StoreError> {
    let json =
        serde_json::to_vec(change).map_err(|e| StoreError::corrupt(format!("change: {e}")))?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(prev.as_bytes());
    hasher.update(b"\n");
    hasher.update(&json);
    Ok(hasher.finalize().to_hex().to_string())
}

/// The log as JSON lines, and the hash of the last line (`None` for an empty log).
pub fn seal(log: &[Change]) -> Result<(String, Option<String>), StoreError> {
    let mut out = String::new();
    let mut prev = GENESIS.to_owned();
    for change in log {
        let hash = hash(&prev, change)?;
        let line = SealedLine {
            prev: prev.clone(),
            hash: hash.clone(),
            change: change.clone(),
        };
        out.push_str(
            &serde_json::to_string(&line).map_err(|e| StoreError::corrupt(format!("log: {e}")))?,
        );
        out.push('\n');
        prev = hash;
    }
    let head = (!log.is_empty()).then_some(prev);
    Ok((out, head))
}

/// Reads and verifies the log. `head` is the hash the manifest promises for the last line.
pub fn unseal(text: &str, head: Option<&str>) -> Result<Vec<Change>, StoreError> {
    let mut log = Vec::new();
    let mut prev = GENESIS.to_owned();
    for (index, line) in text.lines().filter(|l| !l.trim().is_empty()).enumerate() {
        let position = u64::try_from(index).unwrap_or(u64::MAX).saturating_add(1);
        let sealed: SealedLine = serde_json::from_str(line)
            .map_err(|e| StoreError::corrupt(format!("log line {position}: {e}")))?;
        if sealed.prev != prev || sealed.hash != hash(&prev, &sealed.change)? {
            return Err(StoreError::ChainBroken { seq: position });
        }
        if sealed.change.seq != position {
            return Err(StoreError::corrupt(format!(
                "log line {position} carries change number {}",
                sealed.change.seq
            )));
        }
        prev = sealed.hash;
        log.push(sealed.change);
    }
    let actual = (!log.is_empty()).then_some(prev.as_str());
    if actual != head {
        let seq = u64::try_from(log.len()).unwrap_or(u64::MAX);
        return Err(StoreError::ChainBroken { seq });
    }
    Ok(log)
}
