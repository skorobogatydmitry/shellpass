use shellpass::App;

fn main() -> eframe::Result {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            // .with_close_button(true) // TODO: make x visible
            .with_decorations(false) // Hide the OS-specific "chrome" around the window
            .with_inner_size([600.0, 200.0])
            .with_min_inner_size([400.0, 150.0])
            .with_transparent(true),
        ..Default::default()
    };
    eframe::run_native("unused", options, Box::new(|cc| App::new(cc)))
}
