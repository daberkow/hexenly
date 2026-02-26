# Template Layers & Computed Fields Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Support multiple template overlays at arbitrary offsets with computed fields that auto-chain templates (e.g. MBR -> FAT16 at partition offset).

**Architecture:** Add `Computed` field type to the schema that evaluates expressions over resolved fields. Replace single active template with a layer stack (`Vec<TemplateLayer>`) where each layer has a base offset. The engine resolves against a byte slice; the app adjusts offsets. Computed fields can declare `apply_template` to auto-add linked layers.

**Tech Stack:** Rust, egui, TOML templates (serde), existing ArithExpr evaluator in hexenly-templates

---

### Task 1: Schema — Add `Computed` field type and new Field properties

**Files:**
- Modify: `crates/hexenly-templates/src/schema.rs`

**Step 1: Add `Computed` variant to `FieldType` enum (~line 237)**

In the `FieldType` enum, add `Computed` after `Ascii`:
```rust
pub enum FieldType {
    // ... existing variants ...
    Ascii,
    Computed,
}
```

**Step 2: Add `expression` and `apply_template` to `Field` struct (~line 134)**

Add after the `color` field:
```rust
    /// Arithmetic expression for computed fields (e.g. "expr:field_a * 512")
    #[serde(default)]
    pub expression: Option<String>,
    /// Template name to auto-apply at the computed value offset
    #[serde(default)]
    pub apply_template: Option<String>,
```

**Step 3: Handle `Computed` in `FieldType::natural_size()` (~line 250)**

Add to the match arm for variable-length types:
```rust
FieldType::Bytes | FieldType::Utf8 | FieldType::Ascii | FieldType::Computed => None,
```

**Step 4: Run tests**

Run: `cargo test -p hexenly-templates`
Expected: All 29 existing tests pass (no behavior change yet).

**Step 5: Commit**

```
feat: add Computed field type and expression/apply_template to schema
```

---

### Task 2: Resolved types — Add `computed_value` and `TemplateLink`

**Files:**
- Modify: `crates/hexenly-templates/src/resolved.rs`

**Step 1: Add `computed_value` to `ResolvedField` (~line 86)**

Add after `color`:
```rust
    /// For computed fields: the evaluated numeric result.
    pub computed_value: Option<u64>,
```

**Step 2: Add `TemplateLink` struct and update exports**

Add at the end of the file:
```rust
/// A request to auto-apply a template at a computed offset.
#[derive(Debug, Clone)]
pub struct TemplateLink {
    /// Name of the template to apply (matched against registry).
    pub template_name: String,
    /// Absolute byte offset where the template should be applied.
    pub offset: u64,
    /// ID of the computed field that produced this link.
    pub source_field_id: String,
}
```

**Step 3: Run tests**

Run: `cargo test -p hexenly-templates`
Expected: All 29 tests pass.

**Step 4: Commit**

```
feat: add computed_value to ResolvedField and TemplateLink type
```

---

### Task 3: Engine — Add `TemplateLink` to `ResolveResult`

**Files:**
- Modify: `crates/hexenly-templates/src/engine.rs`

**Step 1: Update `ResolveResult` struct (~line 15)**

Add `template_links` field:
```rust
pub struct ResolveResult {
    pub template: ResolvedTemplate,
    pub warnings: Vec<ResolveWarning>,
    pub template_links: Vec<TemplateLink>,
}
```

Import `TemplateLink` from resolved module at top of file.

**Step 2: Initialize `template_links` in `resolve()` (~line 184)**

Add alongside `warnings`:
```rust
    let mut template_links = Vec::new();
```

**Step 3: Include `template_links` in return value (~line 625)**

```rust
    ResolveResult {
        template: ResolvedTemplate { name, description, regions },
        warnings,
        template_links,
    }
```

**Step 4: Run tests**

Run: `cargo test -p hexenly-templates`
Expected: All 29 tests pass (template_links is always empty for now).

**Step 5: Commit**

```
feat: add template_links to ResolveResult
```

---

### Task 4: Engine — Resolve computed fields

**Files:**
- Modify: `crates/hexenly-templates/src/engine.rs`
- Test: `crates/hexenly-templates/src/engine.rs` (inline tests)

**Step 1: Write a failing test for computed field resolution**

Add to the `tests` module at the bottom of engine.rs:
```rust
#[test]
fn test_computed_field() {
    let toml_str = r#"
name = "Test"
description = "Computed field test"
extensions = []
endian = "little"

[[regions]]
id = "header"
label = "Header"
offset = 0
length = 8

[[regions.fields]]
id = "sector_count"
label = "Sector Count"
field_type = "u32_le"
length = 4

[[regions.fields]]
id = "sector_size"
label = "Sector Size"
field_type = "u32_le"
length = 4

[[regions]]
id = "computed_region"
label = "Computed"
offset = "after:header"
length = 0

[[regions.fields]]
id = "total_bytes"
label = "Total Bytes"
field_type = "computed"
length = 0
expression = "expr:sector_count * sector_size"
"#;
    let template = crate::parser::parse_template_str(toml_str).unwrap();
    // sector_count=10 (0x0A000000 LE), sector_size=512 (0x00020000 LE)
    let mut data = vec![0u8; 16];
    data[0..4].copy_from_slice(&10u32.to_le_bytes());
    data[4..8].copy_from_slice(&512u32.to_le_bytes());

    let result = resolve(&template, &data);
    assert!(result.warnings.is_empty(), "warnings: {:?}", result.warnings);

    let computed_region = &result.template.regions[1];
    assert_eq!(computed_region.fields.len(), 1);
    let field = &computed_region.fields[0];
    assert_eq!(field.id, "total_bytes");
    assert_eq!(field.computed_value, Some(5120));
    assert!(field.display_value.contains("5120"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p hexenly-templates test_computed_field`
Expected: FAIL (computed fields not handled yet).

**Step 3: Implement computed field resolution in the field loop**

In the field resolution section of `resolve()` (~line 456 area, where raw bytes are extracted), add a branch before the normal byte extraction:

```rust
// Inside the per-field loop, before extracting raw bytes:
if field.field_type == FieldType::Computed {
    // Evaluate expression
    let computed_val = if let Some(expr_str) = &field.expression {
        if let Some(body) = expr_str.strip_prefix("expr:") {
            match crate::schema::parse_arith_expr_public(body) {
                Ok(expr) => eval_arith(&expr, &field_map),
                Err(e) => {
                    warnings.push(ResolveWarning {
                        message: format!(
                            "field '{}': failed to parse expression: {}",
                            field_id, e
                        ),
                    });
                    None
                }
            }
        } else {
            // Try as simple field reference
            field_map.get(expr_str.as_str()).and_then(|info| info.numeric_value)
        }
    } else {
        warnings.push(ResolveWarning {
            message: format!("field '{}': computed field has no expression", field_id),
        });
        None
    };

    let display = match computed_val {
        Some(v) => format!("0x{:X} ({})", v, v),
        None => "??".to_string(),
    };

    // Register in field_map so other fields can reference it
    field_map.insert(
        field_id.clone(),
        ResolvedFieldInfo {
            offset: field_cursor,
            length: 0,
            numeric_value: computed_val,
        },
    );

    // Emit template link if apply_template is set
    if let (Some(tmpl_name), Some(offset)) = (&field.apply_template, computed_val) {
        template_links.push(TemplateLink {
            template_name: tmpl_name.clone(),
            offset,
            source_field_id: field_id.clone(),
        });
    }

    resolved_fields.push(ResolvedField {
        id: field_id,
        label: field.label.clone(),
        field_type: field.field_type.clone(),
        offset: field_cursor,
        length: 0,
        role: field.role.clone(),
        description: field.description.clone(),
        raw_bytes: vec![],
        display_value: display,
        color: field_color,
        computed_value: computed_val,
    });

    // Don't advance field_cursor — computed fields have zero length
    continue;
}
```

Note: The `parse_arith_expr` function in schema.rs is currently private. It needs to be made `pub` (or we add a public wrapper). Add to schema.rs:

```rust
/// Public entry point for parsing arithmetic expressions.
pub fn parse_arith_expr_public(s: &str) -> Result<ArithExpr, String> {
    parse_arith_expr(s)
}
```

Also update all existing `ResolvedField` construction sites to include `computed_value: None`.

**Step 4: Run test**

Run: `cargo test -p hexenly-templates test_computed_field`
Expected: PASS

**Step 5: Write test for computed field with apply_template**

```rust
#[test]
fn test_computed_field_template_link() {
    let toml_str = r#"
name = "Test"
description = "Template link test"
extensions = []
endian = "little"

[[regions]]
id = "header"
label = "Header"
offset = 0
length = 4

[[regions.fields]]
id = "offset_val"
label = "Offset"
field_type = "u32_le"
length = 4

[[regions]]
id = "links"
label = "Links"
offset = "after:header"
length = 0

[[regions.fields]]
id = "target"
label = "Target Offset"
field_type = "computed"
length = 0
expression = "expr:offset_val * 512"
apply_template = "FAT16"
"#;
    let template = crate::parser::parse_template_str(toml_str).unwrap();
    let mut data = vec![0u8; 8];
    data[0..4].copy_from_slice(&100u32.to_le_bytes());

    let result = resolve(&template, &data);
    assert_eq!(result.template_links.len(), 1);
    assert_eq!(result.template_links[0].template_name, "FAT16");
    assert_eq!(result.template_links[0].offset, 51200);
    assert_eq!(result.template_links[0].source_field_id, "target");
}
```

**Step 6: Run tests**

Run: `cargo test -p hexenly-templates`
Expected: All tests pass including new ones.

**Step 7: Commit**

```
feat: resolve computed fields and emit template links
```

---

### Task 5: App state — Replace single template with layer stack

**Files:**
- Modify: `crates/hexenly-app/src/app.rs`

**Step 1: Define `TemplateLayer` and `LayerSource` types**

Add near the top of app.rs (after imports):
```rust
/// How a template layer was added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayerSource {
    AutoDetected,
    Manual,
    LinkedFrom(String),
}

/// A single active template overlay at a specific offset.
#[derive(Debug, Clone)]
pub struct TemplateLayer {
    pub registry_index: usize,
    pub base_offset: u64,
    pub resolved: ResolvedTemplate,
    pub source: LayerSource,
}
```

**Step 2: Replace template fields in `HexenlyApp` struct**

Remove:
```rust
active_template_index: Option<usize>,
resolved_template: Option<ResolvedTemplate>,
```

Add:
```rust
template_layers: Vec<TemplateLayer>,
/// Offset for the next template selection (set via UI).
template_apply_offset: String,
```

**Step 3: Update initialization**

Replace old field init with:
```rust
template_layers: Vec::new(),
template_apply_offset: "0".to_string(),
```

**Step 4: Create helper methods**

```rust
/// Add a template layer at the given offset and resolve it.
fn add_template_layer(&mut self, registry_index: usize, base_offset: u64, source: LayerSource) {
    // Check for duplicate (same template at same offset)
    if self.template_layers.iter().any(|l| l.registry_index == registry_index && l.base_offset == base_offset) {
        return;
    }

    let entry = &self.template_registry.entries[registry_index];
    let data = match &self.edit_buffer {
        Some(buf) => buf.data(),
        None => return,
    };

    if base_offset as usize >= data.len() {
        return;
    }

    let slice = &data[base_offset as usize..];
    let result = hexenly_templates::engine::resolve(&entry.template, slice);

    // Adjust all offsets by base_offset
    let adjusted_regions: Vec<_> = result.template.regions.into_iter().map(|mut r| {
        r.offset += base_offset;
        for f in &mut r.fields {
            f.offset += base_offset;
        }
        r
    }).collect();

    let resolved = ResolvedTemplate {
        name: result.template.name,
        description: result.template.description,
        regions: adjusted_regions,
    };

    // Log warnings
    for w in &result.warnings {
        self.notifications.push(Notification {
            message: format!("[{}] {}", entry.template.name, w.message),
            level: NotificationLevel::Warning,
            timestamp: std::time::Instant::now(),
        });
    }

    self.template_layers.push(TemplateLayer {
        registry_index,
        base_offset,
        resolved,
        source,
    });

    // Process template links (auto-chain)
    for link in result.template_links {
        if let Some(idx) = self.template_registry.entries.iter().position(|e| e.template.name == link.template_name) {
            self.add_template_layer(idx, link.offset, LayerSource::LinkedFrom(link.source_field_id.clone()));
        }
    }
}

/// Remove a template layer and any layers it chained.
fn remove_template_layer(&mut self, index: usize) {
    if index >= self.template_layers.len() {
        return;
    }
    // Collect IDs of fields in this layer that may have linked other layers
    let field_ids: Vec<String> = self.template_layers[index]
        .resolved.regions.iter()
        .flat_map(|r| r.fields.iter())
        .map(|f| f.id.clone())
        .collect();

    self.template_layers.remove(index);

    // Remove any layers that were LinkedFrom fields in the removed layer
    let mut i = 0;
    while i < self.template_layers.len() {
        if let LayerSource::LinkedFrom(ref src) = self.template_layers[i].source {
            if field_ids.contains(src) {
                self.remove_template_layer(i);
                continue; // Don't increment — vec shifted
            }
        }
        i += 1;
    }
}

/// Parse the template_apply_offset string into a u64.
fn parse_apply_offset(&self) -> u64 {
    let s = self.template_apply_offset.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).unwrap_or(0)
    } else {
        s.parse::<u64>().unwrap_or(0)
    }
}
```

**Step 5: Update all code that referenced old fields**

This is the most tedious part. Search for every use of `active_template_index` and `resolved_template` in app.rs and update:

- `auto_detect_template()`: Instead of setting `active_template_index` and calling `resolve_active_template()`, call `self.add_template_layer(idx, 0, LayerSource::AutoDetected)`.
- `resolve_active_template()`: Remove this method entirely (logic moved into `add_template_layer`).
- Template browser `Select(idx)` handler: Call `self.add_template_layer(idx, self.parse_apply_offset(), LayerSource::Manual)`.
- Template browser `Deselect` handler: Call `self.template_layers.clear()`.
- Hex view call site: Instead of passing `self.resolved_template.as_ref()`, collect all resolved templates. The hex view signature will change (Task 8).
- Structure panel call site: Will show all layers (Task 9).
- Any `self.resolved_template.is_some()` checks: Change to `!self.template_layers.is_empty()`.
- File open (clearing state): Change to `self.template_layers.clear()`.

**Step 6: Build and fix compilation errors**

Run: `cargo build --workspace`
Expected: Should compile with the updated call sites. Hex view and structure panel may need temporary compatibility shims until Tasks 8 and 9.

For a temporary shim, pass the first layer's resolved template to the hex view and structure panel:
```rust
let template_overlay = self.template_layers.first().map(|l| &l.resolved);
```

**Step 7: Run tests**

Run: `cargo test --workspace`
Expected: All tests pass.

**Step 8: Commit**

```
refactor: replace single active template with layer stack
```

---

### Task 6: Template browser — Add offset input

**Files:**
- Modify: `crates/hexenly-app/src/panels/templates.rs`
- Modify: `crates/hexenly-app/src/app.rs` (call site)

**Step 1: Update `show()` signature to accept offset string**

```rust
pub fn show(
    ui: &mut Ui,
    registry: &TemplateRegistry,
    active_indices: &[usize],  // changed: layer indices instead of single
    filter: &mut String,
    apply_offset: &mut String,  // new
) -> Option<TemplateBrowserAction>
```

**Step 2: Add offset input field in the UI**

After the filter input (~line 38), add:
```rust
ui.horizontal(|ui| {
    ui.label("Apply at offset:");
    ui.add(egui::TextEdit::singleline(apply_offset).desired_width(100.0).hint_text("0x0"));
});
ui.add_space(4.0);
```

**Step 3: Update the active template highlighting**

Instead of checking `Some(idx) == active_index`, check if `idx` is in the `active_indices` slice:
```rust
let is_active = active_indices.contains(&idx);
```

**Step 4: Update call site in app.rs**

Pass the new arguments:
```rust
let active_indices: Vec<usize> = self.template_layers.iter().map(|l| l.registry_index).collect();
let action = templates::show(
    ui,
    &self.template_registry,
    &active_indices,
    &mut self.template_filter,
    &mut self.template_apply_offset,
);
```

**Step 5: Build and test**

Run: `cargo build --workspace && cargo test --workspace`
Expected: All pass.

**Step 6: Commit**

```
feat: add offset input to template browser panel
```

---

### Task 7: Template layers panel

**Files:**
- Create: `crates/hexenly-app/src/panels/layers.rs`
- Modify: `crates/hexenly-app/src/panels/mod.rs`
- Modify: `crates/hexenly-app/src/app.rs` (call site)

**Step 1: Create the layers panel**

```rust
use egui::Ui;
use crate::app::{TemplateLayer, LayerSource};

pub enum LayerAction {
    Remove(usize),
    GoToOffset(u64),
}

pub fn show(ui: &mut Ui, layers: &[TemplateLayer]) -> Option<LayerAction> {
    let mut action = None;

    if layers.is_empty() {
        ui.label("No active template layers");
        return None;
    }

    for (i, layer) in layers.iter().enumerate() {
        ui.horizontal(|ui| {
            let source_label = match &layer.source {
                LayerSource::AutoDetected => "auto",
                LayerSource::Manual => "manual",
                LayerSource::LinkedFrom(_) => "linked",
            };

            if ui.small_button("\u{2715}").on_hover_text("Remove layer").clicked() {
                action = Some(LayerAction::Remove(i));
            }

            let offset_text = format!("0x{:X}", layer.base_offset);
            if ui.link(&offset_text).on_hover_text("Go to offset").clicked() {
                action = Some(LayerAction::GoToOffset(layer.base_offset));
            }

            ui.label(format!("{} ({})", layer.resolved.name, source_label));
        });
    }

    action
}
```

**Step 2: Add `mod layers;` to panels/mod.rs**

**Step 3: Integrate in app.rs**

Show the layers panel inside the template browser area (below the template list), or as a separate collapsible section. When `LayerAction::Remove(i)` is returned, call `self.remove_template_layer(i)`. When `GoToOffset(off)` is returned, set `self.cursor_offset = off as usize`.

**Step 4: Build and test**

Run: `cargo build --workspace && cargo test --workspace`
Expected: All pass.

**Step 5: Commit**

```
feat: add template layers panel showing active overlays
```

---

### Task 8: Hex view — Multi-layer rendering

**Files:**
- Modify: `crates/hexenly-app/src/panels/hex_view.rs`
- Modify: `crates/hexenly-app/src/app.rs` (call site)

**Step 1: Change hex view to accept multiple layers**

Update the `show()` signature:
```rust
template_overlay: &[&ResolvedTemplate],  // changed from Option<&ResolvedTemplate>
```

**Step 2: Update the byte coloring loop (~lines 108-134)**

Instead of iterating regions from one template, iterate all layers (last layer wins for overlap):
```rust
if !template_overlay.is_empty() {
    let byte_offset = (first_byte + col) as u64;
    let mut bg_color = None;

    // Last layer wins (iterate in order, last assignment sticks)
    for overlay in template_overlay {
        for region in &overlay.regions {
            if region.contains(byte_offset) {
                let c = region.fields.iter()
                    .find(|f| byte_offset >= f.offset && byte_offset < f.offset + f.length)
                    .and_then(|f| f.color.as_ref())
                    .unwrap_or(&region.color);
                bg_color = Some(Color32::from_rgba_unmultiplied(c.r, c.g, c.b, 38));
            }
        }
    }

    if let Some(color) = bg_color {
        // paint hex background
        // paint ascii background
    }
}
```

**Step 3: Update region label painting (~lines 206-227)**

Iterate all layers for region labels.

**Step 4: Update call site in app.rs**

```rust
let overlays: Vec<&ResolvedTemplate> = self.template_layers.iter().map(|l| &l.resolved).collect();
// pass &overlays to hex_view::show()
```

**Step 5: Build and test**

Run: `cargo build --workspace && cargo test --workspace`
Expected: All pass.

**Step 6: Commit**

```
feat: hex view renders overlays from all template layers
```

---

### Task 9: Structure panel — Per-layer sections

**Files:**
- Modify: `crates/hexenly-app/src/panels/structure.rs`
- Modify: `crates/hexenly-app/src/app.rs` (call site)

**Step 1: Update structure panel to accept multiple layers**

Change signature:
```rust
pub fn show(
    ui: &mut Ui,
    layers: &[TemplateLayer],
    cursor_offset: usize,
) -> Option<StructureAction>
```

**Step 2: Wrap existing rendering in a per-layer loop**

Each layer gets a collapsible section headed by template name + offset:
```rust
for (i, layer) in layers.iter().enumerate() {
    let header = format!("{} @ 0x{:X}", layer.resolved.name, layer.base_offset);
    egui::CollapsingHeader::new(header)
        .default_open(i == 0)
        .show(ui, |ui| {
            // existing region/field rendering, using layer.resolved
        });
}
```

**Step 3: Make computed fields with `apply_template` clickable**

In the field display grid, check if `field.computed_value.is_some()` and render the display value as a clickable link:
```rust
if field.computed_value.is_some() {
    if ui.link(&field.display_value).clicked() {
        action = Some(StructureAction::GoToOffset(field.computed_value.unwrap() as usize));
    }
} else {
    ui.label(&field.display_value);
}
```

**Step 4: Update call site in app.rs**

Pass `&self.template_layers` instead of `&resolved_template`.

**Step 5: Build and test**

Run: `cargo build --workspace && cargo test --workspace`
Expected: All pass.

**Step 6: Commit**

```
feat: structure panel shows per-layer sections with clickable computed fields
```

---

### Task 10: Right-click context menu on hex view

**Files:**
- Modify: `crates/hexenly-app/src/panels/hex_view.rs`
- Modify: `crates/hexenly-app/src/app.rs` (call site)

**Step 1: Add a new `HexViewAction` variant**

```rust
pub enum HexViewAction {
    // ... existing variants ...
    ApplyTemplateAt { offset: u64 },
}
```

Wait — the user needs to also choose which template. Better approach: emit `RequestTemplateMenu(u64)` and let the app handle showing a template picker. Or simpler: use egui's context_menu.

**Step 1 (revised): Add context menu to the hex view response area**

In `hex_view.rs`, after the main hex view rendering, add a context menu on the response rect:
```rust
response.context_menu(|ui| {
    let offset = cursor; // current cursor position
    ui.label(format!("Offset: 0x{:X}", offset));
    ui.separator();
    if ui.button("Apply template here...").clicked() {
        action = Some(HexViewAction::ApplyTemplateAt(offset as u64));
        ui.close_menu();
    }
});
```

Add the new variant to `HexViewAction`.

**Step 2: Handle in app.rs**

When `HexViewAction::ApplyTemplateAt(offset)` is received:
- Set `self.template_apply_offset = format!("0x{:X}", offset)`
- Open the template browser panel: `self.panels.template_browser = true`

This reuses the existing template browser — the user picks a template and it applies at the pre-filled offset. Simple and consistent.

**Step 3: Build and test**

Run: `cargo build --workspace && cargo test --workspace`
Expected: All pass.

**Step 4: Commit**

```
feat: right-click hex view to apply template at cursor offset
```

---

### Task 11: Update MBR template with computed partition offsets

**Files:**
- Modify: `templates/filesystems/mbr.toml`

**Step 1: Add computed fields to MBR partition entries**

For each partition entry that has a starting LBA field, add a computed field that calculates the byte offset and links to the appropriate template. For partition 1:

```toml
[[regions.fields]]
id = "p1_byte_offset"
label = "Partition 1 Byte Offset"
field_type = "computed"
length = 0
expression = "expr:p1_lba_start * 512"
role = "offset"
description = "Byte offset where partition 1 begins"
```

Note: We intentionally do NOT set `apply_template` here because the MBR doesn't know what filesystem is inside. The user sees the computed offset and can manually apply the right template. The `apply_template` feature is useful when the format is known (e.g. a FAT16 EBR always contains a FAT partition).

**Step 2: Build and test**

Run: `cargo build --workspace && cargo test --workspace`
Expected: All pass. Template parser handles the new computed field.

**Step 3: Commit**

```
feat: add computed partition byte offsets to MBR template
```

---

### Task 12: Integration verification

**Step 1: Build the full workspace**

Run: `cargo build --workspace`
Expected: Clean build.

**Step 2: Run all tests**

Run: `cargo test --workspace`
Expected: All tests pass.

**Step 3: Manual verification**

1. `cargo run -p hexenly-app -- ~/HardCard.dd`
2. MBR template should auto-detect at offset 0
3. Structure panel shows MBR fields including computed "Partition 1 Byte Offset"
4. Click the computed offset value to jump there
5. Right-click at that position -> "Apply template here..."
6. Template browser opens with offset pre-filled
7. Select FAT16 from the browser
8. Layers panel now shows: `MBR @ 0x0` and `FAT16 @ 0x46E00`
9. Hex view shows both overlays with different colors
10. Structure panel has collapsible sections for both layers

**Step 4: Final commit**

```
feat: template layers with computed fields and multi-layer overlay
```
