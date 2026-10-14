//! What the surrounding app provides: file dialogs. The desktop app shows the system dialogs;
//! tests and the browser use [`NoDialogs`] or their own stand-in.

use std::path::PathBuf;

/// File dialogs of the platform.
pub trait Host {
    /// Asks for a `.plnwrk` file to open; `None` if cancelled.
    fn pick_open(&mut self) -> Option<PathBuf>;

    /// Asks where to save, suggesting a file name; `None` if cancelled.
    fn pick_save(&mut self, suggested_name: &str) -> Option<PathBuf>;
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
