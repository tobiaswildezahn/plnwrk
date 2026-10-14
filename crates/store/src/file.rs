//! Files on disk (desktop only). Writing is atomic: the new content goes to a temporary file in
//! the same folder, is flushed to disk and then renamed over the old file, so a crash or full
//! disk never leaves a half-written file behind.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use crate::StoreError;

fn io(error: std::io::Error) -> StoreError {
    StoreError::Io {
        reason: error.to_string(),
    }
}

/// Reads a whole file.
pub fn read(path: &Path) -> Result<Vec<u8>, StoreError> {
    fs::read(path).map_err(io)
}

/// Replaces `path` with `bytes` atomically.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
    let folder = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| StoreError::Io {
            reason: format!("not a file path: {}", path.display()),
        })?;
    let temporary = folder.join(format!(".{name}.{}.tmp", std::process::id()));
    let result = (|| {
        let mut file = fs::File::create(&temporary).map_err(io)?;
        file.write_all(bytes).map_err(io)?;
        file.sync_all().map_err(io)?;
        drop(file);
        fs::rename(&temporary, path).map_err(io)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

/// Where automatic saves of `path` go: `.<name>.autosave.plnwrk` next to it.
pub fn autosave_path(path: &Path) -> PathBuf {
    let name = path
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("workspace");
    path.with_file_name(format!(".{name}.autosave.plnwrk"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("plnwrk-file-test-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn writes_and_replaces_atomically() {
        let dir = scratch("atomic");
        let path = dir.join("a.plnwrk");
        write_atomic(&path, b"eins").unwrap();
        write_atomic(&path, b"zwei").unwrap();
        assert_eq!(read(&path).unwrap(), b"zwei");
        let leftovers: Vec<_> = fs::read_dir(&dir).unwrap().filter_map(Result::ok).collect();
        assert_eq!(leftovers.len(), 1, "no temporary files remain");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_failed_write_keeps_the_old_file() {
        let dir = scratch("fail");
        let path = dir.join("a.plnwrk");
        write_atomic(&path, b"alt").unwrap();
        // A directory in place of the target makes the rename fail.
        let blocked = dir.join("b.plnwrk");
        fs::create_dir_all(blocked.join("x")).unwrap();
        assert!(write_atomic(&blocked, b"neu").is_err());
        assert_eq!(read(&path).unwrap(), b"alt");
        let temporaries = fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .count();
        assert_eq!(temporaries, 0);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn autosave_sits_next_to_the_file() {
        let path = Path::new("/tmp/Projekte.plnwrk");
        assert_eq!(
            autosave_path(path),
            Path::new("/tmp/.Projekte.autosave.plnwrk")
        );
    }
}
