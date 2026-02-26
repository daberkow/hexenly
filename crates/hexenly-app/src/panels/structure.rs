//! Structure tree panel — shows resolved template regions and fields with navigation.

use egui::{self, Color32, Grid, RichText, ScrollArea, Ui};

use crate::app::TemplateLayer;
use crate::theme::monospace_font;

/// An interaction returned from the structure panel.
#[derive(Debug)]
pub enum StructureAction {
    GoToOffset(usize),
    Close,
}

/// Render the structure panel. Returns an action if the user navigated or closed.
pub fn show(
    ui: &mut Ui,
    layers: &[TemplateLayer],
    cursor_offset: usize,
) -> Option<StructureAction> {
    let mut action = None;

    ui.horizontal(|ui| {
        ui.heading("Structure");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("x").clicked() {
                action = Some(StructureAction::Close);
            }
        });
    });
    ui.separator();

    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (i, layer) in layers.iter().enumerate() {
                let resolved = &layer.resolved;
                let header_text = format!("{} @ 0x{:X}", resolved.name, layer.base_offset);
                egui::CollapsingHeader::new(RichText::new(header_text).strong())
                    .id_salt(format!("layer_{}", i))
                    .default_open(i == 0)
                    .show(ui, |ui| {
                        for region in &resolved.regions {
                            let color = Color32::from_rgb(region.color.r, region.color.g, region.color.b);
                            let cursor_in_region = region.contains(cursor_offset as u64);

                            let region_header = egui::CollapsingHeader::new(
                                RichText::new(format!(
                                    "* {}  [0x{:X}..0x{:X}]  {} bytes",
                                    region.label,
                                    region.offset,
                                    region.end_exclusive(),
                                    region.length,
                                ))
                                .color(color),
                            )
                            .id_salt(format!("layer_{}_region_{}", i, region.id))
                            .default_open(cursor_in_region);

                            let response = region_header.show(ui, |ui| {
                                Grid::new(format!("structure_{}_{}", i, region.id))
                                    .num_columns(4)
                                    .striped(true)
                                    .spacing([6.0, 2.0])
                                    .show(ui, |ui| {
                                        // Header row
                                        ui.label(RichText::new("Field").strong());
                                        ui.label(RichText::new("Offset").strong());
                                        ui.label(RichText::new("Size").strong());
                                        ui.label(RichText::new("Value").strong());
                                        ui.end_row();

                                        for field in &region.fields {
                                            let label_text = if let Some(fc) = &field.color {
                                                RichText::new(&field.label)
                                                    .font(monospace_font())
                                                    .color(Color32::from_rgb(fc.r, fc.g, fc.b))
                                            } else {
                                                RichText::new(&field.label)
                                                    .font(monospace_font())
                                            };
                                            let field_label = ui.selectable_label(false, label_text);
                                            if field_label.clicked() {
                                                action = Some(StructureAction::GoToOffset(field.offset as usize));
                                            }
                                            if let Some(desc) = &field.description {
                                                field_label.on_hover_text(desc);
                                            }

                                            let offset_label = ui.selectable_label(
                                                false,
                                                RichText::new(format!("0x{:X}", field.offset))
                                                    .font(monospace_font()),
                                            );
                                            if offset_label.clicked() {
                                                action = Some(StructureAction::GoToOffset(field.offset as usize));
                                            }
                                            ui.label(
                                                RichText::new(field.length.to_string())
                                                    .font(monospace_font()),
                                            );
                                            if let Some(computed) = field.computed_value {
                                                if ui.link(
                                                    RichText::new(&field.display_value)
                                                        .font(monospace_font()),
                                                ).clicked() {
                                                    action = Some(StructureAction::GoToOffset(computed as usize));
                                                }
                                            } else {
                                                ui.label(
                                                    RichText::new(&field.display_value)
                                                        .font(monospace_font()),
                                                );
                                            }
                                            ui.end_row();
                                        }
                                    });
                            });

                            // Click region header to go to its start offset
                            if response.header_response.clicked() {
                                action = Some(StructureAction::GoToOffset(region.offset as usize));
                            }
                        }
                    });
            }
        });

    action
}
