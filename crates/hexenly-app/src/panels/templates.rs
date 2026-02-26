//! Template browser panel — browse, filter, and select binary templates.

use egui::{self, RichText, ScrollArea, Ui};
use hexenly_templates::loader::TemplateRegistry;

/// An interaction returned from the template browser.
#[derive(Debug)]
pub enum TemplateBrowserAction {
    Select(usize),
    Deselect,
    Close,
}

/// Render the template browser. Returns an action if the user selected or deselected a template.
pub fn show(
    ui: &mut Ui,
    registry: &TemplateRegistry,
    active_index: Option<usize>,
    filter: &mut String,
) -> Option<TemplateBrowserAction> {
    let mut action = None;

    ui.horizontal(|ui| {
        ui.heading("Templates");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("\u{2715}").clicked() {
                action = Some(TemplateBrowserAction::Close);
            }
        });
    });
    ui.separator();

    // Search filter
    ui.horizontal(|ui| {
        ui.label("Filter:");
        ui.text_edit_singleline(filter);
    });
    ui.add_space(4.0);

    let filter_lower = filter.to_lowercase();

    // Group entries by category
    let mut categories: std::collections::BTreeMap<&str, Vec<(usize, &str)>> =
        std::collections::BTreeMap::new();
    for (idx, entry) in registry.entries.iter().enumerate() {
        let name = &entry.template.name;
        if !filter_lower.is_empty() && !name.to_lowercase().contains(&filter_lower) {
            continue;
        }
        categories
            .entry(&entry.category)
            .or_default()
            .push((idx, name));
    }

    ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height(ui.available_height() - 120.0)
        .show(ui, |ui| {
            if categories.is_empty() {
                ui.label("No templates found");
                return;
            }

            for (category, entries) in &categories {
                egui::CollapsingHeader::new(RichText::new(*category).strong())
                    .default_open(true)
                    .show(ui, |ui| {
                        for &(idx, name) in entries {
                            let is_active = active_index == Some(idx);
                            let response = ui.selectable_label(is_active, name);
                            if response.clicked() {
                                if is_active {
                                    action = Some(TemplateBrowserAction::Deselect);
                                } else {
                                    action = Some(TemplateBrowserAction::Select(idx));
                                }
                            }
                        }
                    });
            }
        });

    // Show selected template metadata
    ui.separator();
    if let Some(idx) = active_index {
        if let Some(entry) = registry.entries.get(idx) {
            let t = &entry.template;
            ui.label(RichText::new(&t.name).strong());
            ui.label(&t.description);
            if !t.extensions.is_empty() {
                ui.label(format!("Extensions: {}", t.extensions.join(", ")));
            }
            if let Some(magic) = &t.magic {
                ui.label(format!("Magic: {magic}"));
            }
        }
    } else {
        ui.label("No template selected");
    }

    action
}
