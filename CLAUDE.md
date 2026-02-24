# Hexenly

A hex editor with structured binary template support, built with Rust and egui.

## Architecture

Three-crate workspace:

- **hexenly-core** — File I/O (mmap), byte interpretation, search, selection
- **hexenly-templates** — Template schema, TOML parsing, resolution engine, loader
- **hexenly-app** — egui/eframe GUI application

## Build & Run

```bash
cargo build --workspace
cargo run -p hexenly-app
cargo run -p hexenly-app -- path/to/file.bin
```

## Key Design Decisions

- **Memory-mapped files** via `memmap2` for large file support
- **`TemplateColor` not `egui::Color32`** — keeps hexenly-templates GUI-agnostic; conversion happens at the render boundary in hexenly-app
- **`include_str!` for built-in templates** — PNG, BMP, ELF, ZIP templates baked into the binary, loaded in `app.rs::HexenlyApp::new()`
- **Simple per-byte region iteration** in hex view — no interval tree needed (<20 regions, <500 visible bytes)
- **Sequential field offsets** — fields with no explicit offset follow the previous field
- **Custom serde Deserialize** for `OffsetExpr` and `LengthExpr` — integers parse as absolute/fixed, strings like `"after:id"`, `"from:id"`, `"to_end"` parse as expressions

## Project Status

- **Phase 1** (complete): Hex viewer, mmap file reading, painter-based hex grid, byte inspector, search, go-to-offset
- **Phase 2** (complete): Template engine — TOML parsing, resolution engine (static offsets/lengths), overlay rendering, template browser sidebar, structure map panel, auto-detection via magic bytes, 4 built-in templates
- **Phase 3** (complete): Dynamic expressions (`AfterField`, `FromField`, `Expr` offsets/lengths), repeating regions (`Count`, `UntilEof`, `UntilMagic`), conditional regions/fields, arithmetic expressions, enum/bitflag display, 7 built-in templates
- **Future ideas**:
  - Nested template overlay — select a region and apply a different template to it (e.g., read a disk image as FAT32, then overlay MBR on the first 512 bytes)
  - Find & replace (hex and ASCII patterns)
  - Copy/paste (as hex, ASCII, C array, Python bytes, etc.)
  - Data visualization (entropy graph, byte histogram, strings view)
  - File diffing (side-by-side comparison of two files)
  - Inspector enhancements (date decoders, text encodings, bitfield view)
  - Export (save selection as file, export to various formats)

## Template Format

Templates are TOML files in `templates/` grouped by category subdirectory. Schema lives in `hexenly-templates/src/schema.rs`. Key types:

- `Template` — name, description, extensions, magic bytes, endianness, regions
- `Region` — id, label, color (#RRGGBB), offset, length, fields
- `Field` — id, label, field_type, length, role, description

## Code Conventions

- Edition 2024
- `thiserror` for error enums
- `tracing` for logging (filter: `hexenly=info`)
- egui immediate-mode UI with painter-based hex rendering
- No `unsafe` code
