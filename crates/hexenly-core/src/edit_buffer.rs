use std::path::{Path, PathBuf};

use crate::{HexError, HexFile};

/// Editing mode for the hex editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditMode {
    /// Typing replaces bytes in-place (file size unchanged).
    Overwrite,
    /// Typing inserts new bytes at cursor (file size grows).
    Insert,
}

/// A reversible editing operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditOp {
    Overwrite {
        offset: usize,
        old_byte: u8,
        new_byte: u8,
    },
    Insert {
        offset: usize,
        byte: u8,
    },
    Delete {
        offset: usize,
        byte: u8,
    },
    DeleteRange {
        offset: usize,
        bytes: Vec<u8>,
    },
    OverwriteRange {
        offset: usize,
        old_bytes: Vec<u8>,
        new_bytes: Vec<u8>,
    },
}

/// In-memory editing buffer with undo/redo support.
///
/// Created from a `HexFile` (or raw bytes), this holds a mutable copy of
/// the file data and tracks all edits so they can be undone/redone.
pub struct EditBuffer {
    data: Vec<u8>,
    undo_stack: Vec<EditOp>,
    redo_stack: Vec<EditOp>,
    dirty: bool,
    mode: EditMode,
    file_path: Option<PathBuf>,
}

impl EditBuffer {
    // ── Constructors ──────────────────────────────────────────────

    /// Create an `EditBuffer` from raw bytes (no associated file path).
    pub fn from_bytes(data: Vec<u8>) -> Self {
        Self {
            data,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            dirty: false,
            mode: EditMode::Overwrite,
            file_path: None,
        }
    }

    /// Create an `EditBuffer` from an open `HexFile`.
    pub fn from_file(hex_file: &HexFile) -> Self {
        Self {
            data: hex_file.as_bytes().to_vec(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            dirty: false,
            mode: EditMode::Overwrite,
            file_path: Some(hex_file.path().to_path_buf()),
        }
    }

    // ── Read accessors (mirror HexFile API) ───────────────────────

    /// The full buffer contents.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Length in bytes.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Read a single byte, or `None` if `offset` is out of range.
    pub fn byte_at(&self, offset: usize) -> Option<u8> {
        self.data.get(offset).copied()
    }

    /// Read a half-open range `[start..end)`, clamped to the buffer length.
    pub fn read_range(&self, start: usize, end: usize) -> &[u8] {
        let end = end.min(self.data.len());
        let start = start.min(end);
        &self.data[start..end]
    }

    /// Number of rows for a given column count.
    pub fn row_count(&self, columns: usize) -> usize {
        self.data.len().div_ceil(columns)
    }

    /// Read one row of bytes (may be shorter than `columns` for the last row).
    pub fn read_row(&self, row: usize, columns: usize) -> &[u8] {
        let start = row * columns;
        let end = (start + columns).min(self.data.len());
        if start >= self.data.len() {
            return &[];
        }
        &self.data[start..end]
    }

    // ── State accessors ───────────────────────────────────────────

    /// Whether the buffer has unsaved changes.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Current editing mode.
    pub fn mode(&self) -> EditMode {
        self.mode
    }

    /// Set the editing mode.
    pub fn set_mode(&mut self, mode: EditMode) {
        self.mode = mode;
    }

    /// Toggle between `Overwrite` and `Insert`.
    pub fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            EditMode::Overwrite => EditMode::Insert,
            EditMode::Insert => EditMode::Overwrite,
        };
    }

    /// The associated file path, if any.
    pub fn file_path(&self) -> Option<&Path> {
        self.file_path.as_deref()
    }

    /// Set the associated file path.
    pub fn set_file_path(&mut self, path: impl Into<PathBuf>) {
        self.file_path = Some(path.into());
    }

    // ── Edit operations ───────────────────────────────────────────

    /// Replace a single byte at `offset`. No-op if the byte is unchanged.
    pub fn overwrite_byte(&mut self, offset: usize, new_byte: u8) {
        if let Some(&old_byte) = self.data.get(offset) {
            if old_byte == new_byte {
                return;
            }
            self.data[offset] = new_byte;
            self.push_op(EditOp::Overwrite {
                offset,
                old_byte,
                new_byte,
            });
        }
    }

    /// Insert a byte at `offset`, shifting subsequent bytes right.
    pub fn insert_byte(&mut self, offset: usize, byte: u8) {
        let offset = offset.min(self.data.len());
        self.data.insert(offset, byte);
        self.push_op(EditOp::Insert { offset, byte });
    }

    /// Delete the byte at `offset`, shifting subsequent bytes left.
    pub fn delete_byte(&mut self, offset: usize) {
        if offset < self.data.len() {
            let byte = self.data.remove(offset);
            self.push_op(EditOp::Delete { offset, byte });
        }
    }

    /// Delete a range of bytes `[start..=end_inclusive]`.
    pub fn delete_range(&mut self, start: usize, end_inclusive: usize) {
        if start >= self.data.len() {
            return;
        }
        let end = (end_inclusive + 1).min(self.data.len());
        let bytes: Vec<u8> = self.data.drain(start..end).collect();
        if !bytes.is_empty() {
            self.push_op(EditOp::DeleteRange {
                offset: start,
                bytes,
            });
        }
    }

    /// Overwrite a range of bytes starting at `offset` with `new_bytes`.
    pub fn overwrite_range(&mut self, offset: usize, new_bytes: &[u8]) {
        if new_bytes.is_empty() || offset >= self.data.len() {
            return;
        }
        let end = (offset + new_bytes.len()).min(self.data.len());
        let old_bytes = self.data[offset..end].to_vec();
        let actual_new = &new_bytes[..end - offset];
        self.data[offset..end].copy_from_slice(actual_new);
        self.push_op(EditOp::OverwriteRange {
            offset,
            old_bytes,
            new_bytes: actual_new.to_vec(),
        });
    }

    /// Push an operation onto the undo stack and clear the redo stack.
    fn push_op(&mut self, op: EditOp) {
        self.undo_stack.push(op);
        self.redo_stack.clear();
        self.dirty = true;
    }

    // ── Undo / Redo ───────────────────────────────────────────────

    /// Whether there are operations to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Whether there are operations to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Undo the most recent operation. Returns `true` if an operation was undone.
    pub fn undo(&mut self) -> bool {
        if let Some(op) = self.undo_stack.pop() {
            self.apply_reverse(&op);
            self.redo_stack.push(op);
            self.dirty = !self.undo_stack.is_empty();
            true
        } else {
            false
        }
    }

    /// Redo the most recently undone operation. Returns `true` if an operation was redone.
    pub fn redo(&mut self) -> bool {
        if let Some(op) = self.redo_stack.pop() {
            self.apply_forward(&op);
            self.undo_stack.push(op);
            self.dirty = true;
            true
        } else {
            false
        }
    }

    /// Apply an operation in reverse (for undo).
    fn apply_reverse(&mut self, op: &EditOp) {
        match op {
            EditOp::Overwrite {
                offset, old_byte, ..
            } => {
                self.data[*offset] = *old_byte;
            }
            EditOp::Insert { offset, .. } => {
                self.data.remove(*offset);
            }
            EditOp::Delete { offset, byte } => {
                self.data.insert(*offset, *byte);
            }
            EditOp::DeleteRange { offset, bytes } => {
                for (i, &b) in bytes.iter().enumerate() {
                    self.data.insert(*offset + i, b);
                }
            }
            EditOp::OverwriteRange {
                offset, old_bytes, ..
            } => {
                self.data[*offset..*offset + old_bytes.len()].copy_from_slice(old_bytes);
            }
        }
    }

    /// Apply an operation forward (for redo).
    fn apply_forward(&mut self, op: &EditOp) {
        match op {
            EditOp::Overwrite {
                offset, new_byte, ..
            } => {
                self.data[*offset] = *new_byte;
            }
            EditOp::Insert { offset, byte } => {
                self.data.insert(*offset, *byte);
            }
            EditOp::Delete { offset, .. } => {
                self.data.remove(*offset);
            }
            EditOp::DeleteRange { offset, bytes } => {
                self.data.drain(*offset..*offset + bytes.len());
            }
            EditOp::OverwriteRange {
                offset, new_bytes, ..
            } => {
                self.data[*offset..*offset + new_bytes.len()].copy_from_slice(new_bytes);
            }
        }
    }

    // ── Save ──────────────────────────────────────────────────────

    /// Save the buffer to its associated file path.
    pub fn save(&mut self) -> Result<(), HexError> {
        let path = self.file_path.clone().ok_or(HexError::NoFilePath)?;
        self.write_atomic(&path)?;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.dirty = false;
        Ok(())
    }

    /// Save the buffer to a new path and update the stored file path.
    pub fn save_as(&mut self, path: &Path) -> Result<(), HexError> {
        self.write_atomic(path)?;
        self.file_path = Some(path.to_path_buf());
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.dirty = false;
        Ok(())
    }

    /// Write data to `path` atomically via a temp file + rename.
    fn write_atomic(&self, path: &Path) -> Result<(), HexError> {
        use std::io::Write;

        let dir = path.parent().unwrap_or(Path::new("."));
        let mut tmp = tempfile::NamedTempFile::new_in(dir).map_err(HexError::Io)?;
        tmp.write_all(&self.data).map_err(HexError::Io)?;
        tmp.persist(path)
            .map_err(|e| HexError::Io(e.error))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overwrite_changes_data_and_sets_dirty() {
        let mut buf = EditBuffer::from_bytes(vec![0x00, 0x11, 0x22]);
        assert!(!buf.is_dirty());

        buf.overwrite_byte(1, 0xFF);

        assert_eq!(buf.byte_at(1), Some(0xFF));
        assert!(buf.is_dirty());
        assert_eq!(buf.len(), 3);
    }

    #[test]
    fn overwrite_same_byte_is_noop() {
        let mut buf = EditBuffer::from_bytes(vec![0xAA, 0xBB]);
        buf.overwrite_byte(0, 0xAA);

        assert!(!buf.is_dirty());
        assert!(!buf.can_undo());
    }

    #[test]
    fn insert_shifts_data_and_increases_len() {
        let mut buf = EditBuffer::from_bytes(vec![0x00, 0x11]);
        buf.insert_byte(1, 0xFF);

        assert_eq!(buf.len(), 3);
        assert_eq!(buf.data(), &[0x00, 0xFF, 0x11]);
        assert!(buf.is_dirty());
    }

    #[test]
    fn delete_shrinks_data() {
        let mut buf = EditBuffer::from_bytes(vec![0x00, 0x11, 0x22]);
        buf.delete_byte(1);

        assert_eq!(buf.len(), 2);
        assert_eq!(buf.data(), &[0x00, 0x22]);
        assert!(buf.is_dirty());
    }

    #[test]
    fn delete_range_removes_span() {
        let mut buf = EditBuffer::from_bytes(vec![0x00, 0x11, 0x22, 0x33, 0x44]);
        buf.delete_range(1, 3);

        assert_eq!(buf.len(), 2);
        assert_eq!(buf.data(), &[0x00, 0x44]);
    }

    #[test]
    fn overwrite_range_replaces_bytes() {
        let mut buf = EditBuffer::from_bytes(vec![0x00, 0x11, 0x22, 0x33]);
        buf.overwrite_range(1, &[0xAA, 0xBB]);

        assert_eq!(buf.data(), &[0x00, 0xAA, 0xBB, 0x33]);
        assert!(buf.is_dirty());
    }

    #[test]
    fn undo_reverses_overwrite() {
        let mut buf = EditBuffer::from_bytes(vec![0x00, 0x11]);
        buf.overwrite_byte(0, 0xFF);
        assert!(buf.undo());
        assert_eq!(buf.byte_at(0), Some(0x00));
        assert!(!buf.is_dirty());
    }

    #[test]
    fn undo_reverses_insert() {
        let mut buf = EditBuffer::from_bytes(vec![0x00, 0x11]);
        buf.insert_byte(1, 0xFF);
        assert!(buf.undo());
        assert_eq!(buf.data(), &[0x00, 0x11]);
        assert!(!buf.is_dirty());
    }

    #[test]
    fn undo_reverses_delete() {
        let mut buf = EditBuffer::from_bytes(vec![0x00, 0x11, 0x22]);
        buf.delete_byte(1);
        assert!(buf.undo());
        assert_eq!(buf.data(), &[0x00, 0x11, 0x22]);
    }

    #[test]
    fn undo_reverses_delete_range() {
        let mut buf = EditBuffer::from_bytes(vec![0x00, 0x11, 0x22, 0x33]);
        buf.delete_range(1, 2);
        assert_eq!(buf.data(), &[0x00, 0x33]);
        assert!(buf.undo());
        assert_eq!(buf.data(), &[0x00, 0x11, 0x22, 0x33]);
    }

    #[test]
    fn undo_reverses_overwrite_range() {
        let mut buf = EditBuffer::from_bytes(vec![0x00, 0x11, 0x22]);
        buf.overwrite_range(0, &[0xAA, 0xBB]);
        assert!(buf.undo());
        assert_eq!(buf.data(), &[0x00, 0x11, 0x22]);
    }

    #[test]
    fn redo_replays_operations() {
        let mut buf = EditBuffer::from_bytes(vec![0x00, 0x11]);
        buf.overwrite_byte(0, 0xFF);
        buf.undo();
        assert_eq!(buf.byte_at(0), Some(0x00));

        assert!(buf.redo());
        assert_eq!(buf.byte_at(0), Some(0xFF));
        assert!(buf.is_dirty());
    }

    #[test]
    fn redo_replays_insert() {
        let mut buf = EditBuffer::from_bytes(vec![0x00]);
        buf.insert_byte(0, 0xFF);
        buf.undo();
        assert!(buf.redo());
        assert_eq!(buf.data(), &[0xFF, 0x00]);
    }

    #[test]
    fn redo_replays_delete() {
        let mut buf = EditBuffer::from_bytes(vec![0x00, 0x11]);
        buf.delete_byte(0);
        buf.undo();
        assert!(buf.redo());
        assert_eq!(buf.data(), &[0x11]);
    }

    #[test]
    fn new_edit_clears_redo_stack() {
        let mut buf = EditBuffer::from_bytes(vec![0x00, 0x11]);
        buf.overwrite_byte(0, 0xFF);
        buf.undo();
        assert!(buf.can_redo());

        buf.overwrite_byte(1, 0xEE);
        assert!(!buf.can_redo());
    }

    #[test]
    fn toggle_mode_switches() {
        let mut buf = EditBuffer::from_bytes(vec![]);
        assert_eq!(buf.mode(), EditMode::Overwrite);

        buf.toggle_mode();
        assert_eq!(buf.mode(), EditMode::Insert);

        buf.toggle_mode();
        assert_eq!(buf.mode(), EditMode::Overwrite);
    }

    #[test]
    fn set_mode() {
        let mut buf = EditBuffer::from_bytes(vec![]);
        buf.set_mode(EditMode::Insert);
        assert_eq!(buf.mode(), EditMode::Insert);
        buf.set_mode(EditMode::Overwrite);
        assert_eq!(buf.mode(), EditMode::Overwrite);
    }

    #[test]
    fn undo_empty_returns_false() {
        let mut buf = EditBuffer::from_bytes(vec![0x00]);
        assert!(!buf.undo());
    }

    #[test]
    fn redo_empty_returns_false() {
        let mut buf = EditBuffer::from_bytes(vec![0x00]);
        assert!(!buf.redo());
    }

    #[test]
    fn can_undo_and_redo() {
        let mut buf = EditBuffer::from_bytes(vec![0x00]);
        assert!(!buf.can_undo());
        assert!(!buf.can_redo());

        buf.overwrite_byte(0, 0xFF);
        assert!(buf.can_undo());
        assert!(!buf.can_redo());

        buf.undo();
        assert!(!buf.can_undo());
        assert!(buf.can_redo());
    }

    #[test]
    fn read_accessors_match_hexfile_api() {
        let data = vec![0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77];
        let buf = EditBuffer::from_bytes(data.clone());

        assert_eq!(buf.len(), 8);
        assert!(!buf.is_empty());
        assert_eq!(buf.byte_at(3), Some(0x33));
        assert_eq!(buf.byte_at(100), None);
        assert_eq!(buf.read_range(2, 5), &[0x22, 0x33, 0x44]);
        assert_eq!(buf.row_count(4), 2);
        assert_eq!(buf.read_row(0, 4), &[0x00, 0x11, 0x22, 0x33]);
        assert_eq!(buf.read_row(1, 4), &[0x44, 0x55, 0x66, 0x77]);
        assert_eq!(buf.read_row(2, 4), &[] as &[u8]);
    }

    #[test]
    fn read_range_clamped() {
        let buf = EditBuffer::from_bytes(vec![0x00, 0x11]);
        assert_eq!(buf.read_range(0, 100), &[0x00, 0x11]);
        assert_eq!(buf.read_range(100, 200), &[] as &[u8]);
    }

    #[test]
    fn file_path_accessors() {
        let mut buf = EditBuffer::from_bytes(vec![]);
        assert!(buf.file_path().is_none());

        buf.set_file_path("/tmp/test.bin");
        assert_eq!(buf.file_path(), Some(Path::new("/tmp/test.bin")));
    }

    #[test]
    fn save_writes_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.bin");

        let mut buf = EditBuffer::from_bytes(vec![0xDE, 0xAD, 0xBE, 0xEF]);
        buf.set_file_path(&path);
        buf.overwrite_byte(0, 0xCA);
        assert!(buf.is_dirty());

        buf.save().unwrap();

        assert!(!buf.is_dirty());
        assert!(!buf.can_undo());
        assert!(!buf.can_redo());

        let written = std::fs::read(&path).unwrap();
        assert_eq!(written, vec![0xCA, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn save_as_writes_new_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("new_file.bin");

        let mut buf = EditBuffer::from_bytes(vec![0x01, 0x02, 0x03]);
        buf.save_as(&path).unwrap();

        assert_eq!(buf.file_path(), Some(path.as_path()));
        let written = std::fs::read(&path).unwrap();
        assert_eq!(written, vec![0x01, 0x02, 0x03]);
    }

    #[test]
    fn save_without_path_errors() {
        let mut buf = EditBuffer::from_bytes(vec![0x00]);
        let result = buf.save();
        assert!(result.is_err());
    }

    #[test]
    fn multiple_undo_redo_cycle() {
        let mut buf = EditBuffer::from_bytes(vec![0x00, 0x11, 0x22]);

        buf.overwrite_byte(0, 0xAA);
        buf.overwrite_byte(1, 0xBB);
        buf.overwrite_byte(2, 0xCC);

        assert_eq!(buf.data(), &[0xAA, 0xBB, 0xCC]);

        buf.undo();
        assert_eq!(buf.data(), &[0xAA, 0xBB, 0x22]);

        buf.undo();
        assert_eq!(buf.data(), &[0xAA, 0x11, 0x22]);

        buf.undo();
        assert_eq!(buf.data(), &[0x00, 0x11, 0x22]);
        assert!(!buf.is_dirty());

        buf.redo();
        buf.redo();
        buf.redo();
        assert_eq!(buf.data(), &[0xAA, 0xBB, 0xCC]);
        assert!(buf.is_dirty());
    }

    #[test]
    fn overwrite_out_of_bounds_is_noop() {
        let mut buf = EditBuffer::from_bytes(vec![0x00]);
        buf.overwrite_byte(10, 0xFF);
        assert!(!buf.is_dirty());
        assert_eq!(buf.data(), &[0x00]);
    }

    #[test]
    fn delete_out_of_bounds_is_noop() {
        let mut buf = EditBuffer::from_bytes(vec![0x00]);
        buf.delete_byte(10);
        assert!(!buf.is_dirty());
    }

    #[test]
    fn delete_range_out_of_bounds_is_noop() {
        let mut buf = EditBuffer::from_bytes(vec![0x00]);
        buf.delete_range(10, 20);
        assert!(!buf.is_dirty());
    }

    #[test]
    fn insert_at_end() {
        let mut buf = EditBuffer::from_bytes(vec![0x00]);
        buf.insert_byte(1, 0xFF);
        assert_eq!(buf.data(), &[0x00, 0xFF]);
    }

    #[test]
    fn insert_beyond_end_clamps() {
        let mut buf = EditBuffer::from_bytes(vec![0x00]);
        buf.insert_byte(100, 0xFF);
        assert_eq!(buf.data(), &[0x00, 0xFF]);
    }

    #[test]
    fn overwrite_range_clamped_to_data_len() {
        let mut buf = EditBuffer::from_bytes(vec![0x00, 0x11]);
        buf.overwrite_range(1, &[0xAA, 0xBB, 0xCC]);
        // Only one byte after offset 1, so only 0xAA is written.
        assert_eq!(buf.data(), &[0x00, 0xAA]);
    }

    #[test]
    fn overwrite_range_empty_is_noop() {
        let mut buf = EditBuffer::from_bytes(vec![0x00]);
        buf.overwrite_range(0, &[]);
        assert!(!buf.is_dirty());
    }

    #[test]
    fn overwrite_range_out_of_bounds_is_noop() {
        let mut buf = EditBuffer::from_bytes(vec![0x00]);
        buf.overwrite_range(10, &[0xFF]);
        assert!(!buf.is_dirty());
    }

    #[test]
    fn from_bytes_starts_clean() {
        let buf = EditBuffer::from_bytes(vec![0x00]);
        assert!(!buf.is_dirty());
        assert_eq!(buf.mode(), EditMode::Overwrite);
        assert!(buf.file_path().is_none());
    }
}
