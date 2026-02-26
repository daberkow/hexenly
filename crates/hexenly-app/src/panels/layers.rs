use egui::Ui;

use crate::app::{LayerSource, TemplateLayer};

pub enum LayerAction {
    Remove(usize),
    GoToOffset(u64),
}

/// Compute the tree indent level for a layer by walking its `LinkedFrom` chain.
fn compute_indent(layers: &[TemplateLayer], index: usize) -> usize {
    match &layers[index].source {
        LayerSource::AutoDetected | LayerSource::Manual => 0,
        LayerSource::LinkedFrom(source_field_id) => {
            for (pi, parent) in layers.iter().enumerate() {
                if pi == index {
                    continue;
                }
                let has_field = parent
                    .resolved
                    .regions
                    .iter()
                    .flat_map(|r| r.fields.iter())
                    .any(|f| f.id == *source_field_id);
                if has_field {
                    return 1 + compute_indent(layers, pi);
                }
            }
            0
        }
    }
}

pub fn show(ui: &mut Ui, layers: &[TemplateLayer]) -> Option<LayerAction> {
    let mut action = None;

    if layers.is_empty() {
        ui.label("No templates applied");
        return None;
    }

    for (i, layer) in layers.iter().enumerate() {
        let indent = compute_indent(layers, i);
        let source_label = match &layer.source {
            LayerSource::AutoDetected => "auto",
            LayerSource::Manual => "manual",
            LayerSource::LinkedFrom(_) => "linked",
        };

        ui.horizontal(|ui| {
            if indent > 0 {
                ui.add_space(indent as f32 * 16.0);
                ui.label("|_");
            }

            if ui
                .small_button("x")
                .on_hover_text("Remove layer")
                .clicked()
            {
                action = Some(LayerAction::Remove(i));
            }

            let offset_text = format!("0x{:X}", layer.base_offset);
            if ui
                .link(&offset_text)
                .on_hover_text("Go to offset")
                .clicked()
            {
                action = Some(LayerAction::GoToOffset(layer.base_offset));
            }

            ui.label(format!("{} ({})", layer.resolved.name, source_label))
                .on_hover_text(&layer.resolved.description);
        });
    }

    action
}
