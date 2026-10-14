//! Files on disk (desktop only).
//!
//! Writing is atomic: the new content goes to a temporary file in the same folder – created
//! exclusively under an unpredictable name, so a planted link cannot redirect it – is flushed to
//! disk and then renamed over the old file; the folder is flushed too. A crash or a full disk
//! never leaves a half-written file behind. New files are readable by their owner only.

use std::fs::{self, File, OpenOptions};
use std::hash::{BuildHasher, Hasher};
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};

use crate::StoreError;

/// Largest file read from disk.
pub const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;

fn io(error: std::io::Error) -> StoreError {
    StoreError::Io {
        reason: error.to_string(),
    }
}

/// Reads a whole file, at most [`MAX_FILE_BYTES`].
pub fn read(path: &Path) -> Result<Vec<u8>, StoreError> {
    let file = File::open(path).map_err(io)?;
    if file.metadata().map_err(io)?.len() > MAX_FILE_BYTES {
        return Err(StoreError::TooLarge {
            limit: MAX_FILE_BYTES,
        });
    }
    let mut content = Vec::new();
    file.take(MAX_FILE_BYTES.saturating_add(1))
        .read_to_end(&mut content)
        .map_err(io)?;
    if u64::try_from(content.len()).unwrap_or(u64::MAX) > MAX_FILE_BYTES {
        return Err(StoreError::TooLarge {
            limit: MAX_FILE_BYTES,
        });
    }
    Ok(content)
}

fn folder_of(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn name_of(path: &Path) -> Result<&str, StoreError> {
    path.file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| StoreError::Io {
            reason: format!("not a file path: {}", path.display()),
        })
}

/// 64 random bits from the operating system (via the standard library's hash seed).
fn random() -> u64 {
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u128(std::process::id().into());
    hasher.finish()
}

/// Creates a new temporary file next to `path`, exclusively, readable by the owner only.
fn temporary(path: &Path) -> Result<(PathBuf, File), StoreError> {
    let folder = folder_of(path);
    let name = name_of(path)?;
    for _ in 0..8 {
        let candidate = folder.join(format!(".{name}.{:016x}.tmp", random()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        match options.open(&candidate) {
            Ok(file) => return Ok((candidate, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(io(e)),
        }
    }
    Err(StoreError::Io {
        reason: "cannot create a temporary file".to_owned(),
    })
}

/// Flushes the folder entry of a rename to disk (where the system supports it).
fn sync_folder(path: &Path) {
    #[cfg(unix)]
    {
        if let Ok(folder) = File::open(folder_of(path)) {
            let _ = folder.sync_all();
        }
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

fn write_temporary(path: &Path, bytes: &[u8]) -> Result<PathBuf, StoreError> {
    let (temp, mut file) = temporary(path)?;
    let written = file
        .write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(io);
    if let Err(error) = written {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    Ok(temp)
}

/// Gives `temp` the permissions of an existing `path` (Unix modes only: on Windows copying a
/// read-only flag would make the rename fail).
fn keep_permissions(path: &Path, temp: &Path) -> Result<(), StoreError> {
    #[cfg(unix)]
    if let Ok(meta) = fs::metadata(path) {
        fs::set_permissions(temp, meta.permissions()).map_err(io)?;
    }
    #[cfg(not(unix))]
    let _ = (path, temp);
    Ok(())
}

/// Replaces `path` with `bytes` atomically. An existing file keeps its permissions.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
    let temp = write_temporary(path, bytes)?;
    let result = keep_permissions(path, &temp).and_then(|()| fs::rename(&temp, path).map_err(io));
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    } else {
        sync_folder(path);
    }
    result
}

/// Writes a new file; never replaces an existing one, even if it appears in the meantime.
pub fn write_new(path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
    let temp = write_temporary(path, bytes)?;
    // A hard link fails if the target exists – an atomic "create, never replace".
    let linked = fs::hard_link(&temp, path);
    let result = match linked {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err(StoreError::Io {
            reason: format!("{} already exists", path.display()),
        }),
        // File systems without hard links (e.g. some USB sticks): create the target exclusively
        // and remove it again if writing fails, so no half file blocks the name.
        Err(_) => {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt as _;
                options.mode(0o600);
            }
            match options.open(path) {
                Ok(mut file) => {
                    let written = file.write_all(bytes).and_then(|()| file.sync_all());
                    if written.is_err() {
                        let _ = fs::remove_file(path);
                    }
                    written.map_err(io)
                }
                Err(e) => Err(io(e)),
            }
        }
    };
    let _ = fs::remove_file(&temp);
    if result.is_ok() {
        sync_folder(path);
    }
    result
}

/// The real path behind `path`: links and `..` resolved, so that locking and saving act on the
/// file itself – a link is followed, not replaced. For a file that does not exist yet, its folder
/// is resolved.
pub fn resolve(path: &Path) -> Result<PathBuf, StoreError> {
    if let Ok(real) = fs::canonicalize(path) {
        return Ok(real);
    }
    let folder = fs::canonicalize(folder_of(path)).map_err(io)?;
    Ok(folder.join(name_of(path)?))
}

/// Where automatic saves of `path` go: `.<name>.autosave.plnwrk` next to it.
pub fn autosave_path(path: &Path) -> PathBuf {
    let name = path
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("workspace");
    path.with_file_name(format!(".{name}.autosave.plnwrk"))
}

/// Holds the lock of a workspace file while it is alive.
#[derive(Debug)]
pub struct Lock {
    _file: File,
}

/// Locks `path` against other plnwrk processes (via `.<name>.lock` next to it, so that saving
/// can still replace the file itself). Fails at once if another process holds it.
pub fn lock(path: &Path) -> Result<Lock, StoreError> {
    let lock_path = folder_of(path).join(format!(".{}.lock", name_of(path)?));
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(io)?;
    match file.try_lock() {
        Ok(()) => Ok(Lock { _file: file }),
        Err(fs::TryLockError::WouldBlock) => Err(StoreError::Io {
            reason: format!("{} is in use by another plnwrk", path.display()),
        }),
        Err(fs::TryLockError::Error(e)) => Err(io(e)),
    }
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

    fn entries(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn writes_and_replaces_atomically() {
        let dir = scratch("atomic");
        let path = dir.join("a.plnwrk");
        write_atomic(&path, b"eins").unwrap();
        write_atomic(&path, b"zwei").unwrap();
        assert_eq!(read(&path).unwrap(), b"zwei");
        assert_eq!(entries(&dir), ["a.plnwrk"], "no temporary files remain");
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
        assert_eq!(entries(&dir), ["a.plnwrk", "b.plnwrk"]);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn new_files_never_replace_existing_ones() {
        let dir = scratch("new");
        let path = dir.join("a.plnwrk");
        write_new(&path, b"erst").unwrap();
        assert!(write_new(&path, b"zweit").is_err());
        assert_eq!(read(&path).unwrap(), b"erst");
        assert_eq!(entries(&dir), ["a.plnwrk"]);
        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn permissions_are_private_for_new_files_and_kept_for_existing_ones() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = scratch("mode");
        let path = dir.join("a.plnwrk");
        write_new(&path, b"x").unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        write_atomic(&path, b"y").unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_planted_link_is_not_followed() {
        // Even if someone guesses a temporary name and plants a link there, create_new refuses
        // to open it, so the link target is never written.
        let dir = scratch("link");
        let victim = dir.join("victim.txt");
        fs::write(&victim, b"unberuehrt").unwrap();
        let planted = dir.join(".a.plnwrk.planted.tmp");
        std::os::unix::fs::symlink(&victim, &planted).unwrap();
        let refused = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&planted);
        assert!(refused.is_err());
        write_atomic(&dir.join("a.plnwrk"), b"daten").unwrap();
        assert_eq!(fs::read(&victim).unwrap(), b"unberuehrt");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_second_lock_is_refused_until_the_first_is_released() {
        let dir = scratch("lock");
        let path = dir.join("a.plnwrk");
        let first = lock(&path).unwrap();
        assert!(matches!(lock(&path), Err(StoreError::Io { .. })));
        drop(first);
        assert!(lock(&path).is_ok());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn oversized_files_are_refused() {
        // Only the limit logic: a sparse file larger than the limit.
        let dir = scratch("big");
        let path = dir.join("big.plnwrk");
        let file = File::create(&path).unwrap();
        file.set_len(MAX_FILE_BYTES + 1).unwrap();
        assert!(matches!(read(&path), Err(StoreError::TooLarge { .. })));
        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn links_are_resolved_to_the_real_file() {
        let dir = scratch("resolve");
        let real = dir.join("real.plnwrk");
        write_new(&real, b"daten").unwrap();
        let link = dir.join("link.plnwrk");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let resolved = resolve(&link).unwrap();
        assert_eq!(resolved, fs::canonicalize(&real).unwrap());
        write_atomic(&resolved, b"neu").unwrap();
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(read(&link).unwrap(), b"neu");
        let fresh = resolve(&dir.join("neu.plnwrk")).unwrap();
        assert_eq!(fresh, fs::canonicalize(&dir).unwrap().join("neu.plnwrk"));
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
