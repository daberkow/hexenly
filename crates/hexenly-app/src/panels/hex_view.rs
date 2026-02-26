//! Painter-based hex grid with offset gutter, hex bytes, ASCII pane,
//! template overlay, selection, and cursor rendering.

use egui::{Color32, Pos2, Rect, ScrollArea, Sense, Stroke, StrokeKind, Ui, Vec2};
use hexenly_core::{ByteClass, Selection, classify_byte};
use hexenly_templates::resolved::ResolvedTemplate;

use crate::app::TextEncoding;
use crate::theme::{HexColors, annotation_font, monospace_font};

/// A decoded character spanning one or more bytes.
struct DecodedChar {
    /// The decoded character (or replacement).
    ch: char,
    /// Column index of the first byte in the row.
    start_col: usize,
    /// Number of bytes this character spans.
    byte_len: usize,
}

/// Decode a row of bytes according to the selected text encoding.
/// Returns a list of `DecodedChar`s covering every byte in the row.
fn decode_row(row_bytes: &[u8], encoding: TextEncoding) -> Vec<DecodedChar> {
    match encoding {
        TextEncoding::Ascii => {
            row_bytes
                .iter()
                .enumerate()
                .map(|(col, &b)| DecodedChar {
                    ch: if (0x20..=0x7E).contains(&b) { b as char } else { '.' },
                    start_col: col,
                    byte_len: 1,
                })
                .collect()
        }
        TextEncoding::Utf8 => {
            let mut result = Vec::new();
            let mut i = 0;
            while i < row_bytes.len() {
                let remaining = &row_bytes[i..];
                match std::str::from_utf8(remaining) {
                    Ok(s) => {
                        // Rest of the row is valid UTF-8
                        for ch in s.chars() {
                            let len = ch.len_utf8();
                            let display = if ch.is_control() { '.' } else { ch };
                            result.push(DecodedChar { ch: display, start_col: i, byte_len: len });
                            i += len;
                        }
                    }
                    Err(e) => {
                        // Valid portion up to the error
                        let valid_up_to = e.valid_up_to();
                        if valid_up_to > 0 {
                            let valid = std::str::from_utf8(&remaining[..valid_up_to]).unwrap();
                            for ch in valid.chars() {
                                let len = ch.len_utf8();
                                let display = if ch.is_control() { '.' } else { ch };
                                result.push(DecodedChar { ch: display, start_col: i, byte_len: len });
                                i += len;
                            }
                        } else {
                            // Invalid byte — show as dot
                            result.push(DecodedChar { ch: '.', start_col: i, byte_len: 1 });
                            i += 1;
                        }
                    }
                }
            }
            result
        }
        TextEncoding::Utf16Le | TextEncoding::Utf16Be => {
            let mut result = Vec::new();
            let mut i = 0;
            let is_le = encoding == TextEncoding::Utf16Le;
            while i < row_bytes.len() {
                if i + 1 >= row_bytes.len() {
                    // Odd trailing byte
                    result.push(DecodedChar { ch: '.', start_col: i, byte_len: 1 });
                    i += 1;
                    continue;
                }
                let unit = if is_le {
                    u16::from_le_bytes([row_bytes[i], row_bytes[i + 1]])
                } else {
                    u16::from_be_bytes([row_bytes[i], row_bytes[i + 1]])
                };
                // Check for surrogate pair
                if (0xD800..=0xDBFF).contains(&unit) {
                    // High surrogate — need low surrogate
                    if i + 3 < row_bytes.len() {
                        let low = if is_le {
                            u16::from_le_bytes([row_bytes[i + 2], row_bytes[i + 3]])
                        } else {
                            u16::from_be_bytes([row_bytes[i + 2], row_bytes[i + 3]])
                        };
                        if (0xDC00..=0xDFFF).contains(&low) {
                            let cp = 0x10000 + ((unit as u32 - 0xD800) << 10) + (low as u32 - 0xDC00);
                            let ch = char::from_u32(cp).unwrap_or('.');
                            let display = if ch.is_control() { '.' } else { ch };
                            result.push(DecodedChar { ch: display, start_col: i, byte_len: 4 });
                            i += 4;
                        } else {
                            // Invalid surrogate pair
                            result.push(DecodedChar { ch: '.', start_col: i, byte_len: 2 });
                            i += 2;
                        }
                    } else {
                        // Not enough bytes for surrogate pair
                        result.push(DecodedChar { ch: '.', start_col: i, byte_len: 2 });
                        i += 2;
                    }
                } else if (0xDC00..=0xDFFF).contains(&unit) {
                    // Lone low surrogate
                    result.push(DecodedChar { ch: '.', start_col: i, byte_len: 2 });
                    i += 2;
                } else {
                    let ch = char::from_u32(unit as u32).unwrap_or('.');
                    let display = if ch.is_control() { '.' } else { ch };
                    result.push(DecodedChar { ch: display, start_col: i, byte_len: 2 });
                    i += 2;
                }
            }
            result
        }
    }
}

/// Persistent state for the hex view across frames.
#[derive(Default)]
pub struct HexViewState {
    /// Scroll to this row on next frame, then clear.
    pub scroll_to_row: Option<usize>,
    /// Byte offset and pane where the current drag started.
    drag_start: Option<(usize, HexPane)>,
}

/// Render the hex view for the current frame. Returns an action if the user
/// clicked or dragged to set the cursor or select a range.
#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut Ui,
    data: &[u8],
    total_len: usize,
    columns: usize,
    cursor: usize,
    selection: Option<&Selection>,
    search_matches: &[usize],
    show_ascii: bool,
    state: &mut HexViewState,
    template_overlay: &[&ResolvedTemplate],
    nibble_high: bool,
    edit_focus: HexPane,
    colors: &HexColors,
    text_encoding: TextEncoding,
) -> Option<HexViewAction> {
    let mut action = None;
    let font = monospace_font();

    // Measure character dimensions using the monospace font.
    let char_width = ui.fonts_mut(|f| f.glyph_width(&font, '0'));
    let row_height = ui.fonts_mut(|f| f.row_height(&font));
    let line_height = row_height + 2.0;

    // Column layout widths
    let offset_chars = 10; // "00000000: "
    let offset_width = char_width * offset_chars as f32;
    let hex_col_width = char_width * 3.0; // "XX "
    let hex_total_width = hex_col_width * columns as f32;
    let gap = char_width * 2.0;
    let ascii_width = if show_ascii {
        char_width * columns as f32 + gap
    } else {
        0.0
    };
    let total_row_width = offset_width + hex_total_width + ascii_width + char_width;

    let row_count = total_len.div_ceil(columns);

    let scroll_to = state.scroll_to_row.take();

    ui.spacing_mut().item_spacing.y = 0.0;

    let output = ScrollArea::vertical()
        .auto_shrink([false, false])
        .show_rows(ui, line_height, row_count, |ui, row_range| {
            let content_height = (row_range.end - row_range.start) as f32 * line_height;
            let (response, painter) = ui.allocate_painter(
                Vec2::new(
                    total_row_width.max(ui.available_width()),
                    content_height.max(ui.available_height()),
                ),
                Sense::click_and_drag(),
            );
            let origin = response.rect.min;

            for (visual_idx, row) in row_range.clone().enumerate() {
                let y = origin.y + visual_idx as f32 * line_height;
                let row_offset = row * columns;
                let row_end = (row_offset + columns).min(data.len());
                let row_bytes = if row_offset < data.len() {
                    &data[row_offset..row_end]
                } else {
                    &[]
                };

                // --- Offset gutter ---
                let offset_text = format!("{:08X}:", row_offset);
                painter.text(
                    Pos2::new(origin.x, y),
                    egui::Align2::LEFT_TOP,
                    &offset_text,
                    font.clone(),
                    colors.offset_column,
                );

                let hex_x_start = origin.x + offset_width;
                let ascii_x_start = hex_x_start + hex_total_width + gap;

                for (col, &byte) in row_bytes.iter().enumerate() {
                    let byte_offset = row_offset + col;

                    let hex_rect = Rect::from_min_size(
                        Pos2::new(hex_x_start + col as f32 * hex_col_width, y),
                        Vec2::new(hex_col_width, line_height),
                    );

                    // --- Template overlay background (painted first, lowest layer) ---
                    // Last layer wins for overlapping regions.
                    if !template_overlay.is_empty() {
                        let mut bg_color = None;

                        for overlay in template_overlay {
                            for region in &overlay.regions {
                                if region.contains(byte_offset as u64) {
                                    // Use field color if the byte falls in a field with a color,
                                    // otherwise fall back to region color.
                                    let c = region.fields.iter()
                                        .find(|f| byte_offset as u64 >= f.offset && (byte_offset as u64) < f.offset + f.length)
                                        .and_then(|f| f.color.as_ref())
                                        .unwrap_or(&region.color);
                                    bg_color = Some(Color32::from_rgba_unmultiplied(c.r, c.g, c.b, 38));
                                }
                            }
                        }

                        if let Some(bg) = bg_color {
                            painter.rect_filled(hex_rect, 0.0, bg);

                            if show_ascii {
                                let ascii_rect = Rect::from_min_size(
                                    Pos2::new(
                                        ascii_x_start + col as f32 * char_width,
                                        y,
                                    ),
                                    Vec2::new(char_width, line_height),
                                );
                                painter.rect_filled(ascii_rect, 0.0, bg);
                            }
                        }
                    }

                    // --- Cursor / selection / search background highlights ---
                    let is_cursor = byte_offset == cursor;
                    let is_selected =
                        selection.is_some_and(|sel| sel.contains(byte_offset));
                    let is_search_hit = search_matches.contains(&byte_offset);

                    if is_cursor {
                        painter.rect_filled(hex_rect, 0.0, colors.cursor_bg);
                        painter.rect_stroke(hex_rect, 0.0, Stroke::new(1.0, colors.cursor_border), StrokeKind::Inside);
                        if edit_focus == HexPane::Hex {
                            let nibble_x = if nibble_high {
                                hex_x_start + col as f32 * hex_col_width
                            } else {
                                hex_x_start + col as f32 * hex_col_width + char_width
                            };
                            let underline_y = y + row_height;
                            painter.line_segment(
                                [
                                    Pos2::new(nibble_x, underline_y),
                                    Pos2::new(nibble_x + char_width, underline_y),
                                ],
                                Stroke::new(2.0, colors.cursor_border),
                            );
                        }
                    } else if is_selected {
                        painter.rect_filled(hex_rect, 0.0, colors.selection_bg);
                    } else if is_search_hit {
                        painter.rect_filled(hex_rect, 0.0, colors.search_highlight);
                    }

                    // --- Hex byte text ---
                    let color = byte_color(byte, colors);
                    let hex_text = format!("{:02X}", byte);
                    painter.text(
                        Pos2::new(hex_x_start + col as f32 * hex_col_width, y),
                        egui::Align2::LEFT_TOP,
                        &hex_text,
                        font.clone(),
                        color,
                    );

                }

                // --- ASCII pane (encoding-aware) ---
                if show_ascii {
                    let decoded = decode_row(row_bytes, text_encoding);
                    for dc in &decoded {
                        let span_width = char_width * dc.byte_len as f32;
                        let x = ascii_x_start + dc.start_col as f32 * char_width;

                        // Background highlights for each byte in the span
                        for b in 0..dc.byte_len {
                            let col = dc.start_col + b;
                            let byte_offset = row_offset + col;
                            let bx = ascii_x_start + col as f32 * char_width;
                            let byte_rect = Rect::from_min_size(
                                Pos2::new(bx, y),
                                Vec2::new(char_width, line_height),
                            );
                            let is_cursor = byte_offset == cursor;
                            let is_selected = selection.is_some_and(|sel| sel.contains(byte_offset));
                            if is_cursor {
                                painter.rect_filled(byte_rect, 0.0, colors.cursor_bg);
                                painter.rect_stroke(byte_rect, 0.0, Stroke::new(1.0, colors.cursor_border), StrokeKind::Inside);
                            } else if is_selected {
                                painter.rect_filled(byte_rect, 0.0, colors.selection_bg);
                            }
                        }

                        // Paint the character centered across its byte columns
                        let text_x = x + (span_width - char_width) / 2.0;
                        painter.text(
                            Pos2::new(text_x, y),
                            egui::Align2::LEFT_TOP,
                            dc.ch.to_string(),
                            font.clone(),
                            colors.ascii_pane,
                        );
                    }
                }

                // --- Region labels above first byte of regions starting in this row ---
                if !template_overlay.is_empty() {
                    let ann_font = annotation_font();
                    for overlay in template_overlay {
                        for region in &overlay.regions {
                            let region_start = region.offset as usize;
                            if region_start >= row_offset
                                && region_start < row_offset + columns
                            {
                                let col_in_row = region_start - row_offset;
                                let label_x = hex_x_start + col_in_row as f32 * hex_col_width;
                                let label_y = y - 1.0;
                                let label_color =
                                    Color32::from_rgb(region.color.r, region.color.g, region.color.b);
                                painter.text(
                                    Pos2::new(label_x, label_y),
                                    egui::Align2::LEFT_BOTTOM,
                                    &region.label,
                                    ann_font.clone(),
                                    label_color,
                                );
                            }
                        }
                    }
                }
            }

            let hit = |pos| {
                pos_to_offset(
                    pos, origin, offset_width, hex_col_width, char_width,
                    hex_total_width, gap, columns, line_height, &row_range, show_ascii,
                    data, text_encoding,
                )
            };

            // Handle click (no drag) — set cursor, clear selection
            if response.clicked()
                && let Some(pos) = response.interact_pointer_pos()
                && let Some((offset, pane)) = hit(pos)
            {
                action = Some(HexViewAction::SetCursor(offset, pane));
            }

            // Handle drag — select byte range
            if response.drag_started()
                && let Some(pos) = response.interact_pointer_pos()
            {
                state.drag_start = hit(pos);
            }
            if (response.dragged() || response.drag_stopped())
                && let Some((start, pane)) = state.drag_start
                && let Some(pos) = response.interact_pointer_pos()
                && let Some((end, _)) = hit(pos)
            {
                action = Some(HexViewAction::Select { start, end, pane });
            }
            if response.drag_stopped() {
                state.drag_start = None;
            }

            // Right-click context menu
            response.context_menu(|ui| {
                ui.label(format!("Offset: 0x{:X}", cursor));
                ui.separator();
                if ui.button("Apply template here...").clicked() {
                    action = Some(HexViewAction::ApplyTemplateAt(cursor as u64));
                    ui.close();
                }
            });
        });

    if let Some(target_row) = scroll_to {
        let target_y = target_row as f32 * line_height;
        let mut scroll_state = output.state;
        scroll_state.offset.y = target_y;
        scroll_state.store(ui.ctx(), output.id);
        ui.ctx().request_repaint();
    }

    action
}

/// Convert a screen position to a byte offset and which pane was hit.
#[allow(clippy::too_many_arguments)]
fn pos_to_offset(
    pos: Pos2,
    origin: Pos2,
    offset_width: f32,
    hex_col_width: f32,
    char_width: f32,
    hex_total_width: f32,
    gap: f32,
    columns: usize,
    line_height: f32,
    row_range: &std::ops::Range<usize>,
    show_ascii: bool,
    data: &[u8],
    text_encoding: TextEncoding,
) -> Option<(usize, HexPane)> {
    let rel_x = pos.x - origin.x;
    let rel_y = pos.y - origin.y;

    if rel_y < 0.0 {
        return None;
    }

    let visual_row = (rel_y / line_height) as usize;
    let row = row_range.start + visual_row;
    if row >= row_range.end {
        return None;
    }

    // Check hex area
    let hex_rel_x = rel_x - offset_width;
    if hex_rel_x >= 0.0 && hex_rel_x < hex_total_width {
        let col = (hex_rel_x / hex_col_width) as usize;
        if col < columns {
            return Some((row * columns + col, HexPane::Hex));
        }
    }

    // Check ASCII area — snap to first byte of multi-byte character
    if show_ascii {
        let ascii_x_start = offset_width + hex_total_width + gap;
        let ascii_rel_x = rel_x - ascii_x_start;
        if ascii_rel_x >= 0.0 {
            let col = (ascii_rel_x / char_width) as usize;
            if col < columns {
                let row_offset = row * columns;
                let row_end = (row_offset + columns).min(data.len());
                if row_offset < data.len() {
                    let row_bytes = &data[row_offset..row_end];
                    let decoded = decode_row(row_bytes, text_encoding);
                    // Find which decoded char contains this column
                    for dc in &decoded {
                        if col >= dc.start_col && col < dc.start_col + dc.byte_len {
                            return Some((row_offset + dc.start_col, HexPane::Ascii));
                        }
                    }
                }
                return Some((row * columns + col, HexPane::Ascii));
            }
        }
    }

    None
}

/// Map a byte to its display color based on its classification.
fn byte_color(byte: u8, colors: &HexColors) -> Color32 {
    match classify_byte(byte) {
        ByteClass::Null => colors.null_byte,
        ByteClass::MaxByte => colors.max_byte,
        ByteClass::PrintableAscii => colors.printable_ascii,
        ByteClass::Other => colors.default_byte,
    }
}

/// Which sub-pane of the hex view: the hex byte grid or the ASCII column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HexPane {
    Hex,
    Ascii,
}

/// An interaction returned from the hex view to the main app loop.
#[derive(Debug)]
pub enum HexViewAction {
    SetCursor(usize, HexPane),
    Select { start: usize, end: usize, pane: HexPane },
    ApplyTemplateAt(u64),
}
