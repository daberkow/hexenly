use egui::{Color32, Grid, RichText, ScrollArea, Ui};
use hexenly_templates::resolved::ResolvedTemplate;

use crate::theme::monospace_font;

#[derive(Debug)]
pub enum StructureAction {
    GoToOffset(usize),
}

pub fn show(
    ui: &mut Ui,
    resolved: &ResolvedTemplate,
    cursor_offset: usize,
) -> Option<StructureAction> {
    let mut action = None;

    ui.horizontal(|ui| {
        ui.heading("Structure");
        ui.label(RichText::new(&resolved.name).strong());
    });
    ui.separator();

    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for region in &resolved.regions {
                let color = Color32::from_rgb(region.color.r, region.color.g, region.color.b);
                let cursor_in_region = region.contains(cursor_offset as u64);

                let header = egui::CollapsingHeader::new(
                    RichText::new(format!(
                        "\u{25CF} {}  [0x{:X}..0x{:X}]  {} bytes",
                        region.label,
                        region.offset,
                        region.end_exclusive(),
                        region.length,
                    ))
                    .color(color),
                )
                .default_open(cursor_in_region);

                let response = header.show(ui, |ui| {
                    Grid::new(format!("structure_{}", region.id))
                        .num_columns(4)
                        .striped(true)
                        .spacing([8.0, 2.0])
                        .show(ui, |ui| {
                            // Header row
                            ui.label(RichText::new("Field").strong());
                            ui.label(RichText::new("Offset").strong());
                            ui.label(RichText::new("Size").strong());
                            ui.label(RichText::new("Value").strong());
                            ui.end_row();

                            for field in &region.fields {
                                let field_label = ui.selectable_label(false, &field.label);
                                if field_label.clicked() {
                                    action = Some(StructureAction::GoToOffset(field.offset as usize));
                                }
                                if let Some(desc) = &field.description {
                                    field_label.on_hover_text(desc);
                                }

                                ui.label(
                                    RichText::new(format!("0x{:X}", field.offset))
                                        .font(monospace_font()),
                                );
                                ui.label(
                                    RichText::new(field.length.to_string())
                                        .font(monospace_font()),
                                );
                                ui.label(
                                    RichText::new(&field.display_value)
                                        .font(monospace_font()),
                                );
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

    action
}
