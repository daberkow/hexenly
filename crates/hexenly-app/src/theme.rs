use egui::{Color32, FontFamily, FontId, Style, TextStyle, Visuals};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

pub struct HexColors {
    pub null_byte: Color32,
    pub max_byte: Color32,
    pub printable_ascii: Color32,
    pub default_byte: Color32,
    pub selection_bg: Color32,
    pub search_highlight: Color32,
    pub offset_column: Color32,
    pub ascii_pane: Color32,
    pub cursor_bg: Color32,
    pub cursor_border: Color32,
    pub bit_set: Color32,
    pub bit_unset: Color32,
    #[allow(dead_code)] // reserved for future modified-byte highlighting
    pub modified_byte: Color32,
}

impl HexColors {
    pub fn dark() -> Self {
        Self {
            null_byte: Color32::from_rgb(100, 100, 100),
            max_byte: Color32::from_rgb(220, 80, 80),
            printable_ascii: Color32::from_rgb(140, 190, 240),
            default_byte: Color32::from_rgb(190, 190, 190),
            selection_bg: Color32::from_rgb(60, 80, 120),
            search_highlight: Color32::from_rgb(140, 120, 40),
            offset_column: Color32::from_rgb(120, 140, 100),
            ascii_pane: Color32::from_rgb(170, 170, 140),
            cursor_bg: Color32::from_rgb(96, 128, 192),
            cursor_border: Color32::from_rgb(140, 170, 220),
            bit_set: Color32::from_rgb(120, 200, 120),
            bit_unset: Color32::from_gray(100),
            modified_byte: Color32::from_rgb(255, 200, 80),
        }
    }

    pub fn light() -> Self {
        Self {
            null_byte: Color32::from_rgb(160, 160, 160),
            max_byte: Color32::from_rgb(180, 40, 40),
            printable_ascii: Color32::from_rgb(30, 80, 140),
            default_byte: Color32::from_rgb(60, 60, 60),
            selection_bg: Color32::from_rgb(170, 200, 240),
            search_highlight: Color32::from_rgb(240, 220, 100),
            offset_column: Color32::from_rgb(80, 110, 60),
            ascii_pane: Color32::from_rgb(90, 90, 70),
            cursor_bg: Color32::from_rgb(140, 180, 230),
            cursor_border: Color32::from_rgb(60, 100, 180),
            bit_set: Color32::from_rgb(40, 140, 40),
            bit_unset: Color32::from_gray(170),
            modified_byte: Color32::from_rgb(200, 150, 20),
        }
    }
}

pub fn monospace_font() -> FontId {
    FontId::new(14.0, FontFamily::Monospace)
}

pub fn annotation_font() -> FontId {
    FontId::new(9.0, FontFamily::Proportional)
}

pub fn apply_theme(ctx: &egui::Context, mode: ThemeMode) {
    let mut style = Style {
        visuals: match mode {
            ThemeMode::Dark => Visuals::dark(),
            ThemeMode::Light => Visuals::light(),
        },
        ..Style::default()
    };

    match mode {
        ThemeMode::Dark => {
            style.visuals.window_fill = Color32::from_rgb(30, 30, 34);
            style.visuals.panel_fill = Color32::from_rgb(30, 30, 34);
            style.visuals.extreme_bg_color = Color32::from_rgb(22, 22, 26);
        }
        ThemeMode::Light => {
            style.visuals.window_fill = Color32::from_rgb(245, 245, 248);
            style.visuals.panel_fill = Color32::from_rgb(245, 245, 248);
            style.visuals.extreme_bg_color = Color32::from_rgb(232, 232, 236);
        }
    }
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
