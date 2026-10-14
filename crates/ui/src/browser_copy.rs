//! Browser: a recovery copy of the open work in the browser's own storage, so a reload, a
//! closed tab or a crash loses nothing that was taken. The app decides when to write; the
//! browser only keeps keys and values ([`Host::store`]). The desktop uses
//! [`crate::autosave`] with files instead.
//!
//! - Every change is written right away (changes are whole commands, not keystrokes).
//! - A download cannot tell whether the browser really saved the file (the person may cancel
//!   its dialog). So the copy stays after a download, marked as downloaded, and goes after
//!   [`KEEP_DOWNLOADED_DAYS`] days.
//! - Each copy knows its tab (kept in the tab's own session storage, which survives a reload).
//!   An open tab holds a lock of the browser on its id ([`Host::live_tabs`]); copies of open
//!   tabs are not offered to other tabs, so two tabs never write over each other's work. The
//!   browser releases the lock itself when a tab ends, also by a crash or while in the
//!   background, so its copies are offered right away.
//! - Encrypted work is not copied: encrypting takes seconds in the browser, and its plain
//!   content does not belong in the browser's storage. VS-NfD never gets here (decision 4.1).

use base64::Engine as _;
use jiff::Timestamp;
use serde_json::{Value, json};

use crate::autosave::Written;
use crate::doc::OpenDoc;
use crate::host::{Host, Upload};
use crate::text;

/// Every key of a copy starts with this.
pub const PREFIX: &str = "plnwrk.copy.";
/// Keys of an older version (signs of life of tabs); removed when found.
const OLD_LIVE: &str = "plnwrk.live.";
/// Downloaded copies are kept this long, in case the download did not really happen.
pub const KEEP_DOWNLOADED_DAYS: i64 = 7;
/// Seconds to wait after a write failed (e.g. the storage is full).
const RETRY: f64 = 10.0;

/// A copy in the browser's storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredCopy {
    /// Its key.
    pub key: String,
    /// Name and bytes of the file.
    pub file: Upload,
    /// When it was written.
    pub time: Option<Timestamp>,
    /// The work was downloaded after this copy (probably saved).
    pub downloaded: bool,
    /// The tab that wrote it.
    pub tab: String,
}

/// A copy as stored.
pub fn encode(name: &str, bytes: &[u8], tab: &str, downloaded: bool) -> String {
    json!({
        "name": name,
        "data": base64::engine::general_purpose::STANDARD.encode(bytes),
        "time": Timestamp::now().to_string(),
        "tab": tab,
        "downloaded": downloaded,
    })
    .to_string()
}

/// A stored copy; `None` if it is not one (damaged, or written by something else).
pub fn decode(key: &str, value: &str) -> Option<StoredCopy> {
    let value: Value = serde_json::from_str(value).ok()?;
    let name = value.get("name")?.as_str()?.to_owned();
    let data = value.get("data")?.as_str()?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .ok()?;
    let text = |field: &str| value.get(field).and_then(Value::as_str);
    Some(StoredCopy {
        key: key.to_owned(),
        file: Upload { name, bytes },
        time: text("time").and_then(|t| t.parse().ok()),
        downloaded: value.get("downloaded").and_then(Value::as_bool) == Some(true),
        tab: text("tab").unwrap_or_default().to_owned(),
    })
}

/// The copies to offer to this tab, newest first: its own, and those of tabs that are not open
/// (`live`: the open tabs; `None` while the browser has not told yet – then only the own ones).
/// Copies downloaded long ago are removed. `now` is the current time.
pub fn offered(
    host: &mut dyn Host,
    tab: &str,
    live: Option<&[String]>,
    now: Timestamp,
) -> Vec<StoredCopy> {
    let stored = host.stored();
    let open_elsewhere =
        |other: &str| other != tab && live.is_none_or(|open| open.iter().any(|t| t == other));
    let mut copies = Vec::new();
    for (key, value) in &stored {
        if key.starts_with(OLD_LIVE) {
            let _ = host.store(key, None);
            continue;
        }
        if !key.starts_with(PREFIX) {
            continue;
        }
        let Some(copy) = decode(key, value) else {
            continue;
        };
        let expired = copy.downloaded
            && copy.time.is_some_and(|time| {
                now.as_second() - time.as_second() > KEEP_DOWNLOADED_DAYS * 86_400
            });
        if expired {
            let _ = host.store(key, None);
        } else if !open_elsewhere(&copy.tab) {
            copies.push(copy);
        }
    }
    copies.sort_by(|a, b| b.time.cmp(&a.time).then(b.key.cmp(&a.key)));
    copies
}

/// The copy of the open work.
#[derive(Debug)]
pub struct BrowserCopy {
    /// Its key, once written (or the key of a recovered copy that was opened).
    key: Option<String>,
    /// The copy is a recovered one that was opened: it stays until the work is saved.
    adopted: bool,
    /// The work was downloaded and not changed since: the copy stays (the download may not
    /// have happened).
    downloaded: bool,
    /// The state (log length) it holds.
    written: Option<usize>,
    /// When a write may be tried again after a failure (seconds).
    retry_at: f64,
    /// The outcome of the last write, for the status bar.
    last: Option<Written>,
}

impl Default for BrowserCopy {
    fn default() -> Self {
        Self {
            key: None,
            adopted: false,
            downloaded: false,
            written: None,
            retry_at: f64::NEG_INFINITY,
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
    pub fn tick(
        &mut self,
        host: &mut dyn Host,
        tab: &str,
        doc: Option<&OpenDoc>,
        time: f64,
    ) -> Option<f64> {
        let wait = None;
        let Some(doc) = doc else {
            return wait;
        };
        if !doc.is_dirty() {
            // Back at a saved state (e.g. undone): an own copy is no longer needed – unless it
            // is a recovered one or the work was just downloaded.
            if !self.adopted
                && !self.downloaded
                && let Some(key) = self.key.take()
            {
                let _ = host.store(&key, None);
                self.written = None;
                self.last = None;
            }
            return wait;
        }
        if self.written == Some(doc.log_len()) {
            return wait;
        }
        if doc.encrypted() {
            self.last = Some(Written::Failed(text::t().no_copy_encrypted.to_owned()));
            return wait;
        }
        if time < self.retry_at {
            return Some(self.retry_at - time);
        }
        match self.write(host, tab, doc, false) {
            Ok(()) => {
                self.downloaded = false;
                self.last = Some(Written::At(
                    jiff::Zoned::now().strftime("%H:%M").to_string(),
                ));
                wait
            }
            Err(reason) => {
                self.retry_at = time + RETRY;
                self.last = Some(Written::Failed(reason));
                Some(RETRY)
            }
        }
    }

    fn write(
        &mut self,
        host: &mut dyn Host,
        tab: &str,
        doc: &OpenDoc,
        downloaded: bool,
    ) -> Result<(), String> {
        let key = self
            .key
            .get_or_insert_with(|| format!("{PREFIX}{}", doc.recovery_key()))
            .clone();
        let bytes = doc.to_bytes().map_err(|e| text::store_error(&e))?;
        host.store(&key, Some(&encode(&doc.name(), &bytes, tab, downloaded)))?;
        self.written = Some(doc.log_len());
        Ok(())
    }

    /// Takes over a recovered copy that was opened: later writes replace it, dropping the work
    /// keeps it.
    pub fn adopt(&mut self, key: String) {
        *self = Self {
            key: Some(key),
            adopted: true,
            ..Self::default()
        };
    }

    /// The work was downloaded. The browser cannot tell whether the file really arrived, so
    /// the copy stays, marked as downloaded (and goes after some days).
    pub fn downloaded(&mut self, host: &mut dyn Host, tab: &str, doc: &OpenDoc) {
        if doc.encrypted() {
            // Never a plain copy of protected work; an earlier one (before protecting) goes.
            if let Some(key) = self.key.take() {
                let _ = host.store(&key, None);
            }
            *self = Self::default();
            return;
        }
        match self.write(host, tab, doc, true) {
            Ok(()) => {
                self.downloaded = true;
                self.adopted = false;
            }
            Err(reason) => {
                // The older copy stays, also once the work counts as saved: the download may
                // not have happened.
                self.downloaded = true;
                self.last = Some(Written::Failed(reason));
            }
        }
    }

    /// The work was dropped on purpose: the copy goes – unless it is a recovered one or the
    /// work was downloaded, which stay (see [`Self::downloaded`]).
    pub fn discard(&mut self, host: &mut dyn Host) {
        if !self.adopted
            && !self.downloaded
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
        let copy = decode("k", &encode("lage", b"\x00\x01PK", "tab-1", true)).unwrap();
        assert_eq!(copy.file.name, "lage");
        assert_eq!(copy.file.bytes, b"\x00\x01PK");
        assert_eq!(copy.tab, "tab-1");
        assert!(copy.downloaded);
        assert!(copy.time.is_some());
        assert!(decode("k", "").is_none());
        assert!(decode("k", "{}").is_none());
        assert!(decode("k", r#"{"name":"x","data":"%%%"}"#).is_none());
        assert!(decode("k", r#"{"name":1,"data":""}"#).is_none());
        let old = decode("k", r#"{"name":"x","data":""}"#).unwrap();
        assert!(!old.downloaded && old.time.is_none() && old.tab.is_empty());
    }
}
