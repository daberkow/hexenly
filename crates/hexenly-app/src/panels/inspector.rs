//! Byte inspector panel — shows every interpretation of the bytes at the cursor.

use egui::{self, Grid, RichText, ScrollArea, Ui};
use hexenly_core::ByteInterpreter;

use crate::theme::{HexColors, monospace_font};

/// Render the inspector panel. Returns `true` if the user clicked the close button.
pub fn show(ui: &mut Ui, data: &[u8], cursor: usize, colors: &HexColors) -> bool {
    let mut close = false;
    ui.horizontal(|ui| {
        ui.heading("Inspector");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("x").clicked() {
                close = true;
            }
        });
    });
    ui.separator();

    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
    let Some(interp) = ByteInterpreter::interpret(data, cursor) else {
        ui.label("No byte selected");
        return;
    };

    ui.label(RichText::new(format!("Offset: 0x{:08X} ({})", cursor, cursor)).monospace());
    ui.add_space(4.0);

    ui.label(RichText::new("Byte").strong());
    Grid::new("inspector_byte").num_columns(2).show(ui, |ui| {
        row(ui, "Hex", &interp.hex);
        row(ui, "Decimal", &interp.decimal);
        row(ui, "Octal", &interp.octal);
        row(ui, "Binary", &interp.binary);
        row(
            ui,
            "ASCII",
            &interp
                .ascii
                .map(|c| format!("'{c}'"))
                .unwrap_or_else(|| "N/A".into()),
        );
    });

    // Bits section
    ui.add_space(8.0);
    ui.label(RichText::new("Bits").strong());
    ui.horizontal(|ui| {
        for bit in (0..8).rev() {
            let set = (interp.byte >> bit) & 1 == 1;
            let text = if set { "1" } else { "0" };
            let color = if set {
                colors.bit_set
            } else {
                colors.bit_unset
            };
            ui.label(RichText::new(text).font(monospace_font()).color(color));
        }
    });

    ui.add_space(8.0);
    ui.label(RichText::new("Little-Endian").strong());
    Grid::new("inspector_le").num_columns(2).show(ui, |ui| {
        opt_row(ui, "u16", interp.u16_le.map(|v| v.to_string()));
        opt_row(ui, "u32", interp.u32_le.map(|v| v.to_string()));
        opt_row(ui, "u64", interp.u64_le.map(|v| v.to_string()));
        opt_row(ui, "i16", interp.i16_le.map(|v| v.to_string()));
        opt_row(ui, "i32", interp.i32_le.map(|v| v.to_string()));
        opt_row(ui, "i64", interp.i64_le.map(|v| v.to_string()));
        opt_row(ui, "f32", interp.f32_le.map(|v| format!("{v:.6e}")));
        opt_row(ui, "f64", interp.f64_le.map(|v| format!("{v:.6e}")));
    });

    ui.add_space(8.0);
    ui.label(RichText::new("Big-Endian").strong());
    Grid::new("inspector_be").num_columns(2).show(ui, |ui| {
        opt_row(ui, "u16", interp.u16_be.map(|v| v.to_string()));
        opt_row(ui, "u32", interp.u32_be.map(|v| v.to_string()));
        opt_row(ui, "u64", interp.u64_be.map(|v| v.to_string()));
        opt_row(ui, "i16", interp.i16_be.map(|v| v.to_string()));
        opt_row(ui, "i32", interp.i32_be.map(|v| v.to_string()));
        opt_row(ui, "i64", interp.i64_be.map(|v| v.to_string()));
        opt_row(ui, "f32", interp.f32_be.map(|v| format!("{v:.6e}")));
        opt_row(ui, "f64", interp.f64_be.map(|v| format!("{v:.6e}")));
    });

    // Date/Time section
    ui.add_space(8.0);
    ui.label(RichText::new("Date/Time").strong());
    Grid::new("inspector_datetime").num_columns(2).show(ui, |ui| {
        opt_row(ui, "Unix u32 LE", interp.unix_ts_u32_le.clone());
        opt_row(ui, "Unix u32 BE", interp.unix_ts_u32_be.clone());
        opt_row(ui, "Unix u64 LE", interp.unix_ts_u64_le.clone());
        opt_row(ui, "Unix u64 BE", interp.unix_ts_u64_be.clone());
        opt_row(ui, "DOS LE", interp.dos_datetime_le.clone());
        opt_row(ui, "DOS BE", interp.dos_datetime_be.clone());
        opt_row(ui, "FILETIME", interp.filetime_le.clone());
    });

    // Text section
    ui.add_space(8.0);
    ui.label(RichText::new("Text").strong());
    Grid::new("inspector_text").num_columns(2).show(ui, |ui| {
        opt_row(ui, "UTF-8", interp.utf8_char.clone());
        opt_row(ui, "UTF-16 LE", interp.utf16_le_char.clone());
        opt_row(ui, "UTF-16 BE", interp.utf16_be_char.clone());
    });
    }); // ScrollArea

    close
}

fn row(ui: &mut Ui, label: &str, value: &str) {
    ui.label(label);
    ui.label(RichText::new(value).font(monospace_font()));
    ui.end_row();
}

fn opt_row(ui: &mut Ui, label: &str, value: Option<String>) {
    if let Some(v) = value {
        ui.label(label);
        ui.label(RichText::new(v).font(monospace_font()));
        ui.end_row();
    }
}
