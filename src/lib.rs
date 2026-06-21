use eframe::CreationContext;
use log::warn;
use std::error::Error;

#[cfg(target_os = "android")]
use crate::ui::android::load_file_picker_activity;
#[cfg(target_os = "android")]
pub(crate) mod android_interface;

use crate::{finder::FINDER, settings::SETTINGS};

const NAME_VERSION: &str = concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"));

pub(crate) mod finder;
pub(crate) mod pass;
pub(crate) mod settings;
pub(crate) mod ui;

pub(crate) struct App;

impl App {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        _cc: &CreationContext,
    ) -> Result<Box<dyn eframe::App>, Box<dyn Error + Send + Sync>> {
        {
            let mut finder = FINDER.lock().expect("finder is poisoned!");
            finder.search_routine();
        }

        {
            let mut settings = SETTINGS.lock().expect("settings are poisoned!");
            settings.update_routine();
        }
        Ok(Box::new(Self {}))
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui::main(ui);
    }
    fn on_exit(&mut self) {
        let settings = SETTINGS.lock().expect("settings are poisoned!");
        if let Err(e) = settings.try_save() {
            warn!("unable to save settings on exit: {e:#}")
        }
    }
}

// is required to run on Android
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    // Log to android output
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );

    load_file_picker_activity().expect("unable to load file picker activity");

    let options = eframe::NativeOptions {
        android_app: Some(app),
        ..Default::default()
    };
    eframe::run_native(
        NAME_VERSION,
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            App::new(cc)
        }),
    )
    .unwrap()
}

#[cfg(target_os = "linux")]
pub fn linux_main() -> eframe::Result {
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
        NAME_VERSION,
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            App::new(cc)
        }),
    )
}
