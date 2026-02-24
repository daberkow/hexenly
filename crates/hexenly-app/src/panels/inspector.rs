use egui::{Grid, RichText, ScrollArea, Ui};
use hexenly_core::ByteInterpreter;

use crate::theme::monospace_font;

pub fn show(ui: &mut Ui, data: &[u8], cursor: usize) {
    ui.heading("Inspector");
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
    }); // ScrollArea
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
