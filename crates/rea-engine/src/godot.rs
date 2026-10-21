//! Godot PCK Archive Inspection
//!
//! **File:** `godot.rs`
//! **Author:** Kevin Thomas
//! **Date:** 2026
//!
//! MIT License
//!
//! Copyright (c) 2026 Kevin Thomas
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy
//! of this software and associated documentation files (the "Software"), to deal
//! in the Software without restriction, including without limitation the rights
//! to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//! copies of the Software, and to permit persons to whom the Software is
//! furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in
//! all copies or substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//! IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//! FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//! AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//! OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
//! SOFTWARE.

/// `Godot` PCK magic (`"GDPC"`).
const MAGIC: [u8; 4] = *b"GDPC";

/// Size of the trailing embedded-archive locator.
const TRAILER: usize = 12;

/// Maximum directory entries recorded.
const MAX_ENTRIES: usize = 100_000;

/// One file declared in a PCK directory.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct PckEntry {
    /// Resource path, such as `res://scene.tscn`.
    pub path: String,

    /// Absolute data offset in the scanned buffer.
    pub offset: u64,

    /// Declared size in bytes.
    pub size: u64,

    /// Whether the entry payload is encrypted.
    pub encrypted: bool,
}

/// A recovered `Godot` PCK archive.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct PckArchive {
    /// Format version.
    pub version: u32,

    /// `Godot` engine version label.
    pub engine: String,

    /// Offset of the archive inside the scanned buffer.
    pub embedded_offset: usize,

    /// File base added to every entry offset.
    pub base: u64,

    /// Directory offset for format version 3.
    pub index: usize,

    /// Whether the directory is encrypted.
    pub encrypted_dir: bool,

    /// Declared file count.
    pub file_count: u32,

    /// Recovered entries.
    pub files: Vec<PckEntry>,
}

/// Reports whether a buffer is a standalone or embedded `Godot` PCK.
///
/// # Arguments
///
/// * `bytes` - Raw target bytes.
///
/// # Returns
///
/// `true` when a valid PCK header is located.
#[must_use]
pub fn is_pck(bytes: &[u8]) -> bool {
    locate(bytes).is_some()
}

/// Locates the PCK header within a buffer.
///
/// # Arguments
///
/// * `bytes` - Raw target bytes.
///
/// # Returns
///
/// The archive start offset, or `None`.
fn locate(bytes: &[u8]) -> Option<usize> {
    if bytes.starts_with(&MAGIC) {
        return Some(0);
    }
    trailer_start(bytes)
}

/// Locates an archive appended with the trailing size-and-magic record.
///
/// # Arguments
///
/// * `bytes` - Raw target bytes.
///
/// # Returns
///
/// The archive start offset, or `None`.
fn trailer_start(bytes: &[u8]) -> Option<usize> {
    let len = bytes.len();
    if len < TRAILER || bytes[len - 4..] != MAGIC {
        return None;
    }
    let size = read_u64(bytes, len - 12)?;
    let start = usize::try_from(len as u64 - 12 - size).ok()?;
    (bytes.get(start..start + 4)? == MAGIC.as_slice()).then_some(start)
}

/// Parses a `Godot` PCK archive.
///
/// # Arguments
///
/// * `bytes` - Raw target bytes.
///
/// # Returns
///
/// The recovered archive, or `None`.
#[must_use]
pub fn parse_pck(bytes: &[u8]) -> Option<PckArchive> {
    let start = locate(bytes)?;
    let (cursor, mut archive, flags) = open_header(bytes, start)?;
    if flags & 1 != 0 {
        archive.encrypted_dir = true;
        return Some(archive);
    }
    fill_directory(bytes, &cursor, &mut archive, start)?;
    Some(archive)
}

/// Reads the archive header and option fields.
///
/// # Arguments
///
/// * `bytes` - Raw target bytes.
/// * `start` - Archive start offset.
///
/// # Returns
///
/// The cursor, archive, and pack flags.
fn open_header(bytes: &[u8], start: usize) -> Option<(Cursor<'_>, PckArchive, u32)> {
    let mut cursor = Cursor::new(bytes, start + 4);
    let version = cursor.u32()?;
    let (major, minor, patch) = (cursor.u32()?, cursor.u32()?, cursor.u32()?);
    let mut archive = header(version, major, minor, patch, start);
    let flags = read_options(&mut cursor, version, &mut archive)?;
    Some((cursor, archive, flags))
}

/// Reads the archive directory at its version-dependent offset.
///
/// # Arguments
///
/// * `bytes` - Raw target bytes.
/// * `cursor` - Cursor after the header.
/// * `archive` - Archive being built.
/// * `start` - Archive start offset.
///
/// # Returns
///
/// `Some(())` when the directory parses.
fn fill_directory(
    bytes: &[u8],
    cursor: &Cursor<'_>,
    archive: &mut PckArchive,
    start: usize,
) -> Option<()> {
    let directory = directory_offset(cursor, archive, start);
    read_directory(bytes, start + directory, archive.version, archive)
}

/// Returns the directory offset for the archive version.
///
/// # Arguments
///
/// * `cursor` - Cursor after the header.
/// * `archive` - Archive being built.
/// * `start` - Archive start offset.
///
/// # Returns
///
/// The directory offset relative to the archive start.
fn directory_offset(cursor: &Cursor<'_>, archive: &PckArchive, start: usize) -> usize {
    if archive.version >= 3 {
        archive.index
    } else {
        cursor.pos - start
    }
}

/// Builds the archive header fields.
///
/// # Arguments
///
/// * `version` - Format version.
/// * `major` - Engine major version.
/// * `minor` - Engine minor version.
/// * `patch` - Engine patch version.
/// * `start` - Archive start offset.
///
/// # Returns
///
/// The partially filled archive.
fn header(version: u32, major: u32, minor: u32, patch: u32, start: usize) -> PckArchive {
    PckArchive {
        version,
        engine: format!("{major}.{minor}.{patch}"),
        embedded_offset: start,
        ..PckArchive::default()
    }
}

/// Reads the version-dependent option fields and returns the flags.
///
/// # Arguments
///
/// * `cursor` - Input cursor.
/// * `version` - Format version.
/// * `archive` - Archive being built.
///
/// # Returns
///
/// The pack flags.
fn read_options(cursor: &mut Cursor<'_>, version: u32, archive: &mut PckArchive) -> Option<u32> {
    let flags = read_flags(cursor, version, archive)?;
    read_index(cursor, version, archive)?;
    cursor.skip(16 * 4)?;
    Some(flags)
}

/// Reads the pack flags and file base.
///
/// # Arguments
///
/// * `cursor` - Input cursor.
/// * `version` - Format version.
/// * `archive` - Archive being built.
///
/// # Returns
///
/// The pack flags.
fn read_flags(cursor: &mut Cursor<'_>, version: u32, archive: &mut PckArchive) -> Option<u32> {
    if version < 2 {
        return Some(0);
    }
    let flags = cursor.u32()?;
    archive.base = cursor.u64()?;
    Some(flags)
}

/// Reads the version 3 directory offset.
///
/// # Arguments
///
/// * `cursor` - Input cursor.
/// * `version` - Format version.
/// * `archive` - Archive being built.
///
/// # Returns
///
/// `Some(())` when the field is read or absent.
fn read_index(cursor: &mut Cursor<'_>, version: u32, archive: &mut PckArchive) -> Option<()> {
    if version >= 3 {
        archive.index = usize::try_from(cursor.u64()?).ok()?;
    }
    Some(())
}

/// Reads the archive directory.
///
/// # Arguments
///
/// * `bytes` - Raw target bytes.
/// * `offset` - Directory offset.
/// * `version` - Format version.
/// * `archive` - Archive being built.
///
/// # Returns
///
/// `Some(())` when the directory parses.
fn read_directory(
    bytes: &[u8],
    offset: usize,
    version: u32,
    archive: &mut PckArchive,
) -> Option<()> {
    let mut cursor = Cursor::new(bytes, offset);
    let count = cursor.u32()?;
    archive.file_count = count;
    let limit = usize::try_from(count).ok()?.min(MAX_ENTRIES);
    read_entries(&mut cursor, version, archive, limit)?;
    Some(())
}

/// Reads a bounded run of directory entries.
///
/// # Arguments
///
/// * `cursor` - Input cursor.
/// * `version` - Format version.
/// * `archive` - Archive being built.
/// * `limit` - Entry limit.
///
/// # Returns
///
/// `Some(())` when entries parse.
fn read_entries(
    cursor: &mut Cursor<'_>,
    version: u32,
    archive: &mut PckArchive,
    limit: usize,
) -> Option<()> {
    for _ in 0..limit {
        let entry = read_entry(cursor, version, archive.base, archive.embedded_offset)?;
        archive.files.push(entry);
    }
    Some(())
}

/// Reads one directory entry.
///
/// # Arguments
///
/// * `cursor` - Input cursor.
/// * `version` - Format version.
/// * `base` - File base offset.
/// * `start` - Archive start offset.
///
/// # Returns
///
/// The entry, or `None` on malformed input.
fn read_entry(cursor: &mut Cursor<'_>, version: u32, base: u64, start: usize) -> Option<PckEntry> {
    let path = read_path(cursor)?;
    let offset = cursor.u64()?;
    let size = cursor.u64()?;
    cursor.skip(16)?;
    let flags = if version >= 2 { cursor.u32()? } else { 0 };
    Some(PckEntry { path, offset: start as u64 + base + offset, size, encrypted: flags & 1 != 0 })
}

/// Reads a padded resource path.
///
/// # Arguments
///
/// * `cursor` - Input cursor.
///
/// # Returns
///
/// The path text, or `None` on malformed input.
fn read_path(cursor: &mut Cursor<'_>) -> Option<String> {
    let len = cursor.u32()? as usize;
    let bytes = cursor.take(len)?;
    let pad = (4 - len % 4) % 4;
    cursor.skip(pad)?;
    Some(String::from_utf8_lossy(bytes).trim_end_matches('\0').to_string())
}

/// Formats a PCK archive summary.
///
/// # Arguments
///
/// * `pck` - Recovered archive.
///
/// # Returns
///
/// The summary text.
#[must_use]
pub fn format_pck_summary(pck: &PckArchive) -> String {
    let (files, encrypted, total) = counts(pck);
    let directory = dir_suffix(pck);
    let (version, engine) = (pck.version, &pck.engine);
    format!("Godot PCK v{version} ({engine}): {files} files, {encrypted} encrypted, {total} bytes{directory}.\n")
}

/// Returns the encrypted-directory suffix.
///
/// # Arguments
///
/// * `pck` - Recovered archive.
///
/// # Returns
///
/// The suffix text.
fn dir_suffix(pck: &PckArchive) -> &'static str {
    if pck.encrypted_dir {
        " (encrypted directory)"
    } else {
        ""
    }
}

/// Counts archive files, encrypted files, and total size.
///
/// # Arguments
///
/// * `pck` - Recovered archive.
///
/// # Returns
///
/// The `(files, encrypted, total)` counts.
fn counts(pck: &PckArchive) -> (usize, usize, u64) {
    let total = pck.files.iter().map(|entry| entry.size).sum();
    let encrypted = pck.files.iter().filter(|entry| entry.encrypted).count();
    (pck.files.len(), encrypted, total)
}

/// A bounds-checked byte cursor.
struct Cursor<'a> {
    /// Source bytes.
    bytes: &'a [u8],

    /// Current offset.
    pos: usize,
}

impl<'a> Cursor<'a> {
    /// Builds a cursor at an offset.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Source bytes.
    /// * `pos` - Starting offset.
    ///
    /// # Returns
    ///
    /// The cursor.
    fn new(bytes: &'a [u8], pos: usize) -> Cursor<'a> {
        Cursor { bytes, pos }
    }

    /// Reads a little-endian `u32`.
    ///
    /// # Returns
    ///
    /// The value, or `None` at the end of input.
    fn u32(&mut self) -> Option<u32> {
        let b = self.take(4)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// Reads a little-endian `u64`.
    ///
    /// # Returns
    ///
    /// The value, or `None` at the end of input.
    fn u64(&mut self) -> Option<u64> {
        let b = self.take(8)?;
        let mut buf = [0u8; 8];
        buf.copy_from_slice(b);
        Some(u64::from_le_bytes(buf))
    }

    /// Borrows the next `n` bytes.
    ///
    /// # Arguments
    ///
    /// * `n` - Byte count.
    ///
    /// # Returns
    ///
    /// The slice, or `None` when out of bounds.
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(n)?;
        let slice = self.bytes.get(self.pos..end)?;
        self.pos = end;
        Some(slice)
    }

    /// Advances by `n` bytes.
    ///
    /// # Arguments
    ///
    /// * `n` - Byte count.
    ///
    /// # Returns
    ///
    /// `Some(())` when the advance is in bounds.
    fn skip(&mut self, n: usize) -> Option<()> {
        self.take(n).map(|_| ())
    }
}

/// Reads a little-endian `u64` at an offset.
///
/// # Arguments
///
/// * `bytes` - Source bytes.
/// * `at` - Source offset.
///
/// # Returns
///
/// The value, or `None` when out of bounds.
fn read_u64(bytes: &[u8], at: usize) -> Option<u64> {
    let b = bytes.get(at..at + 8)?;
    let mut buf = [0u8; 8];
    buf.copy_from_slice(b);
    Some(u64::from_le_bytes(buf))
}

#[cfg(test)]
mod tests {
    // Import all parent module items
    use super::*;

    /// Appends a little-endian `u32`.
    ///
    /// # Arguments
    ///
    /// * `buf` - Output buffer.
    /// * `value` - Value to encode.
    fn push_u32(buf: &mut Vec<u8>, value: u32) {
        buf.extend_from_slice(&value.to_le_bytes());
    }

    /// Appends a little-endian `u64`.
    ///
    /// # Arguments
    ///
    /// * `buf` - Output buffer.
    /// * `value` - Value to encode.
    fn push_u64(buf: &mut Vec<u8>, value: u64) {
        buf.extend_from_slice(&value.to_le_bytes());
    }

    /// Appends a padded resource path.
    ///
    /// # Arguments
    ///
    /// * `buf` - Output buffer.
    /// * `path` - Resource path.
    fn push_path(buf: &mut Vec<u8>, path: &str) {
        push_u32(buf, u32::try_from(path.len()).unwrap_or(u32::MAX));
        buf.extend_from_slice(path.as_bytes());
        while !buf.len().is_multiple_of(4) {
            buf.push(0);
        }
    }

    /// Appends one directory entry.
    ///
    /// # Arguments
    ///
    /// * `buf` - Output buffer.
    /// * `path` - Resource path.
    fn push_entry(buf: &mut Vec<u8>, path: &str) {
        push_path(buf, path);
        push_u64(buf, 0);
        push_u64(buf, 5);
        buf.extend_from_slice(&[0u8; 16]);
        push_u32(buf, 0);
    }

    /// Builds a synthetic v2 PCK.
    ///
    /// # Returns
    ///
    /// The encoded archive.
    fn pck_v2() -> Vec<u8> {
        let mut buf = pck_header_v2();
        push_u32(&mut buf, 1);
        push_entry(&mut buf, "res://a.txt");
        buf
    }

    /// Builds a synthetic v2 PCK header.
    ///
    /// # Returns
    ///
    /// The encoded header.
    fn pck_header_v2() -> Vec<u8> {
        let mut buf = MAGIC.to_vec();
        push_u64(&mut buf, 2 | (4 << 32));
        push_u64(&mut buf, 4);
        push_u32(&mut buf, 0);
        push_u64(&mut buf, 0);
        buf.extend_from_slice(&[0u8; 64]);
        buf
    }

    /// Verifies standalone v2 parsing.
    #[test]
    fn test_parse_pck_v2() {
        let pck = parse_pck(&pck_v2()).expect("pck");
        assert_eq!(pck.version, 2);
        assert_eq!(pck.files.len(), 1);
        assert_eq!(pck.files[0].path, "res://a.txt");
        assert_eq!(pck.files[0].size, 5);
    }

    /// Verifies embedded-archive location via the trailer.
    #[test]
    fn test_embedded_pck() {
        let mut buf = vec![0xabu8; 40];
        let start = buf.len();
        buf.extend_from_slice(&pck_v2());
        push_trailer(&mut buf, start);
        let pck = parse_pck(&buf).expect("embedded");
        assert_eq!(pck.embedded_offset, start);
        assert_eq!(pck.files.len(), 1);
    }

    /// Appends the embedded-archive trailer.
    ///
    /// # Arguments
    ///
    /// * `buf` - Output buffer.
    /// * `start` - Archive start offset.
    fn push_trailer(buf: &mut Vec<u8>, start: usize) {
        let size = u64::try_from(buf.len() - start).unwrap_or(0);
        push_u64(buf, size);
        buf.extend_from_slice(&MAGIC);
    }

    /// Verifies that ordinary data is not treated as a PCK.
    #[test]
    fn test_rejects_plain_data() {
        assert!(!is_pck(b"not a godot pack file"));
    }
}
