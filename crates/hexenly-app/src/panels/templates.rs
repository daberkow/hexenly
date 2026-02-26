//! Template browser panel — browse, filter, and select binary templates.

use egui::{self, RichText, ScrollArea, Ui};
use hexenly_templates::loader::TemplateRegistry;

/// An interaction returned from the template browser.
#[derive(Debug)]
pub enum TemplateBrowserAction {
    Select(usize),
    Deselect,
}

/// Render the template browser. Returns an action if the user selected or deselected a template.
pub fn show(
    ui: &mut Ui,
    registry: &TemplateRegistry,
    active_indices: &[usize],
    filter: &mut String,
    apply_offset: &mut String,
) -> Option<TemplateBrowserAction> {
    let mut action = None;

    ui.label(RichText::new("Template Catalog").strong());

    // Search filter
    ui.horizontal(|ui| {
        ui.label("Filter:");
        ui.text_edit_singleline(filter);
    });
    ui.add_space(4.0);

    // Offset for new layers
    ui.horizontal(|ui| {
        ui.label("Apply at offset:");
        ui.add(egui::TextEdit::singleline(apply_offset).desired_width(100.0).hint_text("0x0"));
    });
    ui.add_space(4.0);

    let filter_lower = filter.to_lowercase();
    let is_searching = !filter_lower.is_empty();

    // Group entries by category
    let mut categories: std::collections::BTreeMap<&str, Vec<(usize, &str, String)>> =
        std::collections::BTreeMap::new();
    for (idx, entry) in registry.entries.iter().enumerate() {
        let name = &entry.template.name;
        if !filter_lower.is_empty() {
            let name_match = name.to_lowercase().contains(&filter_lower);
            let ext_match = entry
                .template
                .extensions
                .iter()
                .any(|e| e.to_lowercase().contains(&filter_lower));
            let desc_match = entry.template.description.to_lowercase().contains(&filter_lower);
            if !name_match && !ext_match && !desc_match {
                continue;
            }
        }
        let mut tooltip = entry.template.description.clone();
        if !entry.template.extensions.is_empty() {
            tooltip.push_str(&format!("\nExtensions: {}", entry.template.extensions.join(", ")));
        }
        categories
            .entry(&entry.category)
            .or_default()
            .push((idx, name, tooltip));
    }

    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if categories.is_empty() {
                ui.label("No templates found");
                return;
            }

            for (category, entries) in &categories {
                let header = egui::CollapsingHeader::new(RichText::new(*category).strong())
                    .default_open(false);
                // Force categories open when searching so results are visible
                let header = if is_searching { header.open(Some(true)) } else { header };
                header.show(ui, |ui| {
                        for (idx, name, tooltip) in entries {
                            let is_active = active_indices.contains(idx);
                            let response = ui
                                .selectable_label(is_active, *name)
                                .on_hover_text(tooltip);
                            if response.clicked() {
                                if is_active {
                                    action = Some(TemplateBrowserAction::Deselect);
                                } else {
                                    action = Some(TemplateBrowserAction::Select(*idx));
                                }
                            }
                        }
                    });
            }
        });

    action
}
