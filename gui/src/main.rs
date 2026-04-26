mod app;

fn main() -> eframe::Result {
    let initial_file = std::env::args().nth(1).map(std::path::PathBuf::from);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("DSN Viewer")
            .with_inner_size([1200.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "DSN Viewer",
        options,
        Box::new(move |_cc| Ok(Box::new(app::DsnViewerApp::with_file(initial_file)))),
    )
}
