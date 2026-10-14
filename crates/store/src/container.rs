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

fn u16_at(bytes: &[u8], at: usize) -> Option<usize> {
    let pair = bytes.get(at..at.checked_add(2)?)?;
    Some(usize::from(u16::from_le_bytes([
        *pair.first()?,
        *pair.get(1)?,
    ])))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<usize> {
    let quad = bytes.get(at..at.checked_add(4)?)?;
    let value = u32::from_le_bytes([*quad.first()?, *quad.get(1)?, *quad.get(2)?, *quad.get(3)?]);
    usize::try_from(value).ok()
}

const LOCAL_HEADER: [u8; 4] = [0x50, 0x4b, 0x03, 0x04];
const CENTRAL_HEADER: [u8; 4] = [0x50, 0x4b, 0x01, 0x02];
const END_RECORD: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];

/// Checks that the bytes are laid out exactly as plnwrk writes ZIP files: entries back to back
/// from the first byte, followed by the central directory and the end record with no comment –
/// nothing before, between or after. Duplicate names, ZIP64, data descriptors and entry comments
/// are refused. ZIP readers are lenient about all of this, so different tools could otherwise
/// see different content in the same file. Also bounds the number of entries before anything
/// is allocated. Returns the entry names in order.
fn check_layout(bytes: &[u8], max_entries: usize) -> Result<Vec<String>, StoreError> {
    if !bytes.starts_with(&LOCAL_HEADER) {
        return Err(StoreError::NotPlnwrk);
    }
    let layout = || StoreError::corrupt("the ZIP is not laid out as plnwrk writes it");
    let end = bytes.len().checked_sub(22).ok_or_else(layout)?;
    let record = bytes.get(end..).ok_or_else(layout)?;
    if !record.starts_with(&END_RECORD) {
        return Err(layout());
    }
    let field = |at| u16_at(record, at).ok_or_else(layout);
    let (disk, directory_disk, on_disk, total, comment) =
        (field(4)?, field(6)?, field(8)?, field(10)?, field(20)?);
    let directory_size = u32_at(record, 12).ok_or_else(layout)?;
    let directory_start = u32_at(record, 16).ok_or_else(layout)?;
    if disk != 0 || directory_disk != 0 || on_disk != total || comment != 0 {
        return Err(layout());
    }
    if total > max_entries {
        return Err(StoreError::TooLarge {
            limit: u64::try_from(max_entries).unwrap_or(u64::MAX),
        });
    }
    if directory_start.checked_add(directory_size) != Some(end) {
        return Err(layout());
    }
    // Central directory: exactly `total` headers filling it completely.
    let mut entries: Vec<(usize, usize, Vec<u8>)> = Vec::with_capacity(total);
    let mut at = directory_start;
    for _ in 0..total {
        let header = bytes.get(at..).ok_or_else(layout)?;
        if !header.starts_with(&CENTRAL_HEADER) {
            return Err(layout());
        }
        let get16 = |offset| u16_at(header, offset).ok_or_else(layout);
        let get32 = |offset| u32_at(header, offset).ok_or_else(layout);
        let (flags, name_len, extra_len, comment_len, start_disk) =
            (get16(8)?, get16(28)?, get16(30)?, get16(32)?, get16(34)?);
        let (compressed, uncompressed, offset) = (get32(20)?, get32(24)?, get32(42)?);
        let zip64 = [compressed, uncompressed, offset].contains(&0xFFFF_FFFF);
        if flags & 0x0008 != 0 || comment_len != 0 || start_disk != 0 || zip64 {
            return Err(layout());
        }
        let name = header.get(46..46 + name_len).ok_or_else(layout)?.to_vec();
        entries.push((offset, compressed, name));
        at = at
            .checked_add(46 + name_len + extra_len)
            .ok_or_else(layout)?;
    }
    if at != end {
        return Err(layout());
    }
    // Local entries: back to back from offset 0 up to the central directory.
    let mut by_offset: Vec<&(usize, usize, Vec<u8>)> = entries.iter().collect();
    by_offset.sort_by_key(|(offset, _, _)| *offset);
    let mut expected = 0usize;
    for (offset, compressed, name) in by_offset {
        if *offset != expected {
            return Err(layout());
        }
        let local = bytes.get(*offset..).ok_or_else(layout)?;
        if !local.starts_with(&LOCAL_HEADER) {
            return Err(layout());
        }
        let name_len = u16_at(local, 26).ok_or_else(layout)?;
        let extra_len = u16_at(local, 28).ok_or_else(layout)?;
        if local.get(30..30 + name_len) != Some(name.as_slice()) {
            return Err(layout());
        }
        expected = offset
            .checked_add(30 + name_len + extra_len)
            .and_then(|start| start.checked_add(*compressed))
            .ok_or_else(layout)?;
    }
    if expected != directory_start {
        return Err(layout());
    }
    let mut names = Vec::with_capacity(entries.len());
    for (_, _, name) in entries {
        let name = String::from_utf8(name).map_err(|_| layout())?;
        if names.contains(&name) {
            return Err(StoreError::corrupt(format!(
                "zip entry {name} appears twice"
            )));
        }
        names.push(name);
    }
    Ok(names)
}

/// All entries of a ZIP by name, within `limits`. Two entries with the same name are refused:
/// other ZIP tools might show the first, plnwrk would read the last.
pub fn read_zip(bytes: &[u8], limits: Limits) -> Result<BTreeMap<String, Vec<u8>>, StoreError> {
    let names = check_layout(bytes, limits.max_entries)?;
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(|_| StoreError::NotPlnwrk)?;
    if archive.len() != names.len() {
        return Err(StoreError::corrupt(
            "the ZIP is not laid out as plnwrk writes it",
        ));
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
    check_layout(bytes, Limits::default().max_entries)?;
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
