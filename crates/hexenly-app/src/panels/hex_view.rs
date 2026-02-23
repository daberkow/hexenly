use egui::{Color32, Pos2, Rect, ScrollArea, Sense, Stroke, StrokeKind, Ui, Vec2};
use hexenly_core::{ByteClass, HexFile, Selection, classify_byte};
use hexenly_templates::resolved::ResolvedTemplate;

use crate::theme::{HexColors, annotation_font, monospace_font};

#[derive(Default)]
pub struct HexViewState {
    /// Scroll to this row on next frame, then clear.
    pub scroll_to_row: Option<usize>,
    /// Byte offset and pane where the current drag started.
    drag_start: Option<(usize, HexPane)>,
}

#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut Ui,
    file: &HexFile,
    columns: usize,
    cursor: usize,
    selection: Option<&Selection>,
    search_matches: &[usize],
    show_ascii: bool,
    state: &mut HexViewState,
    template_overlay: Option<&ResolvedTemplate>,
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

    let row_count = file.row_count(columns);

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
                let row_bytes = file.read_row(row, columns);

                // --- Offset gutter ---
                let offset_text = format!("{:08X}:", row_offset);
                painter.text(
                    Pos2::new(origin.x, y),
                    egui::Align2::LEFT_TOP,
                    &offset_text,
                    font.clone(),
                    HexColors::OFFSET_COLUMN,
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
                    if let Some(tmpl) = template_overlay {
                        for region in &tmpl.regions {
                            if region.contains(byte_offset as u64) {
                                // Use field color if the byte falls in a field with a color,
                                // otherwise fall back to region color.
                                let c = region.fields.iter()
                                    .find(|f| byte_offset as u64 >= f.offset && (byte_offset as u64) < f.offset + f.length)
                                    .and_then(|f| f.color.as_ref())
                                    .unwrap_or(&region.color);
                                let bg = Color32::from_rgba_unmultiplied(c.r, c.g, c.b, 38);
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
                                break;
                            }
                        }
                    }

                    // --- Cursor / selection / search background highlights ---
                    let is_cursor = byte_offset == cursor;
                    let is_selected =
                        selection.is_some_and(|sel| sel.contains(byte_offset));
                    let is_search_hit = search_matches.contains(&byte_offset);

                    if is_cursor {
                        painter.rect_filled(hex_rect, 0.0, HexColors::CURSOR_BG);
                        painter.rect_stroke(hex_rect, 0.0, Stroke::new(1.0, HexColors::CURSOR_BORDER), StrokeKind::Inside);
                    } else if is_selected {
                        painter.rect_filled(hex_rect, 0.0, HexColors::SELECTION_BG);
                    } else if is_search_hit {
                        painter.rect_filled(hex_rect, 0.0, HexColors::SEARCH_HIGHLIGHT);
                    }

                    // --- Hex byte text ---
                    let color = byte_color(byte);
                    let hex_text = format!("{:02X}", byte);
                    painter.text(
                        Pos2::new(hex_x_start + col as f32 * hex_col_width, y),
                        egui::Align2::LEFT_TOP,
                        &hex_text,
                        font.clone(),
                        color,
                    );

                    // --- ASCII pane ---
                    if show_ascii {
                        let ascii_rect = Rect::from_min_size(
                            Pos2::new(ascii_x_start + col as f32 * char_width, y),
                            Vec2::new(char_width, line_height),
                        );
                        if is_cursor {
                            painter.rect_filled(ascii_rect, 0.0, HexColors::CURSOR_BG);
                            painter.rect_stroke(ascii_rect, 0.0, Stroke::new(1.0, HexColors::CURSOR_BORDER), StrokeKind::Inside);
                        } else if is_selected {
                            painter.rect_filled(ascii_rect, 0.0, HexColors::SELECTION_BG);
                        }

                        let ch = if (0x20..=0x7E).contains(&byte) {
                            byte as char
                        } else {
                            '.'
                        };
                        painter.text(
                            Pos2::new(ascii_x_start + col as f32 * char_width, y),
                            egui::Align2::LEFT_TOP,
                            ch.to_string(),
                            font.clone(),
                            HexColors::ASCII_PANE,
                        );
                    }
                }

                // --- Region labels above first byte of regions starting in this row ---
                if let Some(tmpl) = template_overlay {
                    let ann_font = annotation_font();
                    for region in &tmpl.regions {
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

            let hit = |pos| {
                pos_to_offset(
                    pos, origin, offset_width, hex_col_width, char_width,
                    hex_total_width, gap, columns, line_height, &row_range, show_ascii,
                )
            };

            // Handle click (no drag) — set cursor, clear selection
            if response.clicked()
                && let Some(pos) = response.interact_pointer_pos()
                && let Some((offset, _pane)) = hit(pos)
            {
                action = Some(HexViewAction::SetCursor(offset));
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

    // Check ASCII area
    if show_ascii {
        let ascii_x_start = offset_width + hex_total_width + gap;
        let ascii_rel_x = rel_x - ascii_x_start;
        if ascii_rel_x >= 0.0 {
            let col = (ascii_rel_x / char_width) as usize;
            if col < columns {
                return Some((row * columns + col, HexPane::Ascii));
            }
        }
    }

    None
}

fn byte_color(byte: u8) -> Color32 {
    match classify_byte(byte) {
        ByteClass::Null => HexColors::NULL_BYTE,
        ByteClass::MaxByte => HexColors::MAX_BYTE,
        ByteClass::PrintableAscii => HexColors::PRINTABLE_ASCII,
        ByteClass::Other => HexColors::DEFAULT_BYTE,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HexPane {
    Hex,
    Ascii,
}

#[derive(Debug)]
pub enum HexViewAction {
    SetCursor(usize),
    Select { start: usize, end: usize, pane: HexPane },
}
