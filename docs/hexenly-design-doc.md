DESIGN DOCUMENT

**Hexenly**

A Template-Driven Hex Editor

Language: **Rust** GUI: **egui / eframe** License: **MIT**

Version 0.1.0 • February 2026

1\. Overview

Hexenly is an open-source, template-driven hex editor built in Rust.
While standard hex editors show raw bytes with optional ASCII decoding,
Hexenly's core differentiator is its template overlay engine: a system
that maps structured file format definitions onto raw binary data,
visually highlighting regions, fields, and relationships so users can
read binary files the way a format specification describes them.

The project targets developers, reverse engineers, firmware analysts,
and anyone who needs to understand binary file internals without
constantly cross-referencing format documentation.

+-----------------------------------------------------------------------+
| **Key Insight**                                                       |
|                                                                       |
| Hex editors show you bytes. Hexenly shows you structure. A PNG file   |
| isn't just a stream of hex---it's a signature, then an IHDR chunk     |
| with width, height, and color type, then data chunks, then an IEND    |
| terminator. Hexenly makes that visible.                               |
+-----------------------------------------------------------------------+

1.1 Goals

- Provide a fast, responsive hex editor capable of handling
  multi-gigabyte files via memory-mapped I/O

- Implement a template overlay engine that visually annotates raw hex
  with structural meaning

- Support dynamic fields where one field's value determines the size or
  offset of subsequent fields

- Build an open, community-driven template library with a well-defined
  schema format

- Deliver a native desktop experience using egui/eframe with a dark,
  utilitarian aesthetic

- Make the codebase approachable for Rust learners and open-source
  contributors

1.2 Non-Goals (v1)

- Hex editing/writing capabilities (read-only in v1; editing is a v2
  feature)

- Diffing or comparison mode between two files

- Network protocol / live stream analysis

- Built-in disassembler or decompiler

2\. Architecture

Hexenly follows a layered architecture with clean separation between the
core engine and the GUI. This allows the template engine and file
handling logic to be used independently as a library crate.

2.1 Crate Structure

The project is organized as a Cargo workspace with three crates:

  ----------------------- ------------ ------------------------------------------
  **Crate**               **Type**     **Responsibility**

  **hexenly-core**        Library      File I/O, template engine, byte
                                       interpretation, search. No GUI
                                       dependencies.

  **hexenly-templates**   Library      Template schema definitions, parser,
                                       validator, and the built-in template
                                       collection.

  **hexenly-app**         Binary       egui-based GUI application. Depends on
                                       both library crates.
  ----------------------- ------------ ------------------------------------------

2.2 Module Layout

> hexenly/
>
> ├── Cargo.toml \# Workspace root
>
> ├── crates/
>
> │ ├── hexenly-core/
>
> │ │ ├── src/
>
> │ │ │ ├── lib.rs
>
> │ │ │ ├── file.rs \# File handle, mmap, paging
>
> │ │ │ ├── interpret.rs \# Byte interpretation (u8→u64, float, string)
>
> │ │ │ ├── search.rs \# Binary/text pattern search
>
> │ │ │ └── selection.rs \# Range selections, bookmarks
>
> │ ├── hexenly-templates/
>
> │ │ ├── src/
>
> │ │ │ ├── lib.rs
>
> │ │ │ ├── schema.rs \# Template struct definitions
>
> │ │ │ ├── parser.rs \# TOML/YAML template parser
>
> │ │ │ ├── engine.rs \# Overlay resolution engine
>
> │ │ │ ├── validator.rs \# Template lint & validation
>
> │ │ │ └── stdlib/ \# Built-in templates
>
> │ │ │ ├── png.toml
>
> │ │ │ ├── elf.toml
>
> │ │ │ ├── iso9660.toml
>
> │ │ │ └── \...
>
> │ └── hexenly-app/
>
> │ ├── src/
>
> │ │ ├── main.rs
>
> │ │ ├── app.rs \# Main eframe::App impl
>
> │ │ ├── panels/
>
> │ │ │ ├── hex_view.rs \# Hex grid rendering
>
> │ │ │ ├── inspector.rs \# Byte inspector panel
>
> │ │ │ ├── templates.rs \# Template browser sidebar
>
> │ │ │ └── structure.rs \# Structure map / tree view
>
> │ │ └── theme.rs \# Colors, spacing, fonts
>
> └── templates/ \# Community template repo
>
> ├── images/
>
> ├── archives/
>
> ├── executables/
>
> └── filesystems/

2.3 Key Dependencies

  ------------------ ------------- ---------------------------------------
  **Crate**          **Version**   **Purpose**

  eframe             0.29+         GUI framework (wraps egui with native
                                   windowing)

  egui               0.29+         Immediate-mode UI library for rendering
                                   hex grid, panels, controls

  memmap2            0.9+          Memory-mapped file I/O for handling
                                   large files efficiently

  toml               0.8+          Template file parsing (primary schema
                                   format)

  serde              1.0           Serialization/deserialization for
                                   template schemas and config

  thiserror          2.0+          Ergonomic error type definitions

  tracing            0.1           Structured logging and diagnostics
  ------------------ ------------- ---------------------------------------

3\. Template Engine

The template engine is Hexenly's core differentiator. It takes a
template definition and a byte buffer, resolves all field positions
(including dynamic sizes), and produces a flat list of resolved regions
that the GUI can render as overlays.

3.1 Template Schema Format

Templates are defined in TOML files. TOML was chosen over YAML or JSON
for readability, inline tables, and the Rust ecosystem's excellent TOML
support. Each template file describes one file format.

**Example: PNG Template (**png.toml**)**

> \[template\]
>
> name = \"PNG\"
>
> description = \"Portable Network Graphics\"
>
> extensions = \[\"png\"\]
>
> magic = \[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A\]
>
> endian = \"big\"
>
> \[\[region\]\]
>
> id = \"signature\"
>
> label = \"PNG Signature\"
>
> offset = 0
>
> length = 8
>
> color = \"#2b6cb0\"
>
> description = \"Magic bytes identifying this as a PNG file\"
>
> \[\[region\]\]
>
> id = \"ihdr_chunk\"
>
> label = \"IHDR Chunk\"
>
> offset = 8
>
> color = \"#6b46c1\"
>
> description = \"Image header with dimensions and color info\"
>
> \[\[region.field\]\]
>
> id = \"ihdr_length\"
>
> label = \"Data Length\"
>
> offset = 0 \# relative to parent region
>
> length = 4
>
> type = \"u32\"
>
> role = \"size\" \# marks this as a size field
>
> size_target = \"ihdr_data\"
>
> \[\[region.field\]\]
>
> id = \"ihdr_type\"
>
> label = \"Chunk Type\"
>
> offset = 4
>
> length = 4
>
> type = \"ascii\"
>
> \[\[region.field\]\]
>
> id = \"ihdr_data\"
>
> label = \"IHDR Data\"
>
> offset = 8
>
> length = \"ihdr_length\" \# dynamic: resolved from size field
>
> description = \"Width, height, bit depth, color type, etc.\"
>
> \[\[region.field\]\]
>
> id = \"ihdr_crc\"
>
> label = \"CRC-32\"
>
> offset_after = \"ihdr_data\" \# positioned after dynamic field
>
> length = 4
>
> type = \"u32\"

3.2 Schema Data Model

The Rust structs that back the template schema:

> pub struct Template {
>
> pub name: String,
>
> pub description: String,
>
> pub extensions: Vec\<String\>,
>
> pub magic: Option\<Vec\<u8\>\>, // Magic bytes for auto-detection
>
> pub endian: Endianness, // Default endianness
>
> pub regions: Vec\<Region\>,
>
> }
>
> pub struct Region {
>
> pub id: String,
>
> pub label: String,
>
> pub offset: OffsetExpr, // Static or computed
>
> pub length: LengthExpr, // Static or dynamic
>
> pub color: Color,
>
> pub description: Option\<String\>,
>
> pub group: Option\<String\>,
>
> pub fields: Vec\<Field\>,
>
> }
>
> pub struct Field {
>
> pub id: String,
>
> pub label: String,
>
> pub offset: OffsetExpr, // Relative to parent region
>
> pub length: LengthExpr,
>
> pub field_type: Option\<FieldType\>, // u8, u16, u32, u64, ascii,
> utf8, bytes
>
> pub role: Option\<FieldRole\>, // size, offset, count, magic
>
> pub size_target: Option\<String\>, // ID of field this sizes
>
> pub description: Option\<String\>,
>
> }
>
> pub enum OffsetExpr {
>
> Static(u64),
>
> AfterField(String), // offset_after = \"field_id\"
>
> FromField(String), // offset read from another field value
>
> }
>
> pub enum LengthExpr {
>
> Static(u64),
>
> FromField(String), // length = \"field_id\" (dynamic size)
>
> ToEnd, // extends to end of file
>
> }
>
> pub enum FieldType { U8, U16, U32, U64, I8, I16, I32, I64, F32, F64,
> Ascii, Utf8, Bytes }
>
> pub enum FieldRole { Size, Offset, Count, Magic }

3.3 Resolution Engine

The resolution engine is responsible for taking a parsed template and
actual file bytes, then producing a list of ResolvedRegion structs with
concrete, absolute offsets and lengths. This is the step where dynamic
sizes and pointer-based offsets get evaluated.

**Resolution Algorithm:**

1.  Parse the TOML template into the Template struct via serde.

2.  For each region, evaluate its OffsetExpr to get an absolute byte
    offset.

3.  For each field within the region, evaluate offset (relative to
    region start) and length.

4.  If a field has role = \"size\", read the actual bytes at that
    field's position, interpret them as the declared type (e.g. u32
    big-endian), and store the value.

5.  If another field has length = \"field_id\", look up the stored size
    value and use it as that field's length.

6.  If a field has offset_after = \"field_id\", compute its start
    position as the end of the referenced field.

7.  Emit a flat Vec\<ResolvedRegion\> with all offsets absolute and all
    lengths concrete.

+-----------------------------------------------------------------------+
| **Dynamic Size Fields**                                               |
|                                                                       |
| This is what enables Hexenly to handle real-world formats. In PNG,    |
| the chunk length field tells you how many bytes of data follow. In    |
| ELF, the program header count tells you how many entries to expect.   |
| The template engine resolves these at runtime by reading actual byte  |
| values.                                                               |
+-----------------------------------------------------------------------+

3.4 Repeating Structures

Many formats have repeating elements: PNG has a sequence of chunks, ELF
has arrays of section headers, ZIP has a series of local file entries.
The template schema supports this via a repeat directive on regions:

> \[\[region\]\]
>
> id = \"png_chunk\"
>
> label = \"Chunk\"
>
> repeat = \"until_magic\" \# or \"count\" or \"until_eof\"
>
> repeat_until = \[0x49, 0x45, 0x4E, 0x44\] \# stop at IEND
>
> \# Or count-based:
>
> repeat = \"count\"
>
> repeat_count = \"sh_count\" \# references a field value

3.5 Auto-Detection

When a file is opened, Hexenly reads the first 16 bytes and checks them
against all loaded templates' magic arrays. If a match is found, that
template is automatically applied. The user can always override the
selection or apply no template.

4\. File Handling

Hexenly must handle files ranging from a few bytes (a tiny icon) to
multiple gigabytes (a disk image or ISO). The file handling layer uses
memory-mapped I/O to avoid loading entire files into memory.

4.1 Memory-Mapped I/O

The memmap2 crate provides safe memory-mapped file access. The file
appears as a contiguous byte slice (&\[u8\]) but pages are loaded on
demand by the OS. This gives us O(1) random access to any offset without
buffering.

> pub struct HexFile {
>
> mmap: Mmap, // Memory-mapped file data
>
> path: PathBuf,
>
> size: u64,
>
> cursor: u64, // Current viewport position
>
> }
>
> impl HexFile {
>
> pub fn open(path: &Path) -\> Result\<Self\> { \... }
>
> pub fn bytes(&self) -\> &\[u8\] { &self.mmap }
>
> pub fn read_u32_be(&self, offset: u64) -\> Option\<u32\> { \... }
>
> pub fn slice(&self, offset: u64, len: u64) -\> Option\<&\[u8\]\> {
> \... }
>
> }

4.2 Viewport & Paging

The hex view only renders the visible portion of the file. The viewport
tracks the current scroll position and renders rows on demand. For a
16-column layout, each row is 16 bytes, so a visible area of 40 rows
only needs 640 bytes rendered at a time, regardless of total file size.

5\. GUI Design

The interface follows an industrial, utilitarian aesthetic with a dark
color scheme. The layout consists of four primary panels, each
independently toggleable.

5.1 Layout

  ---------------- ------------------------------------------------------
  **Panel**        **Description**

  **Template       Left sidebar. Search and select templates from the
  Browser**        library. Shows loaded template details, region list,
                   and import/create buttons.

  **Hex View**     Center main area. Offset gutter on the left, hex byte
                   grid in the center, text decode pane on the right.
                   Template overlays render inline with colored
                   backgrounds and annotation labels above region starts.

  **Inspector**    Right sidebar. Shows detailed interpretation of the
                   selected byte: hex, decimal, binary, octal,
                   signed/unsigned, ASCII, multi-byte values (U16/U32 LE
                   and BE). When a template is active, also shows the
                   template context---which region and field the byte
                   belongs to, with description.

  **Structure      Bottom of inspector or standalone bottom panel. Visual
  Map**            table of contents showing all template regions as a
                   navigable list. Clicking a region scrolls the hex view
                   to that offset.
  ---------------- ------------------------------------------------------

5.2 Template Overlay Rendering

Template overlays render directly in the hex grid as subtle colored
backgrounds on bytes that belong to a recognized region. The design
principles are:

- Soft, semi-transparent background colors (approximately 15% opacity)
  so hex values remain legible

- Region labels appear as small annotation tags above the first byte of
  each region, with a colored dot indicator

- Clicking a byte in a colored region highlights all bytes in that
  region with a slightly stronger tint

- Size fields (role = \"size\") display a small lightning bolt icon to
  indicate they control the length of another field

- The text decode column also uses the region's color at reduced
  opacity, creating a visual bridge between hex and decoded text

5.3 Hex Grid Detail

The hex grid uses color coding to help users quickly scan byte patterns:

- **Zero bytes (0x00):** Dimmed/dark, nearly invisible. Large zero runs
  are visually collapsed.

- **0xFF bytes:** Highlighted in red. Often indicates uninitialized or
  erased flash memory.

- **Printable ASCII (0x20--0x7E):** Tinted in a light accent color to
  make embedded strings visible at a glance.

- **Other bytes:** Default foreground color.

5.4 Toolbar Controls

The top toolbar provides:

- **Column count selector** (8, 16, 24, 32) --- adjusts how many bytes
  per row

- **Text encoding toggle** (ASCII, UTF-8, Hex) --- changes the
  right-side text decode

- **Panel visibility toggles** --- show/hide template browser,
  inspector, and structure map

- **Go to offset** --- jump to a specific byte offset (accepts hex or
  decimal input)

- **Search** --- find hex patterns or text strings in the file

6\. Template Library

A major goal of Hexenly is building an open-source, community-driven
template library. Templates live in a dedicated directory structure
organized by category.

6.1 Directory Structure

> templates/
>
> ├── images/
>
> │ ├── png.toml
>
> │ ├── jpeg.toml
>
> │ ├── gif.toml
>
> │ ├── bmp.toml
>
> │ └── tiff.toml
>
> ├── executables/
>
> │ ├── elf.toml
>
> │ ├── pe.toml
>
> │ └── macho.toml
>
> ├── archives/
>
> │ ├── zip.toml
>
> │ ├── tar.toml
>
> │ └── gzip.toml
>
> ├── filesystems/
>
> │ ├── iso9660.toml
>
> │ ├── fat32.toml
>
> │ ├── ext4.toml
>
> │ └── cybiko.toml
>
> ├── firmware/
>
> │ ├── uefi.toml
>
> │ └── intel_hex.toml
>
> └── protocols/
>
> ├── pcap.toml
>
> └── protobuf.toml

6.2 Template Metadata & Validation

Every template must pass validation before being accepted into the
library. The validator checks:

- All field IDs are unique within their region

- All size_target and offset_after references point to valid field IDs

- No circular dependencies in dynamic size/offset chains

- Magic bytes, if declared, match the expected format

- Color values are valid hex colors

- Required metadata fields (name, description, extensions) are present

Run validation via CLI: hexenly validate templates/images/png.toml

6.3 Contribution Workflow

The template library is hosted in the same Git repository as the
application. Contributors follow a standard pull request workflow:

8.  Fork the repository and create a new template TOML file in the
    appropriate category directory.

9.  Run hexenly validate \<template.toml\> to verify the template passes
    all checks.

10. Include at least one sample file (or reference to a freely available
    test file) for integration testing.

11. Submit a PR with the template and a brief description of the format.

CI runs the validator automatically and checks that the template
resolves against the provided sample file without errors.

7\. Implementation Plan

The project is structured in milestones that build incrementally,
allowing each phase to produce a working (if limited) application.

  ----------- --------------------- ---------------------------------------
  **Phase**   **Milestone**         **Deliverables**

  **Phase 1** Core Hex Viewer       File open (mmap), hex grid rendering
                                    with configurable columns, ASCII/UTF-8
                                    text pane, byte inspector panel,
                                    go-to-offset, basic search. No
                                    templates yet.

  **Phase 2** Template Engine       TOML template parser, schema structs,
                                    resolution engine with static
                                    offsets/lengths, overlay rendering in
                                    hex view, template browser sidebar,
                                    auto-detection via magic bytes. Ship
                                    with 3--5 built-in templates (PNG, ELF,
                                    ZIP).

  **Phase 3** Dynamic Templates     Dynamic size fields, offset_after
                                    resolution, repeating structures,
                                    pointer-following (field value as
                                    offset). Structure map/tree view. Ship
                                    with ISO 9660 and FAT32 templates that
                                    exercise these features.

  **Phase 4** Community & Polish    Template validator CLI, contribution
                                    docs, CI pipeline, user-customizable
                                    template directory,
                                    bookmarks/annotations, keyboard
                                    shortcuts, cross-platform packaging
                                    (Windows, macOS, Linux).

  **Phase 5** Editing (v2)          Read-write mode with undo/redo,
                                    insert/overwrite toggle, copy/paste hex
                                    or text, save/save-as, diff view
                                    between original and modified bytes.
  ----------- --------------------- ---------------------------------------

8\. Example Use Case: Cybiko Filesystem

As a concrete example of Hexenly's value, consider debugging a corrupt
Cybiko filesystem image. Today, this requires opening the image in a
generic hex editor like HexFiend, manually counting offsets against a
format specification, and placing bookmarks byte by byte.

**With Hexenly and a** cybiko.toml **template:**

- Open the .img file and Hexenly auto-detects the Cybiko filesystem via
  magic bytes

- The header region lights up immediately: magic number, version, block
  size, total/free block counts

- The FAT offset field is a pointer --- the engine reads its value and
  highlights the FAT region at the correct position

- Each directory entry is a repeating structure; Hexenly resolves the
  count from the max_files header field and highlights each entry

- A corrupt file shows the problem visually: if the FAT length field
  claims 4096 bytes but the data ends at 3000, the template overlay
  shows the gap in red, making the corruption immediately obvious

This is the workflow Hexenly enables for any binary format: open, see
structure, understand, diagnose.

9\. Open Questions

  -------- --------------------------- ---------------------------------------
  **\#**   **Question**                **Options / Notes**

  1        Should templates support    Adds complexity but needed for formats
           conditional fields (show    like PE where structure varies by flag
           field X only if field Y     values. Could defer to Phase 3 or 4.
           equals Z)?                  

  2        TOML vs. a custom DSL for   TOML is familiar and has great Rust
           template definitions?       support, but deeply nested structures
                                       get verbose. A custom DSL could be more
                                       expressive but adds a parser to
                                       maintain. Recommend starting with TOML
                                       and evaluating later.

  3        Should we support           E.g., showing \"Color Type: RGBA\"
           computed/display values in  instead of raw 0x06 in PNG. Requires an
           templates?                  enum/mapping table in the template
                                       schema. High user value.

  4        egui font rendering quality Need to prototype early. If egui's text
           for dense hex grids?        rendering isn't crisp enough at small
                                       sizes for hex grids, may need custom
                                       glyph rendering or a different
                                       framework.

  5        Cross-platform template     Use dirs crate for platform-appropriate
           directory locations?        paths (\~/.config/hexenly/ on Linux,
                                       \~/Library/Application Support/hexenly/
                                       on macOS, %APPDATA%/hexenly/ on
                                       Windows).
  -------- --------------------------- ---------------------------------------

*End of Document*
