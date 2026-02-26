use std::fs::File;
use std::path::{Path, PathBuf};

use memmap2::Mmap;

use crate::HexError;

/// A read-only, memory-mapped file handle.
///
/// Uses `memmap2` to map the file into virtual memory, allowing efficient
/// random access without loading the entire file into a heap buffer.
pub struct HexFile {
    mmap: Mmap,
    path: PathBuf,
}

impl HexFile {
    /// Open and memory-map a file. Returns an error if the file is empty or unreadable.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, HexError> {
        let path = path.as_ref().to_path_buf();
        let file = File::open(&path).map_err(HexError::Io)?;
        let metadata = file.metadata().map_err(HexError::Io)?;

        if metadata.len() == 0 {
            return Err(HexError::EmptyFile);
        }

        // SAFETY: We hold the file open and treat the mapping as read-only.
        // The file could be modified externally, which is a known limitation.
        let mmap = unsafe { Mmap::map(&file) }.map_err(HexError::Io)?;

        Ok(Self { mmap, path })
    }

    /// The original filesystem path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// File size in bytes.
    pub fn len(&self) -> usize {
        self.mmap.len()
    }

    /// Whether the file is empty (always `false` since `open` rejects empty files).
    pub fn is_empty(&self) -> bool {
        self.mmap.is_empty()
    }

    /// Read a single byte, or `None` if `offset` is out of range.
    pub fn byte_at(&self, offset: usize) -> Option<u8> {
        self.mmap.get(offset).copied()
    }

    /// Read a half-open range `[start..end)`, clamped to the file length.
    pub fn read_range(&self, start: usize, end: usize) -> &[u8] {
        let end = end.min(self.mmap.len());
        let start = start.min(end);
        &self.mmap[start..end]
    }

    /// The full file contents as a byte slice.
    pub fn as_bytes(&self) -> &[u8] {
        &self.mmap
    }

    /// Number of rows for a given column count.
    pub fn row_count(&self, columns: usize) -> usize {
        self.mmap.len().div_ceil(columns)
    }

    /// Read one row of bytes (may be shorter than `columns` for the last row).
    pub fn read_row(&self, row: usize, columns: usize) -> &[u8] {
        let start = row * columns;
        let end = (start + columns).min(self.mmap.len());
        if start >= self.mmap.len() {
            return &[];
        }
        &self.mmap[start..end]
    }
}
