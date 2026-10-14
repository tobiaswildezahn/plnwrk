//! What the surrounding app provides: file dialogs. The desktop app shows the system dialogs;
//! tests and the browser use [`NoDialogs`] or their own stand-in.

use std::path::PathBuf;

/// A file chosen in the browser: its name and content.
#[derive(Clone, PartialEq, Eq)]
pub struct Upload {
    /// The file name as the browser gives it (`lage.plnwrk`).
    pub name: String,
    /// The content.
    pub bytes: Vec<u8>,
}

impl std::fmt::Debug for Upload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Upload")
            .field("name", &self.name)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

/// File dialogs of the platform. On the desktop files have paths; in the browser they come
/// as uploads and go as downloads (`browser()`), and nothing else touches the disk.
pub trait Host {
    /// Asks for a `.plnwrk` file to open; `None` if cancelled.
    fn pick_open(&mut self) -> Option<PathBuf>;

    /// Asks where to save, suggesting a file name; `None` if cancelled.
    fn pick_save(&mut self, suggested_name: &str) -> Option<PathBuf>;

    /// Whether files are uploaded and downloaded (the browser) instead of having paths.
    fn browser(&self) -> bool {
        false
    }

    /// Browser: asks the person to choose a file; it arrives later through
    /// [`Host::uploaded`] (the app asks every frame).
    fn request_upload(&mut self) {}

    /// Browser: the file chosen since the last call, if any (`Err`: it could not be read).
    fn uploaded(&mut self) -> Option<Result<Upload, String>> {
        None
    }

    /// Browser: offers the bytes as a download under this name. `Err` says why it failed.
    fn download(&mut self, _name: &str, _bytes: &[u8]) -> Result<(), String> {
        Err(String::new())
    }

    /// Browser: whether leaving the page should ask first (there is unsaved work).
    fn set_leave_warning(&mut self, _on: bool) {}

    /// Browser: keeps a value under a key in the browser's own storage (`None` removes it).
    /// `Err` says why it failed (e.g. the storage is full).
    fn store(&mut self, _key: &str, _value: Option<&str>) -> Result<(), String> {
        Err(String::new())
    }

    /// Browser: every key and value kept with [`Host::store`].
    fn stored(&mut self) -> Vec<(String, String)> {
        Vec::new()
    }

    /// Browser: an id of this tab that stays the same when the page is loaded again.
    fn tab(&mut self) -> String {
        String::new()
    }

    /// Browser: whether the tab is in the background (the browser slows it down).
    fn hidden(&self) -> bool {
        false
    }
}

/// No dialogs: every request is cancelled.
#[derive(Debug, Default)]
pub struct NoDialogs;

impl Host for NoDialogs {
    fn pick_open(&mut self) -> Option<PathBuf> {
        None
    }

    fn pick_save(&mut self, _suggested_name: &str) -> Option<PathBuf> {
        None
    }
}

/// Answers given in advance, for tests: each request takes the next one.
#[cfg(test)]
#[derive(Debug, Default)]
pub struct Scripted {
    pub open: Vec<PathBuf>,
    pub save: Vec<PathBuf>,
}

#[cfg(test)]
impl Host for Scripted {
    fn pick_open(&mut self) -> Option<PathBuf> {
        (!self.open.is_empty()).then(|| self.open.remove(0))
    }

    fn pick_save(&mut self, _suggested_name: &str) -> Option<PathBuf> {
        (!self.save.is_empty()).then(|| self.save.remove(0))
    }
}

/// What a test browser saw and what it answers.
#[cfg(test)]
#[derive(Debug, Default)]
pub struct BrowserLog {
    /// How often the person was asked to choose a file.
    pub upload_requests: usize,
    /// The next file "chosen" (taken by the app on its next frame), or why it was not read.
    pub upload: Option<Result<Upload, String>>,
    /// The id of this tab.
    pub tab: String,
    /// The tab is in the background.
    pub hidden: bool,
    /// Every download, oldest first.
    pub downloads: Vec<Upload>,
    /// Downloads fail with this reason.
    pub download_fails: Option<String>,
    /// Whether leaving the page would ask now.
    pub leave_warning: bool,
    /// The browser's storage.
    pub storage: std::collections::BTreeMap<String, String>,
    /// Writing to the storage fails with this reason.
    pub store_fails: Option<String>,
}

/// A browser for tests: shares its log with the test, which keeps a handle.
#[cfg(test)]
#[derive(Debug, Default, Clone)]
pub struct TestBrowser(pub std::rc::Rc<std::cell::RefCell<BrowserLog>>);

#[cfg(test)]
impl Host for TestBrowser {
    fn pick_open(&mut self) -> Option<PathBuf> {
        None
    }

    fn pick_save(&mut self, _suggested_name: &str) -> Option<PathBuf> {
        None
    }

    fn browser(&self) -> bool {
        true
    }

    fn request_upload(&mut self) {
        self.0.borrow_mut().upload_requests += 1;
    }

    fn uploaded(&mut self) -> Option<Result<Upload, String>> {
        self.0.borrow_mut().upload.take()
    }

    fn tab(&mut self) -> String {
        self.0.borrow().tab.clone()
    }

    fn hidden(&self) -> bool {
        self.0.borrow().hidden
    }

    fn download(&mut self, name: &str, bytes: &[u8]) -> Result<(), String> {
        let mut log = self.0.borrow_mut();
        if let Some(reason) = &log.download_fails {
            return Err(reason.clone());
        }
        log.downloads.push(Upload {
            name: name.to_owned(),
            bytes: bytes.to_vec(),
        });
        Ok(())
    }

    fn set_leave_warning(&mut self, on: bool) {
        self.0.borrow_mut().leave_warning = on;
    }

    fn store(&mut self, key: &str, value: Option<&str>) -> Result<(), String> {
        let mut log = self.0.borrow_mut();
        match value {
            Some(_) if log.store_fails.is_some() => {
                Err(log.store_fails.clone().unwrap_or_default())
            }
            Some(value) => {
                log.storage.insert(key.to_owned(), value.to_owned());
                Ok(())
            }
            None => {
                log.storage.remove(key);
                Ok(())
            }
        }
    }

    fn stored(&mut self) -> Vec<(String, String)> {
        let log = self.0.borrow();
        log.storage
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}
