//! ZIP reading and writing with limits: a file is hostile, so entry count and unpacked size are
//! bounded before anything is kept in memory (zip bombs).

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::StoreError;

/// How much a file may unpack to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Unpacked bytes of all entries together.
    pub max_bytes: u64,
    /// Number of entries.
    pub max_entries: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_bytes: 256 * 1024 * 1024,
            max_entries: 64,
        }
    }
}

/// A ZIP with the given entries, deflate-compressed, without timestamps (reproducible).
pub fn write_zip(entries: &[(&str, &[u8])]) -> Result<Vec<u8>, StoreError> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, bytes) in entries {
        writer
            .start_file(*name, options)
            .map_err(|e| StoreError::corrupt(format!("zip: {e}")))?;
        writer
            .write_all(bytes)
            .map_err(|e| StoreError::corrupt(format!("zip: {e}")))?;
    }
    let cursor = writer
        .finish()
        .map_err(|e| StoreError::corrupt(format!("zip: {e}")))?;
    Ok(cursor.into_inner())
}

/// All entries of a ZIP by name, within `limits`.
pub fn read_zip(bytes: &[u8], limits: Limits) -> Result<BTreeMap<String, Vec<u8>>, StoreError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(|_| StoreError::NotPlnwrk)?;
    if archive.len() > limits.max_entries {
        return Err(StoreError::TooLarge {
            limit: u64::try_from(limits.max_entries).unwrap_or(u64::MAX),
        });
    }
    let mut entries = BTreeMap::new();
    let mut total: u64 = 0;
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|e| StoreError::corrupt(format!("zip entry {index}: {e}")))?;
        let name = file
            .name()
            .map_err(|e| StoreError::corrupt(format!("zip entry {index}: {e}")))?
            .into_owned();
        let remaining = limits.max_bytes.saturating_sub(total);
        let mut content = Vec::new();
        (&mut file)
            .take(remaining.saturating_add(1))
            .read_to_end(&mut content)
            .map_err(|e| StoreError::corrupt(format!("zip entry {name}: {e}")))?;
        total = total.saturating_add(u64::try_from(content.len()).unwrap_or(u64::MAX));
        if total > limits.max_bytes {
            return Err(StoreError::TooLarge {
                limit: limits.max_bytes,
            });
        }
        if entries.insert(name.clone(), content).is_some() {
            return Err(StoreError::corrupt(format!(
                "zip entry {name} appears twice"
            )));
        }
    }
    Ok(entries)
}
