use std::fs::File;
use std::path::{Path, PathBuf};

use memmap2::Mmap;

use crate::HexError;

pub struct HexFile {
    mmap: Mmap,
    path: PathBuf,
}

impl HexFile {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, HexError> {
        let path = path.as_ref().to_path_buf();
        let file = File::open(&path).map_err(|e| HexError::Io(e))?;
        let metadata = file.metadata().map_err(|e| HexError::Io(e))?;

        if metadata.len() == 0 {
            return Err(HexError::EmptyFile);
        }

        // SAFETY: We hold the file open and treat the mapping as read-only.
        // The file could be modified externally, which is a known limitation.
        let mmap = unsafe { Mmap::map(&file) }.map_err(|e| HexError::Io(e))?;

        Ok(Self { mmap, path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn len(&self) -> usize {
        self.mmap.len()
    }

    pub fn is_empty(&self) -> bool {
        self.mmap.is_empty()
    }

    pub fn byte_at(&self, offset: usize) -> Option<u8> {
        self.mmap.get(offset).copied()
    }

    pub fn read_range(&self, start: usize, end: usize) -> &[u8] {
        let end = end.min(self.mmap.len());
        let start = start.min(end);
        &self.mmap[start..end]
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.mmap
    }

    /// Number of rows for a given column count.
    pub fn row_count(&self, columns: usize) -> usize {
        (self.mmap.len() + columns - 1) / columns
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
