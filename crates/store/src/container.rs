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

/// How an entry is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Packing {
    /// Deflate-compressed (text).
    Compressed,
    /// As is (already encrypted, compression would not help).
    Stored,
}

/// A ZIP with the given entries, without timestamps or comment (reproducible within one build).
pub fn write_zip(entries: &[(&str, &[u8], Packing)]) -> Result<Vec<u8>, StoreError> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes, packing) in entries {
        let method = match packing {
            Packing::Compressed => CompressionMethod::Deflated,
            Packing::Stored => CompressionMethod::Stored,
        };
        let options = SimpleFileOptions::default().compression_method(method);
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

/// The number of entries the end-of-central-directory record announces. plnwrk files end with
/// that record (no comment, no ZIP64); anything else is refused.
fn announced_entries(bytes: &[u8]) -> Result<usize, StoreError> {
    let layout = || StoreError::corrupt("unexpected ZIP layout");
    let start = bytes.len().checked_sub(22).ok_or_else(layout)?;
    let record = bytes.get(start..).ok_or_else(layout)?;
    let field = |at: usize| -> Option<u16> {
        let pair = record.get(at..at + 2)?;
        Some(u16::from_le_bytes([*pair.first()?, *pair.get(1)?]))
    };
    let signature = record.get(..4).ok_or_else(layout)?;
    if signature != [0x50, 0x4b, 0x05, 0x06] || field(20) != Some(0) {
        return Err(layout());
    }
    field(10).map(usize::from).ok_or_else(layout)
}

/// All entries of a ZIP by name, within `limits`. Two entries with the same name are refused:
/// other ZIP tools might show the first, plnwrk would read the last.
pub fn read_zip(bytes: &[u8], limits: Limits) -> Result<BTreeMap<String, Vec<u8>>, StoreError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(|_| StoreError::NotPlnwrk)?;
    if announced_entries(bytes)? != archive.len() {
        return Err(StoreError::corrupt("the ZIP lists entries twice"));
    }
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

/// One entry by name, at most `limit` bytes (for reading the manifest alone).
pub fn read_entry(bytes: &[u8], name: &str, limit: u64) -> Result<Vec<u8>, StoreError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(|_| StoreError::NotPlnwrk)?;
    let mut file = archive.by_name(name).map_err(|_| StoreError::NotPlnwrk)?;
    let mut content = Vec::new();
    (&mut file)
        .take(limit.saturating_add(1))
        .read_to_end(&mut content)
        .map_err(|e| StoreError::corrupt(format!("zip entry {name}: {e}")))?;
    if u64::try_from(content.len()).unwrap_or(u64::MAX) > limit {
        return Err(StoreError::TooLarge { limit });
    }
    Ok(content)
}
