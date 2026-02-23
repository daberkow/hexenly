# Phase 4b: Keyboard Navigation, Bookmarks, CI & Cross-Platform Builds

## Summary

Three independent features: full keyboard navigation for the hex view, persistent bookmarks with a sidebar panel, and GitHub Actions CI with cross-platform release builds.

## 1. Keyboard Navigation

**Keys:**
- Arrow Left/Right — move cursor ±1 byte
- Arrow Up/Down — move cursor ±1 row (±columns bytes)
- Page Up/Down — move cursor ±1 screenful (±visible_rows × columns)
- Home/End — start/end of current row
- Ctrl+Home/Ctrl+End — start/end of file
- Shift + any of the above — extend selection from current cursor

**Implementation in `app.rs`:**
- All handled in `handle_shortcuts()` via `consume_key`
- Cursor clamped to `0..file.len()-1`
- After each move, auto-scroll via `scroll_to_cursor()`
- Shift variants: if no selection exists, anchor at current cursor then extend; if selection exists, extend the end

No new files — additions to `app.rs` only.

## 2. Bookmarks

**Data model (`selection.rs`):**
- Extend existing `Bookmark` struct: `{ name: String, offset: usize, note: String }`
- `Vec<Bookmark>` stored on `HexenlyApp`

**UI — Sidebar panel (`panels/bookmarks.rs`):**
- Toggleable via toolbar "Bookmarks" button
- Lists bookmarks sorted by offset
- Each row: name, hex offset, note as tooltip or secondary line
- Click row to jump to offset
- Delete button (X) per row
- "Add" button at top — bookmarks current cursor, inline text field for name
- Note editable inline in the panel

**Shortcuts:**
- Ctrl+B — add bookmark at current cursor offset
- Ctrl+Shift+B / Ctrl+Shift+N — jump to previous/next bookmark

**Persistence — JSON sidecar:**
- File `/path/to/data.bin` → sidecar `/path/to/.data.bin.hexenly.json`
- Format: `{ "bookmarks": [{ "name": "Header", "offset": 0, "note": "" }, ...] }`
- Auto-save on add/delete/edit
- Auto-load on file open
- Dot-prefixed hidden file

## 3. CI & Cross-Platform Builds

**`ci.yml` — every push and PR:**
- Matrix: ubuntu-latest, macos-latest, windows-latest
- Steps: `cargo build --workspace`, `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`

**`release.yml` — on version tags (`v*`):**
- Same OS matrix
- `cargo build --release -p hexenly-app`
- Rename binary per platform: `hexenly-linux-x86_64`, `hexenly-macos-x86_64`, `hexenly-windows-x86_64.exe`
- Upload as GitHub Release assets via `softprops/action-gh-release`

No installers — raw executables only.

## Verification

- All keyboard shortcuts work: arrow, page, home/end, shift+selection
- Bookmarks: add, name, note, jump, delete, persist across reopen
- CI: push triggers build on all 3 platforms; tag triggers release with binaries
