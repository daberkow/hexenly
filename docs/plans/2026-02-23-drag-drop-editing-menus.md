# Drag & Drop, File Editing, and Menu Reorganization Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add drag-and-drop file opening, full file editing with insert/overwrite modes and undo/redo, and reorganize the toolbar into a menu bar + status bar.

**Architecture:** EditBuffer in hexenly-core owns a `Vec<u8>` working copy of the file data, with a command-pattern undo/redo stack. The app reads all bytes from EditBuffer instead of HexFile's mmap. The toolbar becomes an egui menu bar, and a status bar shows edit mode + dirty state.

**Tech Stack:** Rust, egui/eframe 0.33, memmap2 (read-only initial load), rfd (file dialogs)

---

### Task 1: EditBuffer Data Model

Create the core editing buffer with undo/redo support in hexenly-core.

**Files:**
- Create: `crates/hexenly-core/src/edit_buffer.rs`
- Modify: `crates/hexenly-core/src/lib.rs:1-9`

**Step 1: Create edit_buffer.rs with types and constructor**

Create `crates/hexenly-core/src/edit_buffer.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditMode {
    Overwrite,
    Insert,
}

#[derive(Debug, Clone)]
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

pub struct EditBuffer {
    data: Vec<u8>,
    undo_stack: Vec<EditOp>,
    redo_stack: Vec<EditOp>,
    dirty: bool,
    mode: EditMode,
    file_path: Option<std::path::PathBuf>,
}

impl EditBuffer {
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

    pub fn from_file(file: &crate::HexFile) -> Self {
        let mut buf = Self::from_bytes(file.as_bytes().to_vec());
        buf.file_path = Some(file.path().to_path_buf());
        buf
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn byte_at(&self, offset: usize) -> Option<u8> {
        self.data.get(offset).copied()
    }

    pub fn read_range(&self, start: usize, end: usize) -> &[u8] {
        let end = end.min(self.data.len());
        let start = start.min(end);
        &self.data[start..end]
    }

    pub fn row_count(&self, columns: usize) -> usize {
        self.data.len().div_ceil(columns)
    }

    pub fn read_row(&self, row: usize, columns: usize) -> &[u8] {
        let start = row * columns;
        let end = (start + columns).min(self.data.len());
        if start >= self.data.len() {
            return &[];
        }
        &self.data[start..end]
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mode(&self) -> EditMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: EditMode) {
        self.mode = mode;
    }

    pub fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            EditMode::Overwrite => EditMode::Insert,
            EditMode::Insert => EditMode::Overwrite,
        };
    }

    pub fn file_path(&self) -> Option<&std::path::Path> {
        self.file_path.as_deref()
    }

    pub fn set_file_path(&mut self, path: std::path::PathBuf) {
        self.file_path = Some(path);
    }
}
```

**Step 2: Add edit operations (overwrite, insert, delete)**

Append to `EditBuffer` impl in `crates/hexenly-core/src/edit_buffer.rs`:

```rust
    pub fn overwrite_byte(&mut self, offset: usize, new_byte: u8) {
        if offset >= self.data.len() {
            return;
        }
        let old_byte = self.data[offset];
        if old_byte == new_byte {
            return;
        }
        self.data[offset] = new_byte;
        self.undo_stack.push(EditOp::Overwrite {
            offset,
            old_byte,
            new_byte,
        });
        self.redo_stack.clear();
        self.dirty = true;
    }

    pub fn insert_byte(&mut self, offset: usize, byte: u8) {
        let offset = offset.min(self.data.len());
        self.data.insert(offset, byte);
        self.undo_stack.push(EditOp::Insert { offset, byte });
        self.redo_stack.clear();
        self.dirty = true;
    }

    pub fn delete_byte(&mut self, offset: usize) {
        if offset >= self.data.len() {
            return;
        }
        let byte = self.data.remove(offset);
        self.undo_stack.push(EditOp::Delete { offset, byte });
        self.redo_stack.clear();
        self.dirty = true;
    }

    pub fn delete_range(&mut self, start: usize, end_inclusive: usize) {
        if start >= self.data.len() {
            return;
        }
        let end = (end_inclusive + 1).min(self.data.len());
        let bytes: Vec<u8> = self.data.drain(start..end).collect();
        if !bytes.is_empty() {
            self.undo_stack.push(EditOp::DeleteRange {
                offset: start,
                bytes,
            });
            self.redo_stack.clear();
            self.dirty = true;
        }
    }

    pub fn overwrite_range(&mut self, offset: usize, new_bytes: &[u8]) {
        if offset >= self.data.len() || new_bytes.is_empty() {
            return;
        }
        let end = (offset + new_bytes.len()).min(self.data.len());
        let old_bytes = self.data[offset..end].to_vec();
        self.data[offset..end].copy_from_slice(&new_bytes[..end - offset]);
        self.undo_stack.push(EditOp::OverwriteRange {
            offset,
            old_bytes,
            new_bytes: new_bytes[..end - offset].to_vec(),
        });
        self.redo_stack.clear();
        self.dirty = true;
    }
```

**Step 3: Add undo/redo**

Append to `EditBuffer` impl:

```rust
    pub fn undo(&mut self) -> bool {
        let Some(op) = self.undo_stack.pop() else {
            return false;
        };
        match &op {
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
                    self.data.insert(offset + i, b);
                }
            }
            EditOp::OverwriteRange {
                offset, old_bytes, ..
            } => {
                self.data[*offset..*offset + old_bytes.len()].copy_from_slice(old_bytes);
            }
        }
        self.redo_stack.push(op);
        self.dirty = !self.undo_stack.is_empty();
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(op) = self.redo_stack.pop() else {
            return false;
        };
        match &op {
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
                let end = (*offset + bytes.len()).min(self.data.len());
                self.data.drain(*offset..end);
            }
            EditOp::OverwriteRange {
                offset, new_bytes, ..
            } => {
                self.data[*offset..*offset + new_bytes.len()].copy_from_slice(new_bytes);
            }
        }
        self.undo_stack.push(op);
        self.dirty = true;
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }
```

**Step 4: Add save support**

Append to `EditBuffer` impl:

```rust
    /// Save data to the file path. Writes to a temp file then renames for atomicity.
    pub fn save(&mut self) -> Result<(), crate::HexError> {
        let Some(path) = &self.file_path else {
            return Err(crate::HexError::NoFilePath);
        };
        Self::write_atomic(path, &self.data)?;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.dirty = false;
        Ok(())
    }

    /// Save data to a new path. Updates the file_path.
    pub fn save_as(&mut self, path: &std::path::Path) -> Result<(), crate::HexError> {
        Self::write_atomic(path, &self.data)?;
        self.file_path = Some(path.to_path_buf());
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.dirty = false;
        Ok(())
    }

    fn write_atomic(path: &std::path::Path, data: &[u8]) -> Result<(), crate::HexError> {
        use std::io::Write;
        let dir = path.parent().unwrap_or(std::path::Path::new("."));
        let mut tmp = tempfile::NamedTempFile::new_in(dir).map_err(crate::HexError::Io)?;
        tmp.write_all(data).map_err(crate::HexError::Io)?;
        tmp.persist(path).map_err(|e| crate::HexError::Io(e.error))?;
        Ok(())
    }
```

**Step 5: Register module and add new error variant**

Modify `crates/hexenly-core/src/lib.rs` — add `pub mod edit_buffer;` and `pub use` line, plus new error variant:

```rust
pub mod edit_buffer;
pub mod file;
pub mod interpret;
pub mod search;
pub mod selection;

pub use edit_buffer::{EditBuffer, EditMode};
pub use file::HexFile;
pub use interpret::{ByteClass, ByteInterpreter, Interpretation, classify_byte};
pub use search::{SearchPattern, find_all, find_next, find_prev};
pub use selection::{Bookmark, Selection};

#[derive(Debug, thiserror::Error)]
pub enum HexError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("File is empty")]
    EmptyFile,
    #[error("No file path set")]
    NoFilePath,
}
```

**Step 6: Add tempfile dependency**

Add `tempfile` to workspace deps in root `Cargo.toml`:
```toml
tempfile = "3"
```

Add to `crates/hexenly-core/Cargo.toml`:
```toml
tempfile = { workspace = true }
```

**Step 7: Write unit tests**

Append to `crates/hexenly-core/src/edit_buffer.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overwrite_byte_changes_data() {
        let mut buf = EditBuffer::from_bytes(vec![0x00, 0x11, 0x22]);
        buf.overwrite_byte(1, 0xFF);
        assert_eq!(buf.data(), &[0x00, 0xFF, 0x22]);
        assert!(buf.is_dirty());
    }

    #[test]
    fn insert_byte_shifts_data() {
        let mut buf = EditBuffer::from_bytes(vec![0xAA, 0xBB]);
        buf.insert_byte(1, 0xCC);
        assert_eq!(buf.data(), &[0xAA, 0xCC, 0xBB]);
        assert_eq!(buf.len(), 3);
    }

    #[test]
    fn delete_byte_shrinks_data() {
        let mut buf = EditBuffer::from_bytes(vec![0xAA, 0xBB, 0xCC]);
        buf.delete_byte(1);
        assert_eq!(buf.data(), &[0xAA, 0xCC]);
        assert_eq!(buf.len(), 2);
    }

    #[test]
    fn delete_range_removes_span() {
        let mut buf = EditBuffer::from_bytes(vec![0x01, 0x02, 0x03, 0x04, 0x05]);
        buf.delete_range(1, 3);
        assert_eq!(buf.data(), &[0x01, 0x05]);
    }

    #[test]
    fn undo_overwrite_restores_old_byte() {
        let mut buf = EditBuffer::from_bytes(vec![0xAA, 0xBB]);
        buf.overwrite_byte(0, 0xFF);
        assert_eq!(buf.data()[0], 0xFF);
        buf.undo();
        assert_eq!(buf.data()[0], 0xAA);
    }

    #[test]
    fn undo_insert_removes_byte() {
        let mut buf = EditBuffer::from_bytes(vec![0xAA, 0xBB]);
        buf.insert_byte(1, 0xCC);
        assert_eq!(buf.len(), 3);
        buf.undo();
        assert_eq!(buf.data(), &[0xAA, 0xBB]);
    }

    #[test]
    fn undo_delete_reinserts_byte() {
        let mut buf = EditBuffer::from_bytes(vec![0xAA, 0xBB, 0xCC]);
        buf.delete_byte(1);
        buf.undo();
        assert_eq!(buf.data(), &[0xAA, 0xBB, 0xCC]);
    }

    #[test]
    fn redo_replays_operation() {
        let mut buf = EditBuffer::from_bytes(vec![0xAA, 0xBB]);
        buf.overwrite_byte(0, 0xFF);
        buf.undo();
        assert_eq!(buf.data()[0], 0xAA);
        buf.redo();
        assert_eq!(buf.data()[0], 0xFF);
    }

    #[test]
    fn new_edit_clears_redo_stack() {
        let mut buf = EditBuffer::from_bytes(vec![0xAA, 0xBB]);
        buf.overwrite_byte(0, 0xFF);
        buf.undo();
        assert!(buf.can_redo());
        buf.overwrite_byte(1, 0xCC);
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
    fn overwrite_same_byte_is_noop() {
        let mut buf = EditBuffer::from_bytes(vec![0xAA]);
        buf.overwrite_byte(0, 0xAA);
        assert!(!buf.is_dirty());
        assert!(!buf.can_undo());
    }

    #[test]
    fn save_writes_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.bin");
        std::fs::write(&path, &[0xAA, 0xBB]).unwrap();

        let file = crate::HexFile::open(&path).unwrap();
        let mut buf = EditBuffer::from_file(&file);
        buf.overwrite_byte(0, 0xFF);
        buf.save().unwrap();

        let saved = std::fs::read(&path).unwrap();
        assert_eq!(saved, &[0xFF, 0xBB]);
        assert!(!buf.is_dirty());
        assert!(!buf.can_undo());
    }
}
```

**Step 8: Run tests to verify**

Run: `cargo test -p hexenly-core`
Expected: All tests pass.

**Step 9: Commit**

```bash
git add crates/hexenly-core/src/edit_buffer.rs crates/hexenly-core/src/lib.rs crates/hexenly-core/Cargo.toml Cargo.toml
git commit -m "feat: add EditBuffer with undo/redo and atomic save"
```

---

### Task 2: Integrate EditBuffer into HexenlyApp

Replace all byte reads from HexFile with EditBuffer. Keep HexFile for initial mmap load only.

**Files:**
- Modify: `crates/hexenly-app/src/app.rs`

**Step 1: Add EditBuffer field and update constructor**

In `crates/hexenly-app/src/app.rs`, add to imports:

```rust
use hexenly_core::{Bookmark, EditBuffer, EditMode, HexFile, SearchPattern, Selection, find_all};
```

Add field to `HexenlyApp` struct (after `file: Option<HexFile>`):

```rust
    edit_buffer: Option<EditBuffer>,
```

Add to `HexenlyApp::new()` constructor initializer:

```rust
    edit_buffer: None,
```

**Step 2: Update open_path to create EditBuffer**

In `open_path()`, after `self.file = Some(f);` and before `self.cursor_offset = 0;`, add:

```rust
                // Create edit buffer from file data
                let file_ref = self.file.as_ref().unwrap();
                if file_ref.len() > 100 * 1024 * 1024 {
                    self.notifications.push(Notification {
                        message: format!(
                            "Large file ({}) — editing will use significant memory",
                            format_size(file_ref.len())
                        ),
                        level: NotificationLevel::Warning,
                        created: Instant::now(),
                    });
                }
                self.edit_buffer = Some(EditBuffer::from_file(file_ref));
```

**Step 3: Add helper methods for reading through EditBuffer**

Add helper methods to `HexenlyApp`:

```rust
    /// Get a reference to the data bytes (from edit buffer, falling back to file).
    fn data_bytes(&self) -> Option<&[u8]> {
        if let Some(buf) = &self.edit_buffer {
            Some(buf.data())
        } else {
            self.file.as_ref().map(|f| f.as_bytes())
        }
    }

    fn data_len(&self) -> usize {
        self.edit_buffer
            .as_ref()
            .map(|b| b.len())
            .or_else(|| self.file.as_ref().map(|f| f.len()))
            .unwrap_or(0)
    }
```

**Step 4: Update hex_view::show to accept &[u8] instead of &HexFile**

In `crates/hexenly-app/src/panels/hex_view.rs`, change the `show` function signature:

```rust
pub fn show(
    ui: &mut Ui,
    data: &[u8],
    total_len: usize,
    columns: usize,
    cursor: usize,
    selection: Option<&Selection>,
    search_matches: &[usize],
    show_ascii: bool,
    state: &mut HexViewState,
    template_overlay: Option<&ResolvedTemplate>,
) -> Option<HexViewAction> {
```

Replace all `file.` calls inside `show()`:
- `file.row_count(columns)` → `total_len.div_ceil(columns)`
- `file.read_row(row, columns)` → the slice `&data[start..end]` where start = `row * columns`, end = `(start + columns).min(total_len)`, guarded by `if start >= total_len { &[] }`
- `file.len()` → `total_len`

Remove the `use hexenly_core::HexFile;` import from hex_view.rs.

**Step 5: Update inspector::show to accept &[u8] instead of &HexFile**

In `crates/hexenly-app/src/panels/inspector.rs`, change:

```rust
pub fn show(ui: &mut Ui, data: &[u8], cursor: usize) {
```

Replace `file.as_bytes()` with `data`. Remove the `HexFile` import.

**Step 6: Update all call sites in app.rs**

In the `update()` method of `app.rs`, update the hex_view call:

```rust
                let data = self.edit_buffer.as_ref().map(|b| b.data()).unwrap_or(file.as_bytes());
                let data_len = self.edit_buffer.as_ref().map(|b| b.len()).unwrap_or(file.len());
                let action = hex_view::show(
                    ui,
                    data,
                    data_len,
                    self.columns,
                    self.cursor_offset,
                    self.selection.as_ref(),
                    &self.search_matches,
                    self.show_ascii_pane,
                    &mut self.hex_view_state,
                    self.resolved_template.as_ref(),
                );
```

Update the inspector call:

```rust
                    if let Some(buf) = &self.edit_buffer {
                        inspector::show(ui, buf.data(), self.cursor_offset);
                    } else if let Some(file) = &self.file {
                        inspector::show(ui, file.as_bytes(), self.cursor_offset);
                    } else {
                        ui.label("No file open");
                    }
```

Update `selected_bytes()` to read from edit_buffer:

```rust
    fn selected_bytes(&self) -> Option<&[u8]> {
        let sel = self.selection.as_ref()?;
        let bytes = self.data_bytes()?;
        let start = sel.start.min(bytes.len());
        let end = (sel.end + 1).min(bytes.len());
        Some(&bytes[start..end])
    }
```

Update `do_search()` to search edit buffer data:

```rust
    fn do_search(&mut self) {
        self.search_error = None;
        let Some(bytes) = self.data_bytes() else { return };
        // ... pattern parsing stays the same ...
        self.search_matches = find_all(bytes, &pattern, 10_000);
        // ... rest stays the same ...
    }
```

Update `move_cursor()`, `set_cursor_abs()`, `move_cursor_select()`, `set_cursor_select()` — replace `file.len()` / `file.is_empty()` with `self.data_len()` / `(self.data_len() == 0)`.

Update auto-columns calculation: replace `file.len()` references with `self.data_len()`.

Update `auto_detect_template()` to read from edit buffer:

```rust
    fn auto_detect_template(&mut self, path: &std::path::Path) {
        let bytes = match &self.edit_buffer {
            Some(buf) => buf.data(),
            None => {
                let Some(file) = &self.file else { return };
                file.as_bytes()
            }
        };
        // ... rest stays the same, using `bytes` ...
    }
```

Update `resolve_active_template()` similarly — use `self.edit_buffer.as_ref().map(|b| b.data())` or fall back to `file.as_bytes()`.

**Step 7: Build and verify**

Run: `cargo build --workspace`
Expected: Compiles cleanly.

Run: `cargo test --workspace`
Expected: All tests pass.

**Step 8: Commit**

```bash
git add crates/hexenly-app/src/app.rs crates/hexenly-app/src/panels/hex_view.rs crates/hexenly-app/src/panels/inspector.rs
git commit -m "refactor: route all byte reads through EditBuffer"
```

---

### Task 3: Drag and Drop

**Files:**
- Modify: `crates/hexenly-app/src/app.rs:851-860` (the `update()` method)

**Step 1: Add drag-and-drop handling in update()**

At the top of `update()`, after the `pending_open` handling block and before `self.handle_shortcuts(ctx)`, add:

```rust
        // Handle drag-and-drop
        ctx.input(|i| {
            if !i.raw.dropped_files.is_empty() {
                if let Some(path) = i.raw.dropped_files[0].path.clone() {
                    return Some(path);
                }
                None
            } else {
                None
            }
        }).map(|path| self.open_path(&path));
```

**Step 2: Add drop zone visual indicator**

In the `CentralPanel` section where "No file open" is shown, also show a drop indicator when files are hovered:

```rust
        // Show drop indicator overlay when files are being dragged over the window
        let is_dragging_files = ctx.input(|i| !i.raw.hovered_files.is_empty());
        if is_dragging_files {
            let screen_rect = ctx.screen_rect();
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("drop_overlay"),
            ));
            painter.rect_filled(
                screen_rect,
                0.0,
                Color32::from_rgba_unmultiplied(40, 80, 160, 60),
            );
            painter.text(
                screen_rect.center(),
                egui::Align2::CENTER_CENTER,
                "Drop file to open",
                egui::FontId::new(24.0, egui::FontFamily::Proportional),
                Color32::WHITE,
            );
        }
```

Place this after the `CentralPanel` block but before `self.show_notifications(ctx)`.

**Step 3: Build and test manually**

Run: `cargo run -p hexenly-app`
Test: Drag a file onto the window, verify it opens.

**Step 4: Commit**

```bash
git add crates/hexenly-app/src/app.rs
git commit -m "feat: drag-and-drop file opening with visual indicator"
```

---

### Task 4: Menu Bar

Replace the flat toolbar with an egui menu bar.

**Files:**
- Modify: `crates/hexenly-app/src/app.rs:705-753` (show_toolbar), `crates/hexenly-app/src/app.rs:864-869` (update toolbar panel)

**Step 1: Replace show_toolbar with show_menu_bar**

Replace the `show_toolbar` method entirely with a new `show_menu_bar` method:

```rust
    fn show_menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.add(egui::Button::new("Open").shortcut_text("Ctrl+O")).clicked() {
                    self.open_file_dialog();
                    ui.close_menu();
                }
                let has_file = self.edit_buffer.is_some();
                let is_dirty = self.edit_buffer.as_ref().is_some_and(|b| b.is_dirty());
                if ui.add_enabled(has_file && is_dirty, egui::Button::new("Save").shortcut_text("Ctrl+S")).clicked() {
                    self.save_file();
                    ui.close_menu();
                }
                if ui.add_enabled(has_file, egui::Button::new("Save As...").shortcut_text("Ctrl+Shift+S")).clicked() {
                    self.save_file_as();
                    ui.close_menu();
                }
            });

            ui.menu_button("Edit", |ui| {
                let can_undo = self.edit_buffer.as_ref().is_some_and(|b| b.can_undo());
                let can_redo = self.edit_buffer.as_ref().is_some_and(|b| b.can_redo());
                if ui.add_enabled(can_undo, egui::Button::new("Undo").shortcut_text("Ctrl+Z")).clicked() {
                    if let Some(buf) = &mut self.edit_buffer {
                        buf.undo();
                    }
                    ui.close_menu();
                }
                if ui.add_enabled(can_redo, egui::Button::new("Redo").shortcut_text("Ctrl+Shift+Z")).clicked() {
                    if let Some(buf) = &mut self.edit_buffer {
                        buf.redo();
                    }
                    ui.close_menu();
                }
                ui.separator();
                if ui.add(egui::Button::new("Find").shortcut_text("Ctrl+F")).clicked() {
                    self.show_search = !self.show_search;
                    self.focus_search = self.show_search;
                    self.show_goto = false;
                    ui.close_menu();
                }
                if ui.add(egui::Button::new("Go to Offset").shortcut_text("Ctrl+G")).clicked() {
                    self.show_goto = !self.show_goto;
                    self.show_search = false;
                    ui.close_menu();
                }
                ui.separator();
                let has_file = self.file.is_some();
                if ui.add_enabled(has_file, egui::Button::new("Select All").shortcut_text("Ctrl+A")).clicked() {
                    if let Some(len) = Some(self.data_len()).filter(|&l| l > 0) {
                        self.selection = Some(Selection::new(0, len - 1));
                        self.selection_anchor = Some(0);
                    }
                    ui.close_menu();
                }
            });

            ui.menu_button("View", |ui| {
                ui.menu_button("Columns", |ui| {
                    if ui.selectable_label(self.auto_columns, "Auto").clicked() {
                        self.auto_columns = true;
                        ui.close_menu();
                    }
                    for &n in &[8, 16, 24, 32, 48] {
                        if ui.selectable_label(!self.auto_columns && self.columns == n, format!("{n}")).clicked() {
                            self.columns = n;
                            self.auto_columns = false;
                            ui.close_menu();
                        }
                    }
                });
                ui.menu_button("Encoding", |ui| {
                    if ui.selectable_label(self.text_encoding == TextEncoding::Ascii, "ASCII").clicked() {
                        self.text_encoding = TextEncoding::Ascii;
                        ui.close_menu();
                    }
                    if ui.selectable_label(self.text_encoding == TextEncoding::Utf8, "UTF-8").clicked() {
                        self.text_encoding = TextEncoding::Utf8;
                        ui.close_menu();
                    }
                });
                ui.separator();
                if ui.selectable_label(self.show_ascii_pane, "ASCII Pane").clicked() {
                    self.show_ascii_pane = !self.show_ascii_pane;
                }
                if ui.selectable_label(self.show_inspector, "Inspector").clicked() {
                    self.show_inspector = !self.show_inspector;
                }
                if ui.selectable_label(self.show_template_browser, "Templates").clicked() {
                    self.show_template_browser = !self.show_template_browser;
                }
                if ui.selectable_label(self.show_structure_panel, "Structure").clicked() {
                    self.show_structure_panel = !self.show_structure_panel;
                }
                if ui.selectable_label(self.show_bookmarks, "Bookmarks").clicked() {
                    self.show_bookmarks = !self.show_bookmarks;
                }
            });
        });
    }
```

**Step 2: Update the TopBottomPanel in update() to use menu bar**

Replace the toolbar panel in `update()`:

```rust
        // Top menu bar
        TopBottomPanel::top("menubar").show(ctx, |ui| {
            self.show_menu_bar(ui);
            self.show_search_bar(ui);
            self.show_goto_bar(ui);
        });
```

**Step 3: Add save_file and save_file_as stub methods**

```rust
    fn save_file(&mut self) {
        if let Some(buf) = &mut self.edit_buffer {
            if let Err(e) = buf.save() {
                self.notifications.push(Notification {
                    message: format!("Save failed: {e}"),
                    level: NotificationLevel::Error,
                    created: Instant::now(),
                });
            }
        }
    }

    fn save_file_as(&mut self) {
        let Some(buf) = &mut self.edit_buffer else { return };
        let mut dialog = rfd::FileDialog::new();
        if let Some(path) = buf.file_path() {
            if let Some(dir) = path.parent() {
                dialog = dialog.set_directory(dir);
            }
            if let Some(name) = path.file_name() {
                dialog = dialog.set_file_name(name.to_string_lossy().to_string());
            }
        }
        if let Some(path) = dialog.save_file() {
            if let Err(e) = buf.save_as(&path) {
                self.notifications.push(Notification {
                    message: format!("Save failed: {e}"),
                    level: NotificationLevel::Error,
                    created: Instant::now(),
                });
            }
        }
    }
```

**Step 4: Add keyboard shortcuts for save, undo/redo, select-all**

In `handle_shortcuts()`, inside `ctx.input_mut(|i| { ... })`, add:

```rust
            let save = i.consume_key(egui::Modifiers::COMMAND, Key::S);
            let save_as = i.consume_key(
                egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT),
                Key::S,
            );
            let undo = i.consume_key(egui::Modifiers::COMMAND, Key::Z);
            let redo = i.consume_key(
                egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT),
                Key::Z,
            );
            let select_all = i.consume_key(egui::Modifiers::COMMAND, Key::A);
            let insert_key = i.consume_key(egui::Modifiers::NONE, Key::Insert);
```

Add these to the destructured return tuple and handle them after the existing shortcut handling:

```rust
        if save {
            self.save_file();
        }
        if save_as {
            self.save_file_as();
        }
        if undo {
            if let Some(buf) = &mut self.edit_buffer {
                buf.undo();
            }
        }
        if redo {
            if let Some(buf) = &mut self.edit_buffer {
                buf.redo();
            }
        }
        if select_all && self.data_len() > 0 {
            let max = self.data_len() - 1;
            self.selection = Some(Selection::new(0, max));
            self.selection_anchor = Some(0);
        }
        if insert_key {
            if let Some(buf) = &mut self.edit_buffer {
                buf.toggle_mode();
            }
        }
```

**Step 5: Build and verify**

Run: `cargo build -p hexenly-app`
Expected: Compiles.

**Step 6: Commit**

```bash
git add crates/hexenly-app/src/app.rs
git commit -m "feat: menu bar with File/Edit/View menus, save, undo/redo shortcuts"
```

---

### Task 5: Status Bar Updates

Add dirty indicator and edit mode to the existing status bar.

**Files:**
- Modify: `crates/hexenly-app/src/app.rs:812-847` (show_status_bar)

**Step 1: Update show_status_bar**

Replace `show_status_bar` to include dirty indicator and edit mode. Note: change `&self` to `&mut self` since clicking the mode indicator mutates state.

```rust
    fn show_status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if let Some(file) = &self.file {
                let name = file
                    .path()
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unknown".into());
                let is_dirty = self.edit_buffer.as_ref().is_some_and(|b| b.is_dirty());
                let dirty_marker = if is_dirty { " *" } else { "" };
                ui.label(RichText::new(format!("{name}{dirty_marker}")).strong());
                ui.label(RichText::new(format_size(self.data_len())).weak());

                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    // Edit mode indicator (clickable)
                    if let Some(buf) = &self.edit_buffer {
                        let mode_text = match buf.mode() {
                            EditMode::Overwrite => "OVR",
                            EditMode::Insert => "INS",
                        };
                        let mode_label = ui.add(
                            egui::Button::new(RichText::new(mode_text).monospace())
                                .frame(false),
                        );
                        if mode_label.clicked() {
                            // Need to re-borrow mutably
                        }
                    }

                    if let Some(resolved) = &self.resolved_template {
                        ui.label(&resolved.name);
                        ui.label(RichText::new("Template:").weak());
                        ui.separator();
                    }
                    if let Some(sel) = &self.selection {
                        if ui.small_button("Copy Text").clicked() {
                            self.copy_selection_text(ui.ctx());
                        }
                        if ui.small_button("Copy Hex").clicked() {
                            self.copy_selection_hex(ui.ctx());
                        }
                        ui.label(format!("{} bytes", sel.len()));
                        ui.label(RichText::new("Selected:").weak());
                        ui.separator();
                    }
                    ui.label(format!("0x{:08X} ({})", self.cursor_offset, self.cursor_offset));
                    ui.label(RichText::new("Offset:").weak());
                });
            } else {
                ui.label("No file open \u{2014} Ctrl+O to open");
            }
        });
    }
```

Note: The edit mode button click needs special handling since `self.edit_buffer` is borrowed. Track the click as a flag and mutate after:

```rust
        // At the end of the method, outside the ui.horizontal closure, handle mode toggle.
        // Actually, since show_status_bar takes &mut self, restructure to:
```

The implementation will need to track `mode_clicked` as a bool, then toggle after the borrow ends. The implementer should use the pattern:

```rust
    fn show_status_bar(&mut self, ui: &mut egui::Ui) {
        let mut toggle_mode = false;
        ui.horizontal(|ui| {
            // ... all the rendering ...
            // For the mode button:
            if let Some(buf) = &self.edit_buffer {
                let mode_text = match buf.mode() {
                    EditMode::Overwrite => "OVR",
                    EditMode::Insert => "INS",
                };
                if ui.add(egui::Button::new(RichText::new(mode_text).monospace()).frame(false)).clicked() {
                    toggle_mode = true;
                }
            }
            // ...
        });
        if toggle_mode {
            if let Some(buf) = &mut self.edit_buffer {
                buf.toggle_mode();
            }
        }
    }
```

**Step 2: Build and verify**

Run: `cargo build -p hexenly-app`
Expected: Compiles.

**Step 3: Commit**

```bash
git add crates/hexenly-app/src/app.rs
git commit -m "feat: status bar with dirty indicator and edit mode toggle"
```

---

### Task 6: Hex Pane Editing (Typing Hex Digits)

Add keyboard input handling for editing bytes in the hex pane.

**Files:**
- Modify: `crates/hexenly-app/src/app.rs` (add nibble state, handle hex keystrokes)

**Step 1: Add nibble editing state to HexenlyApp**

Add fields to `HexenlyApp` struct:

```rust
    /// True = waiting for high nibble (first digit), false = waiting for low nibble (second digit).
    nibble_high: bool,
    /// Which pane has edit focus for keyboard input.
    edit_focus: HexPane,
```

Initialize in `new()`:

```rust
    nibble_high: true,
    edit_focus: HexPane::Hex,
```

**Step 2: Reset nibble state on cursor movement**

In `move_cursor()`, `set_cursor_abs()`, `move_cursor_select()`, and `set_cursor_select()`, add:

```rust
    self.nibble_high = true;
```

Also reset in `open_path()`.

**Step 3: Track edit focus from hex view clicks**

When the hex view returns `SetCursor` or `Select`, check which pane was clicked. Update `HexViewAction::SetCursor` to include the pane:

In `crates/hexenly-app/src/panels/hex_view.rs`, change:

```rust
pub enum HexViewAction {
    SetCursor(usize, HexPane),
    Select { start: usize, end: usize, pane: HexPane },
}
```

Update the click handler in hex_view to pass the pane:

```rust
            if response.clicked()
                && let Some(pos) = response.interact_pointer_pos()
                && let Some((offset, pane)) = hit(pos)
            {
                action = Some(HexViewAction::SetCursor(offset, pane));
            }
```

Update `app.rs` to handle the new variant:

```rust
                    Some(HexViewAction::SetCursor(off, pane)) if off < data_len => {
                        self.cursor_offset = off;
                        self.selection = None;
                        self.selection_anchor = None;
                        self.edit_focus = pane;
                        self.nibble_high = true;
                    }
                    Some(HexViewAction::Select { start, end, pane }) => {
                        // ... existing logic ...
                        self.edit_focus = pane;
                        self.nibble_high = true;
                    }
```

**Step 4: Handle hex digit keystrokes**

Add a new method `handle_edit_input` in `app.rs`, called from `update()` after `handle_shortcuts()`:

```rust
    fn handle_edit_input(&mut self, ctx: &Context) {
        let Some(buf) = &self.edit_buffer else { return };
        if buf.is_empty() {
            return;
        }

        // Collect text events (typed characters)
        let typed_chars: Vec<char> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| {
                    if let egui::Event::Text(s) = e {
                        s.chars().next()
                    } else {
                        None
                    }
                })
                .collect()
        });

        for ch in typed_chars {
            match self.edit_focus {
                HexPane::Hex => self.handle_hex_input(ch),
                HexPane::Ascii => self.handle_ascii_input(ch),
            }
        }
    }

    fn handle_hex_input(&mut self, ch: char) {
        let Some(digit) = ch.to_digit(16) else { return };
        let digit = digit as u8;
        let Some(buf) = &mut self.edit_buffer else { return };

        let offset = self.cursor_offset;

        if buf.mode() == EditMode::Insert && self.nibble_high {
            // Insert a new 0x00 byte, then we'll set the high nibble
            buf.insert_byte(offset, 0x00);
        }

        // Now overwrite the nibble
        if offset >= buf.len() {
            return;
        }
        let current = buf.data()[offset];
        let new_byte = if self.nibble_high {
            (digit << 4) | (current & 0x0F)
        } else {
            (current & 0xF0) | digit
        };
        buf.overwrite_byte(offset, new_byte);

        if self.nibble_high {
            self.nibble_high = false;
        } else {
            self.nibble_high = true;
            // Advance cursor
            let max = buf.len().saturating_sub(1);
            if self.cursor_offset < max {
                self.cursor_offset += 1;
            }
        }
    }

    fn handle_ascii_input(&mut self, ch: char) {
        if !ch.is_ascii() || ch.is_ascii_control() {
            return;
        }
        let Some(buf) = &mut self.edit_buffer else { return };
        let offset = self.cursor_offset;

        if buf.mode() == EditMode::Insert {
            buf.insert_byte(offset, ch as u8);
        } else {
            buf.overwrite_byte(offset, ch as u8);
        }

        // Advance cursor
        let max = buf.len().saturating_sub(1);
        if self.cursor_offset < max {
            self.cursor_offset += 1;
        }
        self.nibble_high = true;
    }
```

**Step 5: Handle Delete/Backspace**

Add to `handle_shortcuts()` — consume Delete and Backspace keys:

```rust
            let delete = i.consume_key(egui::Modifiers::NONE, Key::Delete);
            let backspace = i.consume_key(egui::Modifiers::NONE, Key::Backspace);
```

Then handle them:

```rust
        // Delete / Backspace
        if (delete || backspace) && self.edit_buffer.is_some() {
            self.handle_delete(delete);
        }
```

Add the handler method:

```rust
    fn handle_delete(&mut self, is_forward: bool) {
        let Some(buf) = &mut self.edit_buffer else { return };
        if buf.is_empty() {
            return;
        }

        // If there's a selection, operate on the range
        if let Some(sel) = self.selection.take() {
            if buf.mode() == EditMode::Insert {
                buf.delete_range(sel.start, sel.end);
                self.cursor_offset = sel.start.min(buf.len().saturating_sub(1));
            } else {
                // Overwrite mode: zero the selected bytes
                let zeros = vec![0u8; sel.len()];
                buf.overwrite_range(sel.start, &zeros);
                self.cursor_offset = sel.start;
            }
            self.selection_anchor = None;
            self.nibble_high = true;
            return;
        }

        // No selection — single byte operation
        if buf.mode() == EditMode::Insert {
            if is_forward {
                // Delete key: remove byte at cursor
                buf.delete_byte(self.cursor_offset);
                if self.cursor_offset >= buf.len() && buf.len() > 0 {
                    self.cursor_offset = buf.len() - 1;
                }
            } else {
                // Backspace: remove byte before cursor
                if self.cursor_offset > 0 {
                    self.cursor_offset -= 1;
                    buf.delete_byte(self.cursor_offset);
                }
            }
        } else {
            // Overwrite mode: zero the byte
            buf.overwrite_byte(self.cursor_offset, 0x00);
        }
        self.nibble_high = true;
    }
```

**Step 6: Call handle_edit_input from update()**

In the `update()` method, after `self.handle_shortcuts(ctx)`:

```rust
        self.handle_edit_input(ctx);
```

**Step 7: Consume text events to prevent them leaking to other widgets**

In `handle_edit_input`, after processing typed chars, consume the text events so they don't propagate to search/goto fields. This needs care — only consume when search/goto bars are not focused. The simplest approach: only call `handle_edit_input` when `!self.show_search && !self.show_goto`.

```rust
        // Only process edit input when not in a text input mode
        if !self.show_search && !self.show_goto {
            self.handle_edit_input(ctx);
        }
```

**Step 8: Build and test manually**

Run: `cargo run -p hexenly-app -- <some test file>`
Test: Click on a byte in hex pane, type hex digits. Verify bytes change. Try insert mode. Try ASCII pane typing.

**Step 9: Commit**

```bash
git add crates/hexenly-app/src/app.rs crates/hexenly-app/src/panels/hex_view.rs
git commit -m "feat: hex and ASCII pane editing with insert/overwrite modes"
```

---

### Task 7: Visual Feedback for Edit Mode

Add visual cues in the hex view for the current nibble position and modified bytes.

**Files:**
- Modify: `crates/hexenly-app/src/panels/hex_view.rs`
- Modify: `crates/hexenly-app/src/theme.rs`
- Modify: `crates/hexenly-app/src/app.rs`

**Step 1: Add nibble cursor visual to hex_view**

Update `hex_view::show()` signature to accept nibble state and edit mode:

```rust
pub fn show(
    ui: &mut Ui,
    data: &[u8],
    total_len: usize,
    columns: usize,
    cursor: usize,
    selection: Option<&Selection>,
    search_matches: &[usize],
    show_ascii: bool,
    state: &mut HexViewState,
    template_overlay: Option<&ResolvedTemplate>,
    nibble_high: bool,
    edit_focus: HexPane,
) -> Option<HexViewAction> {
```

In the cursor rendering section, when `is_cursor` is true in the hex pane, draw the nibble indicator:

```rust
                    if is_cursor {
                        painter.rect_filled(hex_rect, 0.0, HexColors::CURSOR_BG);
                        painter.rect_stroke(hex_rect, 0.0, Stroke::new(1.0, HexColors::CURSOR_BORDER), StrokeKind::Inside);

                        // Draw nibble cursor underline
                        if edit_focus == HexPane::Hex {
                            let nibble_x = if nibble_high {
                                hex_x_start + col as f32 * hex_col_width
                            } else {
                                hex_x_start + col as f32 * hex_col_width + char_width
                            };
                            let underline_y = y + row_height;
                            painter.line_segment(
                                [
                                    Pos2::new(nibble_x, underline_y),
                                    Pos2::new(nibble_x + char_width, underline_y),
                                ],
                                Stroke::new(2.0, HexColors::CURSOR_BORDER),
                            );
                        }
                    }
```

**Step 2: Update call site in app.rs**

Pass `self.nibble_high` and `self.edit_focus` to `hex_view::show()`.

**Step 3: Add MODIFIED_BYTE color to theme**

In `crates/hexenly-app/src/theme.rs`, add to `HexColors`:

```rust
    pub const MODIFIED_BYTE: Color32 = Color32::from_rgb(255, 200, 80);
```

This color will be used later if we track which bytes are modified. For now, just define it.

**Step 4: Build and verify**

Run: `cargo build -p hexenly-app`

**Step 5: Commit**

```bash
git add crates/hexenly-app/src/app.rs crates/hexenly-app/src/panels/hex_view.rs crates/hexenly-app/src/theme.rs
git commit -m "feat: nibble cursor indicator and modified byte color"
```

---

### Task 8: Final Polish and Edge Cases

Handle remaining edge cases and wire everything together.

**Files:**
- Modify: `crates/hexenly-app/src/app.rs`

**Step 1: Handle selection + typing (delete selection then type)**

In `handle_hex_input` and `handle_ascii_input`, before processing the keystroke, check if there's a selection and clear it first:

```rust
    fn handle_hex_input(&mut self, ch: char) {
        let Some(digit) = ch.to_digit(16) else { return };
        let digit = digit as u8;
        let Some(buf) = &mut self.edit_buffer else { return };

        // If selection exists, delete it first (insert mode) or start overwriting from selection start (overwrite mode)
        if let Some(sel) = self.selection.take() {
            if buf.mode() == EditMode::Insert {
                buf.delete_range(sel.start, sel.end);
            }
            self.cursor_offset = sel.start.min(buf.len().saturating_sub(1));
            self.selection_anchor = None;
            self.nibble_high = true;
        }

        let offset = self.cursor_offset;
        // ... rest of existing logic ...
    }
```

Same pattern for `handle_ascii_input`.

**Step 2: Update window title to show dirty state**

In `update()`, after the theme application:

```rust
        // Update window title with dirty indicator
        let title = if let Some(file) = &self.file {
            let name = file.path().file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".into());
            let dirty = if self.edit_buffer.as_ref().is_some_and(|b| b.is_dirty()) { " *" } else { "" };
            format!("{name}{dirty} - Hexenly")
        } else {
            "Hexenly".to_string()
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
```

**Step 3: Warn before closing with unsaved changes**

Add an unsaved-changes check. In `update()`, check for close request:

```rust
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.edit_buffer.as_ref().is_some_and(|b| b.is_dirty()) {
                // Prevent close, show notification
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.notifications.push(Notification {
                    message: "Unsaved changes! Save first or press Ctrl+Q to force quit.".into(),
                    level: NotificationLevel::Warning,
                    created: Instant::now(),
                });
            }
        }
```

For simplicity, add Ctrl+Q as a force-quit shortcut that closes without saving. Add to handle_shortcuts:

```rust
            let force_quit = i.consume_key(egui::Modifiers::COMMAND, Key::Q);
```

Handle it:

```rust
        if force_quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
```

Note: To allow force-quit to bypass the unsaved check, track a `force_closing: bool` field on HexenlyApp. Set it to true on Ctrl+Q, and in the close_requested handler, skip the cancel if `force_closing` is true.

**Step 4: Update "no file open" message**

In the CentralPanel no-file section, the message already says "Drop a file or press Ctrl+O to open" — this is correct for drag-and-drop support.

**Step 5: Build full workspace and run all tests**

Run: `cargo build --workspace && cargo test --workspace && cargo clippy --workspace`
Expected: All pass.

**Step 6: Commit**

```bash
git add crates/hexenly-app/src/app.rs
git commit -m "feat: selection+edit, window title dirty state, unsaved changes warning"
```

---

## Summary of Tasks

| Task | Description | Key Files |
|------|-------------|-----------|
| 1 | EditBuffer data model with undo/redo + save | `hexenly-core/src/edit_buffer.rs`, `lib.rs` |
| 2 | Integrate EditBuffer into app | `app.rs`, `hex_view.rs`, `inspector.rs` |
| 3 | Drag and drop file opening | `app.rs` |
| 4 | Menu bar (File/Edit/View) | `app.rs` |
| 5 | Status bar (dirty + mode indicator) | `app.rs` |
| 6 | Hex/ASCII pane editing + delete/backspace | `app.rs`, `hex_view.rs` |
| 7 | Nibble cursor visual | `hex_view.rs`, `theme.rs` |
| 8 | Polish: selection+edit, title, close warning | `app.rs` |
