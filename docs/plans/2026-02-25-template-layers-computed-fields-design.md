# Template Layers & Computed Fields

## Context

When opening a raw disk image (e.g. a DOS 4.0 hard drive), the MBR template auto-detects at offset 0, but the actual FAT16 filesystem lives at a partition offset deep inside the image (e.g. 0x46E00). Today templates can only apply at offset 0 and only one template can be active at a time, so there's no way to overlay both MBR and FAT16 simultaneously.

This design adds three capabilities:
1. **Template base offset** — apply a template at any byte position
2. **Multiple active template layers** — several templates active at once, each at different offsets
3. **Computed fields with template chaining** — fields that calculate values from other fields and optionally auto-apply a linked template at the computed offset

## Schema Changes (hexenly-templates)

### New field type: `computed`

A computed field evaluates an arithmetic expression instead of reading bytes from the file.

```toml
[[regions.fields]]
id = "partition_offset"
label = "Partition Byte Offset"
field_type = "computed"
expression = "expr:starting_lba * 512"
role = "offset"
apply_template = "FAT16"
description = "Byte offset where the FAT16 partition begins"
```

New optional properties on `Field` in `schema.rs`:
- `expression: Option<String>` — arithmetic expression using the existing `expr:` syntax
- `apply_template: Option<String>` — template name to auto-apply at the computed value

### New resolved types

`ResolvedField` gains:
- `computed_value: Option<u64>` — result of expression evaluation

`ResolveResult` gains:
- `template_links: Vec<TemplateLink>`

```rust
struct TemplateLink {
    template_name: String,
    offset: u64,
    source_field_id: String,
}
```

### Engine changes

In `engine::resolve()`:
- When `field_type == Computed`, skip byte reading. Evaluate the `expression` using the existing `ArithExpr` evaluator and `field_map`.
- Set `computed_value`, `length = 0`, `raw_bytes = vec![]`.
- Format `display_value` as hex+decimal (e.g. `"0x46E00 (290304)"`).
- If `apply_template` is set, emit a `TemplateLink` in the result.

No `base_offset` parameter needed on `resolve()` itself — the app slices the file bytes before calling the engine, then adds the base offset back to all resolved positions.

## App State Changes (hexenly-app)

### Template layer stack

Replace `active_template_index: Option<usize>` with:

```rust
struct TemplateLayer {
    registry_index: usize,
    base_offset: u64,
    resolved: ResolvedTemplate,
    source: LayerSource,
}

enum LayerSource {
    AutoDetected,
    Manual,
    LinkedFrom(String), // parent field id
}
```

State: `template_layers: Vec<TemplateLayer>`

### Resolution flow

1. When a template is applied at `base_offset`, call `engine::resolve(template, &file_bytes[base_offset..])`.
2. Add `base_offset` to all resolved region/field offsets to make them absolute within the file.
3. Check `template_links` in the result. For each link, look up the template name in the registry. If a layer for that (template, offset) pair doesn't already exist, add it (prevents infinite loops).
4. Cache the resolved result in the layer.

### Hex view rendering

Iterate all layers' resolved regions when coloring bytes. Topmost (most recently added) layer wins when regions from different layers overlap the same byte.

### Structure panel

Show a collapsible section per layer, labeled with template name and base offset (e.g. "MBR @ 0x0", "FAT16 @ 0x46E00"). Computed fields with `apply_template` render as clickable links that scroll the hex view to that offset.

## UI Changes

### Template Layers sub-panel

Below the template browser list, a "Active Layers" section shows each active layer:
- Template name + base offset
- Source indicator (auto / manual / linked)
- Remove button (X) — removing a linked-from layer also removes layers it chained

### Offset input in template browser

A text input labeled "Apply at offset" (defaults to "0"). Accepts hex (0x...) or decimal. When the user selects a template, it applies at this offset.

### Right-click context menu

Right-clicking a byte in the hex view offers "Apply template at 0xNNNN..." with a submenu of available templates. Selecting one adds a manual layer at the cursor position.

## Files to modify

- `crates/hexenly-templates/src/schema.rs` — add `expression`, `apply_template` to `Field`; add `Computed` to `FieldType`
- `crates/hexenly-templates/src/engine.rs` — handle computed field resolution, emit `TemplateLink`s
- `crates/hexenly-templates/src/resolved.rs` — add `computed_value` to `ResolvedField`, add `TemplateLink`
- `crates/hexenly-app/src/app.rs` — replace single active template with layer stack, implement chaining logic, add base offset adjustment
- `crates/hexenly-app/src/panels/templates.rs` — add layers sub-panel, offset input field
- `crates/hexenly-app/src/panels/hex_view.rs` — iterate all layers for byte coloring
- `crates/hexenly-app/src/panels/structure.rs` — collapsible per-layer sections, clickable computed fields
- `templates/filesystems/mbr.toml` — add computed fields for partition byte offsets with `apply_template`

## Verification

- `cargo build --workspace` and `cargo test --workspace` pass
- Open a disk image with MBR — auto-detects MBR template at offset 0
- MBR computed field shows partition byte offset and auto-applies FAT16 at that offset
- Layers panel shows both MBR and FAT16 layers
- Structure panel shows collapsible sections for both
- Hex view colors bytes from both templates (FAT16 colors at partition offset)
- User can right-click any byte and manually apply a template there
- User can type an offset in the template browser and apply a template
- User can remove layers individually; removing a parent removes chained children
- No infinite loops from circular template links
