# Phase 4a: Visual Polish & Feedback — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix visual feedback, alignment, cursor prominence, and code cleanliness across the Hexenly UI.

**Architecture:** Six independent changes to the hexenly-app and hexenly-core crates. No new crates or modules. Each task is self-contained and can be committed separately.

**Tech Stack:** Rust, egui 0.33, eframe

---

### Task 1: Clippy Cleanup

Start with this — fixes code quality issues that affect all subsequent work.

**Files:**
- Modify: `crates/hexenly-core/src/file.rs:16-25,58`
- Modify: `crates/hexenly-core/src/search.rs:46-51,68-73`
- Modify: `crates/hexenly-core/src/selection.rs:28-31`
- Modify: `crates/hexenly-templates/src/engine.rs:155-159`
- Modify: `crates/hexenly-templates/src/validator.rs:74-83,264-273`
- Modify: `crates/hexenly-app/src/app.rs:266-274,456-471,526-530`
- Modify: `crates/hexenly-app/src/panels/hex_view.rs:7-18,20,203-207`

**Step 1: Fix hexenly-core redundant closures and div_ceil**

In `crates/hexenly-core/src/file.rs`:

```rust
// Line 16: replace |e| HexError::Io(e) with HexError::Io
let file = File::open(&path).map_err(HexError::Io)?;
// Line 17:
let metadata = file.metadata().map_err(HexError::Io)?;
// Line 25:
let mmap = unsafe { Mmap::map(&file) }.map_err(HexError::Io)?;
// Line 58: replace manual div_ceil
self.mmap.len().div_ceil(columns)
```

**Step 2: Fix hexenly-core Iterator::find**

In `crates/hexenly-core/src/search.rs`:

```rust
// Lines 46-51: replace manual loop in find_next wrap-around
(0..=wrap_end).find(|&i| &data[i..i + needle.len()] == needle)

// Lines 68-73: replace manual loop in find_prev wrap-around
(start..=max_pos).rev().find(|&i| &data[i..i + needle.len()] == needle)
```

**Step 3: Add is_empty to Selection**

In `crates/hexenly-core/src/selection.rs`, add after the `len()` method:

```rust
pub fn is_empty(&self) -> bool {
    false // A selection always contains at least one byte (start == end means 1 byte selected)
}
```

Note: `Selection::new` normalizes so `start <= end`, and `len()` returns `end - start + 1` (minimum 1). A selection is never truly empty, but clippy requires the method to exist alongside `len()`.

**Step 4: Fix hexenly-templates collapsible ifs**

In `crates/hexenly-templates/src/engine.rs` lines 155-159:

```rust
if let Ok(bit_idx) = key.parse::<u8>()
    && bit_idx < 64 && (val >> bit_idx) & 1 == 1
{
    names.push((bit_idx, name.as_str()));
}
```

In `crates/hexenly-templates/src/validator.rs` lines 74-83:

```rust
if let Some(color) = &region.color
    && TemplateColor::from_hex(color).is_none()
{
    warnings.push(ValidationWarning {
        message: format!(
            "invalid color '{}' in region '{}' (expected #RRGGBB)",
            color, region.id
        ),
    });
}
```

In `crates/hexenly-templates/src/validator.rs` lines 264-273:

```rust
if let Some(target) = &field.size_target
    && !all_ids.contains(target.as_str())
{
    warnings.push(ValidationWarning {
        message: format!(
            "field '{}': size_target references unknown ID '{}'",
            field.id, target
        ),
    });
}
```

**Step 5: Fix hexenly-app collapsible ifs and derivable Default**

In `crates/hexenly-app/src/panels/hex_view.rs`:
- Replace manual Default impl (lines 12-18) with `#[derive(Default)]` on HexViewState (line 7)
- Add `#[allow(clippy::too_many_arguments)]` above `pub fn show(` (line 20)
- Collapse if at lines 203-207:

```rust
if response.clicked()
    && let Some(pos) = response.interact_pointer_pos()
{
    action = hit_test(pos, origin, offset_width, hex_col_width, columns, line_height, &row_range);
}
```

In `crates/hexenly-app/src/app.rs`:

Lines 266-274 (goto_offset):
```rust
if let Some(off) = offset
    && let Some(file) = &self.file
    && off < file.len()
{
    self.cursor_offset = off;
    self.selection = None;
    self.scroll_to_cursor();
}
```

Lines 456-471 (structure panel):
```rust
if self.show_structure_panel
    && let Some(resolved) = &self.resolved_template
{
    let resolved_clone = resolved.clone();
    let cursor = self.cursor_offset;
    TopBottomPanel::bottom("structure")
        .default_height(200.0)
        .resizable(true)
        .show(ctx, |ui| {
            let action = structure::show(ui, &resolved_clone, cursor);
            if let Some(StructureAction::GoToOffset(off)) = action {
                self.cursor_offset = off;
                self.scroll_to_cursor();
            }
        });
}
```

Lines 526-530 (hex view cursor):
```rust
if let Some(HexViewAction::SetCursor(off)) = action
    && off < file.len()
{
    self.cursor_offset = off;
}
```

**Step 6: Verify**

Run: `cargo clippy --workspace 2>&1`
Expected: Only `Finished` line, zero warnings.

Run: `cargo test --workspace 2>&1`
Expected: All tests pass.

**Step 7: Commit**

```bash
git add -A && git commit -m "fix: resolve all clippy warnings across workspace"
```

---

### Task 2: Search Error Feedback

**Files:**
- Modify: `crates/hexenly-app/src/app.rs:19-53,210-229,347-379`

**Step 1: Add search_error field to HexenlyApp**

In `crates/hexenly-app/src/app.rs`, add to the struct after `search_match_idx` (line 38):

```rust
search_error: Option<String>,
```

And in `Self { ... }` initializer (after line 110):

```rust
search_error: None,
```

**Step 2: Set error in do_search()**

Replace `do_search()` (lines 210-229):

```rust
fn do_search(&mut self) {
    self.search_error = None;
    let Some(file) = &self.file else { return };
    let pattern = if self.search_hex_mode {
        match SearchPattern::from_hex_string(&self.search_input) {
            Some(p) => p,
            None => {
                self.search_error = Some("Invalid hex pattern".into());
                return;
            }
        }
    } else {
        SearchPattern::from_text(&self.search_input)
    };

    self.search_matches = find_all(file.as_bytes(), &pattern, 10_000);
    if let Some(&first) = self.search_matches.first() {
        self.search_match_idx = Some(0);
        self.cursor_offset = first;
        self.scroll_to_cursor();
    } else {
        self.search_match_idx = None;
    }
}
```

**Step 3: Display error in show_search_bar()**

In `show_search_bar()`, after the match counter (after line 377), add inside the `ui.horizontal` closure:

```rust
if let Some(err) = &self.search_error {
    ui.label(RichText::new(err).color(Color32::from_rgb(220, 80, 80)));
}
```

This requires adding `RichText` and `Color32` to the imports at the top of `app.rs` (line 2):

```rust
use egui::{CentralPanel, Color32, Context, Key, RichText, SidePanel, TopBottomPanel};
```

Also clear the error when input changes. After `let re = ui.text_edit_singleline(...)` (line 353), add:

```rust
if re.changed() {
    self.search_error = None;
}
```

**Step 4: Verify**

Run: `cargo build -p hexenly-app 2>&1`
Expected: Compiles with no errors.

Run: `cargo test --workspace 2>&1`
Expected: All tests pass.

**Step 5: Commit**

```bash
git add crates/hexenly-app/src/app.rs && git commit -m "feat: show error feedback for invalid hex search patterns"
```

---

### Task 3: Structure Panel Alignment

**Files:**
- Modify: `crates/hexenly-app/src/panels/structure.rs:47,57-62`

**Step 1: Apply monospace font to field name labels**

In `crates/hexenly-app/src/panels/structure.rs`, change the field label construction (lines 57-62). Add `.font(monospace_font())` to the RichText:

```rust
let label_text = if let Some(fc) = &field.color {
    RichText::new(&field.label)
        .font(monospace_font())
        .color(Color32::from_rgb(fc.r, fc.g, fc.b))
} else {
    RichText::new(&field.label)
        .font(monospace_font())
};
```

**Step 2: Tighten grid spacing**

Change line 47 from `[8.0, 2.0]` to `[6.0, 2.0]`:

```rust
.spacing([6.0, 2.0])
```

**Step 3: Verify**

Run: `cargo build -p hexenly-app 2>&1`
Expected: Compiles with no errors.

**Step 4: Commit**

```bash
git add crates/hexenly-app/src/panels/structure.rs && git commit -m "fix: use monospace font throughout structure panel for alignment"
```

---

### Task 4: Status Bar Hierarchy

**Files:**
- Modify: `crates/hexenly-app/src/app.rs:2,398-426`

**Step 1: Update imports**

Make sure the import at line 2 of `app.rs` includes `Layout` (in addition to `Color32` and `RichText` added in Task 2):

```rust
use egui::{CentralPanel, Color32, Context, Key, Layout, RichText, SidePanel, TopBottomPanel};
```

**Step 2: Rewrite show_status_bar()**

Replace the entire `show_status_bar()` method:

```rust
fn show_status_bar(&self, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        if let Some(file) = &self.file {
            let name = file
                .path()
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".into());
            ui.label(RichText::new(&name).strong());
            ui.label(RichText::new(format_size(file.len())).weak());

            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(resolved) = &self.resolved_template {
                    ui.label(&resolved.name);
                    ui.label(RichText::new("Template:").weak());
                    ui.separator();
                }
                if let Some(sel) = &self.selection {
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

**Step 3: Verify**

Run: `cargo build -p hexenly-app 2>&1`
Expected: Compiles with no errors.

**Step 4: Commit**

```bash
git add crates/hexenly-app/src/app.rs && git commit -m "fix: improve status bar layout with visual hierarchy"
```

---

### Task 5: Cursor Prominence

**Files:**
- Modify: `crates/hexenly-app/src/theme.rs:14`
- Modify: `crates/hexenly-app/src/panels/hex_view.rs:1,131-132,156-157`

**Step 1: Brighten cursor color and add border color**

In `crates/hexenly-app/src/theme.rs`, change line 14:

```rust
pub const CURSOR_BG: Color32 = Color32::from_rgb(96, 128, 192);
```

Add a new constant after it:

```rust
pub const CURSOR_BORDER: Color32 = Color32::from_rgb(140, 170, 220);
```

**Step 2: Add stroke import and cursor border in hex view**

In `crates/hexenly-app/src/panels/hex_view.rs`, update the import at line 1 to include `Stroke`:

```rust
use egui::{Color32, Pos2, Rect, ScrollArea, Sense, Stroke, Ui, Vec2};
```

Replace the cursor fill at line 131-132:

```rust
if is_cursor {
    painter.rect_filled(hex_rect, 0.0, HexColors::CURSOR_BG);
    painter.rect_stroke(hex_rect, 0.0, Stroke::new(1.0, HexColors::CURSOR_BORDER));
```

Replace the ASCII pane cursor fill at lines 156-157:

```rust
if is_cursor {
    painter.rect_filled(ascii_rect, 0.0, HexColors::CURSOR_BG);
    painter.rect_stroke(ascii_rect, 0.0, Stroke::new(1.0, HexColors::CURSOR_BORDER));
```

**Step 3: Verify**

Run: `cargo build -p hexenly-app 2>&1`
Expected: Compiles with no errors.

**Step 4: Commit**

```bash
git add crates/hexenly-app/src/theme.rs crates/hexenly-app/src/panels/hex_view.rs && git commit -m "fix: improve cursor visibility with brighter color and border"
```

---

### Task 6: Template Load Error Notifications

**Files:**
- Modify: `crates/hexenly-app/src/app.rs:1-2,19-53,55-93,187-208,512-537`

**Step 1: Add notification types and field**

At the top of `app.rs`, add `use std::time::Instant;` to the imports (line 1 area).

After the `TextEncoding` enum (after line 17), add:

```rust
#[derive(Debug, Clone)]
enum NotificationLevel {
    Error,
    Warning,
}

#[derive(Debug, Clone)]
struct Notification {
    message: String,
    level: NotificationLevel,
    created: Instant,
}

const NOTIFICATION_DURATION_SECS: f32 = 5.0;
```

Add to `HexenlyApp` struct (after `template_filter` field):

```rust
notifications: Vec<Notification>,
```

And in the `Self { ... }` initializer:

```rust
notifications: Vec::new(),
```

**Step 2: Populate notifications from template load errors**

In `HexenlyApp::new()`, after the `for (name, err) in &registry.load_errors` loop (lines 91-93), change to collect notifications:

```rust
let mut notifications = Vec::new();
for (name, err) in &registry.load_errors {
    tracing::error!("Failed to load template {name}: {err}");
    notifications.push(Notification {
        message: format!("Failed to load template {name}: {err}"),
        level: NotificationLevel::Error,
        created: Instant::now(),
    });
}
```

And in the initializer, change `notifications: Vec::new()` to `notifications,`.

**Step 3: Populate notifications from template resolve warnings**

In `resolve_active_template()` (around lines 203-205), change the warning loop:

```rust
for warning in &result.warnings {
    tracing::warn!("Template resolve: {warning}");
    self.notifications.push(Notification {
        message: format!("Template: {warning}"),
        level: NotificationLevel::Warning,
        created: Instant::now(),
    });
}
```

**Step 4: Add notification rendering method**

Add a new method to `HexenlyApp`:

```rust
fn show_notifications(&mut self, ui: &mut egui::Ui) {
    let now = Instant::now();
    self.notifications.retain(|n| now.duration_since(n.created).as_secs_f32() < NOTIFICATION_DURATION_SECS);

    if self.notifications.is_empty() {
        return;
    }

    egui::Area::new(egui::Id::new("notifications"))
        .anchor(egui::Align2::RIGHT_TOP, [-8.0, 8.0])
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                for notification in &self.notifications {
                    let elapsed = now.duration_since(notification.created).as_secs_f32();
                    let alpha = if elapsed > NOTIFICATION_DURATION_SECS - 1.0 {
                        ((NOTIFICATION_DURATION_SECS - elapsed).max(0.0) * 255.0) as u8
                    } else {
                        255
                    };
                    let color = match notification.level {
                        NotificationLevel::Error => Color32::from_rgba_unmultiplied(220, 80, 80, alpha),
                        NotificationLevel::Warning => Color32::from_rgba_unmultiplied(220, 180, 60, alpha),
                    };
                    ui.label(RichText::new(&notification.message).color(color));
                }
            });
        });

    // Request repaint while notifications are visible (for fade-out)
    ui.ctx().request_repaint();
}
```

**Step 5: Call notification rendering from update()**

In the `update()` method, inside the `CentralPanel::default().show(ctx, |ui| { ... })` block (around line 513), add at the end of the closure (before the closing `});`):

```rust
self.show_notifications(ui);
```

**Step 6: Verify**

Run: `cargo build -p hexenly-app 2>&1`
Expected: Compiles with no errors.

Run: `cargo test --workspace 2>&1`
Expected: All tests pass.

**Step 7: Commit**

```bash
git add crates/hexenly-app/src/app.rs && git commit -m "feat: add notification overlay for template load errors and warnings"
```

---

### Task 7: Final Verification

**Step 1: Full clippy check**

Run: `cargo clippy --workspace 2>&1`
Expected: Zero warnings.

**Step 2: Full test suite**

Run: `cargo test --workspace 2>&1`
Expected: All 24+ tests pass.

**Step 3: Build release**

Run: `cargo build --workspace --release 2>&1`
Expected: Compiles successfully.
