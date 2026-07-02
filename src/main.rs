// Linux's entrypont
#[cfg(target_os = "linux")]
fn main() -> eframe::Result {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_decorations(false)
            .with_inner_size([600.0, 200.0])
            .with_min_inner_size([400.0, 150.0])
            .with_transparent(true),
        ..Default::default()
    };
    eframe::run_native(
        shellpass::NAME_VERSION,
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            shellpass::App::new(cc)
        }),
    )
}

// just sate the analyzer
#[cfg(target_os = "android")]
fn main() {
    panic!("Use `cargo apk2 run --lib'")
}
