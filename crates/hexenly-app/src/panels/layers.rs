use egui::Ui;

use crate::app::{LayerSource, TemplateLayer};

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
