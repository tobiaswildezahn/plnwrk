//! Browser: a recovery copy of the open work in the browser's own storage, so a reload, a
//! closed tab or a crash loses at most a second of work. The app decides when to write; the
//! browser only keeps keys and values ([`Host::store`]). The desktop uses
//! [`crate::autosave`] with files instead.
//!
//! Encrypted work is not copied: encrypting takes seconds in the browser, and its plain
//! content does not belong in the browser's storage. VS-NfD never gets here (decision 4.1).

use base64::Engine as _;
use serde_json::{Value, json};

use crate::autosave::Written;
use crate::doc::OpenDoc;
use crate::host::{Host, Upload};
use crate::text;

/// Every key of a copy starts with this.
pub const PREFIX: &str = "plnwrk.copy.";
/// Seconds at least between two writes while changes go on.
const PAUSE: f64 = 1.0;
/// Seconds to wait after a write failed (e.g. the storage is full).
const RETRY: f64 = 10.0;

/// A copy as stored: its name and the bytes of the file.
pub fn encode(name: &str, bytes: &[u8]) -> String {
    json!({
        "name": name,
        "data": base64::engine::general_purpose::STANDARD.encode(bytes),
    })
    .to_string()
}

/// A stored copy; `None` if it is not one (damaged, or written by something else).
pub fn decode(value: &str) -> Option<Upload> {
    let value: Value = serde_json::from_str(value).ok()?;
    let name = value.get("name")?.as_str()?.to_owned();
    let data = value.get("data")?.as_str()?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .ok()?;
    Some(Upload { name, bytes })
}

/// The copies in the browser's storage, newest first (their keys start with the time).
pub fn offered(host: &mut dyn Host) -> Vec<(String, Upload)> {
    let mut copies: Vec<(String, Upload)> = host
        .stored()
        .into_iter()
        .filter(|(key, _)| key.starts_with(PREFIX))
        .filter_map(|(key, value)| decode(&value).map(|upload| (key, upload)))
        .collect();
    copies.sort_by(|a, b| b.0.cmp(&a.0));
    copies
}

/// The copy of the open work.
#[derive(Debug)]
pub struct BrowserCopy {
    /// Its key, once written (or the key of a recovered copy that was opened).
    key: Option<String>,
    /// The copy is a recovered one that was opened: it stays until the work is saved.
    adopted: bool,
    /// The state (log length) it holds.
    written: Option<usize>,
    /// When it was last written or tried (seconds).
    last_try: f64,
    /// Wait this long after the last try.
    pause: f64,
    /// The outcome of the last write, for the status bar.
    last: Option<Written>,
}

impl Default for BrowserCopy {
    fn default() -> Self {
        Self {
            key: None,
            adopted: false,
            written: None,
            last_try: f64::NEG_INFINITY,
            pause: PAUSE,
            last: None,
        }
    }
}

impl BrowserCopy {
    /// What the last write did, for the status bar.
    pub fn last(&self) -> Option<&Written> {
        self.last.as_ref()
    }

    /// Its key in the browser's storage, if it has one.
    pub fn key(&self) -> Option<&str> {
        self.key.as_deref()
    }

    /// Called every frame: writes the copy when the work changed. Returns how long until it
    /// should be called again, if a write waits.
    pub fn tick(&mut self, host: &mut dyn Host, doc: Option<&OpenDoc>, time: f64) -> Option<f64> {
        let doc = doc?;
        if !doc.is_dirty() {
            // Back at the saved state (e.g. undone): an own copy is no longer needed.
            if !self.adopted
                && let Some(key) = self.key.take()
            {
                let _ = host.store(&key, None);
                self.written = None;
                self.last = None;
            }
            return None;
        }
        if self.written == Some(doc.log_len()) {
            return None;
        }
        if doc.encrypted() {
            self.last = Some(Written::Failed(text::t().no_copy_encrypted.to_owned()));
            return None;
        }
        let since = time - self.last_try;
        if since < self.pause {
            return Some(self.pause - since);
        }
        self.last_try = time;
        let key = self
            .key
            .get_or_insert_with(|| format!("{PREFIX}{}", doc.recovery_key()))
            .clone();
        let written = doc
            .to_bytes()
            .map_err(|e| text::store_error(&e))
            .and_then(|bytes| host.store(&key, Some(&encode(&doc.name(), &bytes))));
        match written {
            Ok(()) => {
                self.written = Some(doc.log_len());
                self.pause = PAUSE;
                self.last = Some(Written::At(
                    jiff::Zoned::now().strftime("%H:%M").to_string(),
                ));
                None
            }
            Err(reason) => {
                self.pause = RETRY;
                self.last = Some(Written::Failed(reason));
                Some(RETRY)
            }
        }
    }

    /// Takes over a recovered copy that was opened: later writes replace it, saving deletes
    /// it, dropping the work keeps it.
    pub fn adopt(&mut self, key: String) {
        *self = Self {
            key: Some(key),
            adopted: true,
            ..Self::default()
        };
    }

    /// The work was saved (downloaded): the copy goes, a recovered one too.
    pub fn saved(&mut self, host: &mut dyn Host) {
        if let Some(key) = self.key.take() {
            let _ = host.store(&key, None);
        }
        *self = Self::default();
    }

    /// The work was dropped on purpose: the copy goes – unless it is a recovered one, which
    /// stays until its content is saved.
    pub fn discard(&mut self, host: &mut dyn Host) {
        if !self.adopted
            && let Some(key) = self.key.take()
        {
            let _ = host.store(&key, None);
        }
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_copy_reads_back_and_rubbish_is_no_copy() {
        let copy = decode(&encode("lage", b"\x00\x01PK")).unwrap();
        assert_eq!(copy.name, "lage");
        assert_eq!(copy.bytes, b"\x00\x01PK");
        assert!(decode("").is_none());
        assert!(decode("{}").is_none());
        assert!(decode(r#"{"name":"x","data":"%%%"}"#).is_none());
        assert!(decode(r#"{"name":1,"data":""}"#).is_none());
    }
}
