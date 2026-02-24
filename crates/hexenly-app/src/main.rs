mod app;
mod panels;
mod theme;

use app::HexenlyApp;

const WINDOW_WIDTH: f32 = 1280.0;
const WINDOW_HEIGHT: f32 = 800.0;

fn load_icon() -> egui::IconData {
    let png_bytes = include_bytes!("../../../docs/logo128.png");
    let img = image::load_from_memory(png_bytes).expect("Failed to decode icon PNG");
    let rgba = img.to_rgba8();
    egui::IconData {
        rgba: rgba.to_vec(),
        width: rgba.width(),
        height: rgba.height(),
    }
}

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("hexenly=info".parse().unwrap()),
        )
        .init();

    let path = std::env::args().nth(1);

    let icon = load_icon();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([WINDOW_WIDTH, WINDOW_HEIGHT])
            .with_title("Hexenly")
            .with_icon(icon),
        ..Default::default()
    };

    eframe::run_native(
        "Hexenly",
        options,
        Box::new(|_cc| Ok(Box::new(HexenlyApp::new(path)))),
    )
}
