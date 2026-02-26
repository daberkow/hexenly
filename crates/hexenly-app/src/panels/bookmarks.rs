//! Bookmarks panel — add, edit, delete, and navigate named byte markers.

use egui::{self, RichText, ScrollArea, Ui};
use hexenly_core::Bookmark;

/// An interaction returned from the bookmarks panel.
#[derive(Debug)]
pub enum BookmarkAction {
    Add,
    GoToOffset(usize),
    Delete(usize),
    Updated, // name or note was edited, trigger save
    Close,
}

/// Render the bookmarks panel. Returns an action if the user added, deleted, or navigated.
pub fn show(
    ui: &mut Ui,
    bookmarks: &mut [Bookmark],
    cursor_offset: usize,
) -> Option<BookmarkAction> {
    let mut action = None;

    ui.horizontal(|ui| {
        ui.heading("Bookmarks");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("\u{2715}").clicked() {
                action = Some(BookmarkAction::Close);
            }
        });
    });
    ui.separator();

    if ui.button("Add at cursor").clicked() {
        action = Some(BookmarkAction::Add);
    }

    ui.add_space(4.0);

    if bookmarks.is_empty() {
        ui.label(
            RichText::new("No bookmarks yet. Click \"Add at cursor\" to bookmark the current offset.")
                .weak(),
        );
        return action;
    }

    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // We iterate by index so we can emit Delete(idx) with the correct index.
            // Bookmarks are kept sorted by offset externally (on Add).
            let mut idx = 0;
            while idx < bookmarks.len() {
                let bookmark_offset = bookmarks[idx].offset;

                ui.group(|ui| {
                    // Top row: name editor, offset label, delete button
                    ui.horizontal(|ui| {
                        let name_response = ui.add(
                            egui::TextEdit::singleline(&mut bookmarks[idx].name)
                                .desired_width(100.0),
                        );
                        if name_response.lost_focus() {
                            action = Some(BookmarkAction::Updated);
                        }

                        let offset_text = match bookmarks[idx].end {
                            Some(end) => format!("0x{bookmark_offset:08X}..0x{end:08X}"),
                            None => format!("0x{bookmark_offset:08X}"),
                        };
                        let offset_label = ui.add(
                            egui::Label::new(
                                RichText::new(offset_text).monospace().small(),
                            )
                            .sense(egui::Sense::click()),
                        );
                        if offset_label.clicked() {
                            action = Some(BookmarkAction::GoToOffset(bookmark_offset));
                        }
                        offset_label.on_hover_text("Click to jump to this offset");

                        if ui.small_button("X").clicked() {
                            action = Some(BookmarkAction::Delete(idx));
                        }
                    });

                    // Note editor below
                    let note_response = ui.add(
                        egui::TextEdit::singleline(&mut bookmarks[idx].note)
                            .hint_text("Add a note...")
                            .desired_width(ui.available_width())
                            .font(egui::TextStyle::Small),
                    );
                    if note_response.lost_focus() {
                        action = Some(BookmarkAction::Updated);
                    }
                });

                ui.add_space(2.0);
                idx += 1;
            }

            // Show cursor offset hint at bottom
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!("Cursor: 0x{cursor_offset:08X}"))
                    .weak()
                    .small(),
            );
        });

    action
}
