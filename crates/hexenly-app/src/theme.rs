use egui::{Color32, FontFamily, FontId, Style, TextStyle, Visuals};

pub struct HexColors;

impl HexColors {
    pub const NULL_BYTE: Color32 = Color32::from_rgb(100, 100, 100);
    pub const MAX_BYTE: Color32 = Color32::from_rgb(220, 80, 80);
    pub const PRINTABLE_ASCII: Color32 = Color32::from_rgb(140, 190, 240);
    pub const DEFAULT_BYTE: Color32 = Color32::from_rgb(190, 190, 190);
    pub const SELECTION_BG: Color32 = Color32::from_rgb(60, 80, 120);
    pub const SEARCH_HIGHLIGHT: Color32 = Color32::from_rgb(140, 120, 40);
    pub const OFFSET_COLUMN: Color32 = Color32::from_rgb(120, 140, 100);
    pub const ASCII_PANE: Color32 = Color32::from_rgb(170, 170, 140);
    pub const CURSOR_BG: Color32 = Color32::from_rgb(96, 128, 192);
    pub const CURSOR_BORDER: Color32 = Color32::from_rgb(140, 170, 220);
}

pub fn monospace_font() -> FontId {
    FontId::new(14.0, FontFamily::Monospace)
}

pub fn annotation_font() -> FontId {
    FontId::new(9.0, FontFamily::Proportional)
}

pub fn apply_theme(ctx: &egui::Context) {
    let mut style = Style {
        visuals: Visuals::dark(),
        ..Style::default()
    };

    style.visuals.window_fill = Color32::from_rgb(30, 30, 34);
    style.visuals.panel_fill = Color32::from_rgb(30, 30, 34);
    style.visuals.extreme_bg_color = Color32::from_rgb(22, 22, 26);
    style.visuals.window_corner_radius = egui::CornerRadius::same(2);

    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(14.0, FontFamily::Monospace),
    );
    style
        .text_styles
        .insert(TextStyle::Body, FontId::new(13.0, FontFamily::Proportional));
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(13.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(11.0, FontFamily::Proportional),
    );

    ctx.set_style(style);
}
