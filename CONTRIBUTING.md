# Contributing to Hexenly

Thanks for your interest in contributing! This guide covers the most common way to contribute — writing binary format templates — as well as general development setup.

## Writing Templates

Templates are TOML files that teach Hexenly how to decode binary formats. They live in the `templates/` directory, organized by category:

```
templates/
  archives/     ZIP, TAR, GZIP, ...
  executables/  ELF, PE, ...
  filesystems/  FAT32, MBR, GPT, ...
  images/       PNG, BMP, GIF, JPEG, ...
  media/        WAV, ...
```

### Quick Start

1. Create a new `.toml` file in the appropriate category folder
2. Register it in `crates/hexenly-app/src/app.rs` (see [Registering a Template](#registering-a-template))
3. Build and test: `cargo build --workspace && cargo test --workspace`

### Template Structure

Every template needs a header and at least one region with fields:

```toml
name = "Format Name"
description = "Short description of the format"
extensions = ["ext1", "ext2"]
magic = "89504E47"          # hex string to match at magic_offset
magic_offset = 0            # where to look for magic bytes (default: 0)
endian = "little"           # "little" or "big" (default: "little")

[[regions]]
id = "header"               # unique identifier
label = "File Header"       # display name
color = "#2ECC71"           # hex color for overlay
offset = 0                  # byte offset (or expression)
group = "header"            # optional grouping label
description = "..."         # optional tooltip text

[[regions.fields]]
id = "magic"                # unique identifier
label = "Magic"             # display name
field_type = "ascii"        # data type (see below)
length = 4                  # byte length (or expression)
role = "magic"              # optional semantic role
description = "..."         # optional tooltip text
color = "#3498DB"           # optional per-field color override
```

### Field Types

| Type | Size | Description |
|------|------|-------------|
| `u8`, `i8` | 1 | Unsigned/signed 8-bit integer |
| `u16_le`, `u16_be` | 2 | 16-bit integer (little/big endian) |
| `u32_le`, `u32_be` | 4 | 32-bit integer |
| `u64_le`, `u64_be` | 8 | 64-bit integer |
| `i16_le`, `i16_be` | 2 | Signed 16-bit integer |
| `i32_le`, `i32_be` | 4 | Signed 32-bit integer |
| `i64_le`, `i64_be` | 8 | Signed 64-bit integer |
| `f32_le`, `f32_be` | 4 | 32-bit float |
| `f64_le`, `f64_be` | 8 | 64-bit float |
| `bytes` | variable | Raw byte sequence |
| `ascii` | variable | ASCII text |
| `utf8` | variable | UTF-8 text |

### Field Roles

Roles are optional hints that give semantic meaning to a field:

`magic`, `version`, `size`, `offset`, `count`, `checksum`, `padding`, `reserved`, `data`

### Dynamic Expressions

**Field-referenced lengths** — a field's length comes from another field's value:

```toml
length = "from:data_size"
```

**Field-referenced offsets** — a region starts right after another region/field:

```toml
offset = "after:header"
```

**Value-referenced offsets** — a region's offset comes from a field's value:

```toml
offset = "from:e_lfanew"
```

**Arithmetic expressions** — compute offsets or lengths with math:

```toml
offset = "expr:block_num * block_size"
length = "expr:total_size - header_size"
```

**To-end length** — consume all remaining bytes:

```toml
length = "to_end"
```

### Repeating Regions

Regions can repeat with three modes:

```toml
# Repeat until end of file
repeat = "until_eof"

# Repeat a fixed number of times (from a field's value)
repeat = "count"
repeat_count = "num_entries"

# Repeat until sentinel bytes are found
repeat = "until_magic"
repeat_until = "504B0102"
```

### Conditional Regions and Fields

Skip a region or field based on another field's value:

```toml
condition = "version == 2"
condition = "flags != 0"
condition = "type_code >= 0x80"
```

Supported operators: `==`, `!=`, `<`, `>`, `<=`, `>=`. Values can be decimal or hex (`0x` prefix).

### Enum Labels

Map numeric values to human-readable names:

```toml
[[regions.fields]]
id = "compression"
label = "Compression Method"
field_type = "u16_le"
length = 2

[regions.fields.enum_values]
"0" = "Stored"
"8" = "Deflated"
"14" = "LZMA"
```

### Bit Flags

Decode individual bits into named flags:

```toml
[[regions.fields]]
id = "flags"
label = "Flags"
field_type = "u16_le"
length = 2

[regions.fields.bit_flags]
"0" = "Encrypted"
"3" = "Data Descriptor"
"11" = "UTF-8"
```

### Registering a Template

After creating the TOML file, add it to `crates/hexenly-app/src/app.rs` in the `HexenlyApp::new()` function:

```rust
registry.load_builtin(
    "category",      // folder name: images, archives, executables, filesystems, media
    "Format Name",   // display name in template browser
    include_str!("../../../templates/category/format.toml"),
);
```

### Tips

- Look at existing templates in `templates/` for real-world examples
- Use distinct colors for different regions so they're visually clear in the hex overlay
- Add `description` to fields with non-obvious meanings
- Use `enum_values` liberally — they make the structure panel much more useful
- Keep region and field IDs short but descriptive (they're used in expressions)
- Test with a real file of that format to make sure offsets line up

## Development

### Building

```sh
cargo build --workspace
```

### Testing

```sh
cargo test --workspace
```

### Linting

```sh
cargo clippy --workspace -- -D warnings
```

### Project Structure

```
crates/
  hexenly-core/       File I/O, search, edit buffer
  hexenly-templates/  Template schema, TOML parsing, resolution engine
  hexenly-app/        egui GUI application
templates/            Built-in template TOML files
```

### Code Conventions

- Rust edition 2024
- `thiserror` for error types
- `tracing` for logging
- No `unsafe` code
- Keep hexenly-templates GUI-agnostic (no egui types)
