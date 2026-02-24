# Drag & Drop, File Editing, and Menu Reorganization

## Summary

Add drag-and-drop file opening, full file editing with insert/overwrite modes and undo/redo, and reorganize the toolbar into a menu bar + status bar.

## 1. Drag and Drop

Use eframe's `raw_input.dropped_files` to detect file drops. Open the first dropped file via the existing `open_path()` flow. Show a visual drop indicator when files are hovered over the window using `hovered_files`.

No architectural changes needed.

## 2. Edit Data Model

### EditBuffer (hexenly-core)

```rust
pub struct EditBuffer {
    original: Vec<u8>,      // snapshot from file open
    data: Vec<u8>,          // current working copy
    undo_stack: Vec<EditOp>,
    redo_stack: Vec<EditOp>,
    dirty: bool,
    mode: EditMode,         // Insert | Overwrite
}

pub enum EditMode {
    Insert,
    Overwrite,
}

pub enum EditOp {
    Overwrite { offset: usize, old_bytes: Vec<u8>, new_bytes: Vec<u8> },
    Insert { offset: usize, bytes: Vec<u8> },
    Delete { offset: usize, bytes: Vec<u8> },
}
```

### File open flow

1. `HexFile::open()` mmaps the file (for initial read)
2. `EditBuffer::from_hex_file(&hex_file)` copies bytes into Vec
3. For files >100MB, show a warning dialog before copying
4. All UI reads go through `EditBuffer.data` instead of mmap
5. `HexFile` retained for file path and metadata

### Save flow

- Write `edit_buffer.data` to a temp file in the same directory
- Rename temp file over original (atomic on most filesystems)
- Reset dirty flag, clear undo stack, update `original` to match `data`
- Save As: prompt for path via `rfd::FileDialog`, then same write flow

### Undo/Redo

- Each edit pushes an `EditOp` onto the undo stack
- Undo reverses the top op, pushes it onto redo stack
- Any new edit clears the redo stack
- `Ctrl+Z` undo, `Ctrl+Shift+Z` redo

## 3. Edit Interaction Model

### Two modes: Overwrite (default) and Insert

Toggled via Insert key. Indicator in status bar.

### Hex pane editing

- First hex digit (0-9, a-f) enters high nibble, cursor enters half-byte state
- Second digit completes the byte, cursor advances
- Overwrite: replaces byte at cursor
- Insert: inserts new 0x00 byte at cursor, then nibble-edits it

### ASCII pane editing

- Typing printable character overwrites/inserts full byte at cursor
- Overwrite: replaces byte, advances cursor
- Insert: inserts byte, advances cursor

### Delete/Backspace

- Overwrite mode: zeroes the byte at cursor
- Insert mode: Delete removes byte at cursor, Backspace removes byte before cursor

### Selection + edit

- Typing with selection: delete selected bytes first (insert) or overwrite from selection start (overwrite), then enter typed value
- Delete/Backspace with selection: remove or zero the selected range

### Nibble state

Track `nibble_high: bool` on app state. Reset to true on cursor movement, selection change, or mode switch.

## 4. Menu Bar + Status Bar

### Menu bar replaces flat toolbar

```
File           Edit              View
-----          -----             -----
Open  Ctrl+O   Undo  Ctrl+Z     Columns >  Auto, 8, 16, 24, 32, 48
Save  Ctrl+S   Redo  Ctrl+Shift+Z  Encoding > ASCII, UTF-8
Save As        -----             ---------
  Ctrl+Shift+S Find  Ctrl+F     ASCII Pane
-----          Go to   Ctrl+G   Inspector
Quit           -----             Templates
               Select All        Structure
                 Ctrl+A          Bookmarks
```

### Status bar (new, bottom of window)

- Left: file path, file size, dirty indicator
- Center/right: current offset (hex + decimal), selection length
- Right: edit mode INS/OVR (clickable to toggle)

### What moves

- Column selector radio buttons -> View > Columns submenu
- Text encoding toggles -> View > Encoding submenu
- Panel toggle buttons -> View menu items
- Open button -> File menu
- Search/goto bars remain inline below menu bar (transient input fields)

## Design Decisions

- **In-memory Vec<u8>** for editing simplicity; warn above 100MB
- **Full undo/redo** via command pattern from day one
- **Atomic save** via temp file + rename
- **Status bar** for mode indicator, offset display, dirty state
- **Menu bar** to accommodate growing feature set cleanly
