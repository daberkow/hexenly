<p align="center">
  <img src="docs/logo256.png" alt="Hexenly" width="256">
</p>

<h1 align="center">Hexenly</h1>

<p align="center">A hex editor with structured binary template support, built with Rust and egui.</p>

Hexenly lets you open any binary file and inspect its raw bytes in a familiar hex+ASCII view. Apply built-in templates for common formats (PNG, BMP, ELF, ZIP, ISO 9660, FAT32, Cybiko CFS) to see color-coded structure overlays, field-level breakdowns, and decoded values — all without leaving the editor.

## Features

- Memory-mapped file I/O for fast loading of large files
- Color-coded hex + ASCII display with configurable column widths (8/16/24/32)
- Hex and ASCII pane editing with insert/overwrite modes
- Undo/redo with full edit history
- Byte inspector with little-endian and big-endian interpretations
- Hex and text search with match navigation
- Go-to-offset (decimal or `0x` hex)
- Drag-and-drop file opening
- Menu bar with File/Edit/View menus and keyboard shortcuts
- Template engine with TOML-based binary format definitions
- Dynamic template expressions: field references, arithmetic, conditions, repeating regions
- Auto-detection of file format via magic bytes or extension
- Structure panel showing resolved regions and fields with decoded values
- Click any field in the structure panel to jump to its offset

## Requirements

- Rust 1.85+ (edition 2024)
- A working C compiler and linker (for native dependencies)
- On Linux: development packages for a display server
  - X11: `libxcb`, `libxkbcommon` and related `-dev`/`-devel` packages
  - Wayland: `libwayland-client`, `libxkbcommon` and related `-dev`/`-devel` packages
  - Fedora: `sudo dnf install libxcb-devel libxkbcommon-devel wayland-devel`
  - Ubuntu/Debian: `sudo apt install libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev`

## Building

```sh
cargo build --release
```

## Running

```sh
cargo run -p hexenly-app
```

Open a file directly:

```sh
cargo run -p hexenly-app -- path/to/file.png
```

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+O` | Open file |
| `Ctrl+S` | Save |
| `Ctrl+Shift+S` | Save As |
| `Ctrl+Z` | Undo |
| `Ctrl+Y` | Redo |
| `Ctrl+F` | Search |
| `Ctrl+G` | Go to offset |
| `Ctrl+A` | Select all |
| `Insert` | Toggle insert/overwrite mode |
| `Esc` | Close dialog |

## Built-in Templates

Hexenly ships with templates for these formats:

| Format | Coverage |
|--------|----------|
| PNG | Signature + IHDR chunk |
| BMP | File header + DIB header |
| ELF | Identification + 64-bit header |
| ZIP | Local file entries (repeating, with dynamic field lengths) |
| ISO 9660 | Primary volume descriptor + path table |
| FAT32 | Boot sector + BPB + FSInfo |
| Cybiko CFS | Xtreme flash filesystem (boot blocks + file pages) |

Templates are TOML files — see `templates/` for examples. When you open a file, Hexenly checks magic bytes first, then falls back to the file extension to auto-apply the right template.

## License

MIT
