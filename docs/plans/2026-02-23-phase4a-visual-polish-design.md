# Phase 4a: Visual Polish & Feedback

## Summary

Targeted fixes for visual feedback, alignment, and code cleanliness. Six independent work items that each improve the UI without architectural changes.

## 1. Search Feedback

**Problem:** Invalid hex search input silently does nothing.

**Changes in `app.rs`:**
- Add `search_error: Option<String>` to `HexenlyApp`
- In `do_search()`, when `from_hex_string` returns `None`, set `search_error = Some("Invalid hex pattern".into())`
- In `show_search_bar()`, render the error as a red-tinted label next to the match counter
- Clear the error when `search_input` changes or `search_hex_mode` is toggled

## 2. Structure Panel Alignment

**Problem:** Field name column uses proportional font; offset/size/value use monospace. Grid rows look uneven.

**Changes in `structure.rs`:**
- Apply `monospace_font()` to field name labels inside `selectable_label`
- Tighten grid horizontal spacing from `[8.0, 2.0]` to `[6.0, 2.0]`

## 3. Status Bar Hierarchy

**Problem:** Single horizontal row crams all info together, wraps badly, no visual weight differentiation.

**Changes in `app.rs` `show_status_bar()`:**
- Left-aligned group: filename + file size (static context)
- Right-aligned group: cursor offset, selection count, template name (dynamic state)
- Dim the labels ("Offset:", "Template:") using `Color32::GRAY`, keep values at full brightness
- Use `ui.with_layout(Layout::right_to_left(...))` for the right group

## 4. Cursor Prominence

**Problem:** Cursor background blends into template overlays and selection colors.

**Changes in `hex_view.rs` and `theme.rs`:**
- Brighten `CURSOR_BG` from `#506496` to `#6080C0`
- Add a 1px rect stroke (border) around the cursor cell in addition to the fill
- Apply the same border treatment to the ASCII pane cursor

## 5. Template Load Error Notifications

**Problem:** Template parse failures are logged via `tracing::error!` but invisible to users.

**New types in `app.rs`:**
- `Notification { message: String, level: NotificationLevel, created: Instant }`
- `NotificationLevel { Error, Warning }`
- `notifications: Vec<Notification>` field on `HexenlyApp`

**Rendering:**
- Small overlay in the top-right corner of the central panel
- Auto-dismiss after 5 seconds based on elapsed time from `created`
- No animation, just alpha fade in the last second

**Populated from:**
- Template load errors in `HexenlyApp::new()`
- Template resolve warnings in `resolve_active_template()`

## 6. Clippy Cleanup

Fix all 16 warnings across the workspace:

**hexenly-core (7):**
- Redundant closures: `map_err(|e| HexError::Io(e))` to `map_err(HexError::Io)` (3x)
- Use `div_ceil` instead of manual computation
- Use `Iterator::find` instead of manual loop (2x)
- Add `is_empty()` to `Selection`

**hexenly-templates (2):**
- Collapsible if statements in engine.rs and validator.rs

**hexenly-app (7):**
- Collapsible if blocks in app.rs and hex_view.rs
- Derivable Default for HexViewState
- Suppress 9-param warning on `hex_view::show()` with `#[allow(clippy::too_many_arguments)]`

## Verification

- `cargo clippy --workspace` should be clean after step 6
- `cargo test --workspace` must pass after every step
- Manual visual check: open a ZIP file and confirm structure panel alignment, cursor visibility, search error feedback
