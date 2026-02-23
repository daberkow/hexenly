mod app;
mod panels;
mod theme;

use app::HexenlyApp;

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("hexenly=info".parse().unwrap()),
        )
        .init();

    let path = std::env::args().nth(1);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_title("Hexenly"),
        ..Default::default()
    };

    eframe::run_native(
        "Hexenly",
        options,
        Box::new(|_cc| Ok(Box::new(HexenlyApp::new(path)))),
    )
}
