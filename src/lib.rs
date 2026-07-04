use eframe::CreationContext;
use std::error::Error;

#[cfg(target_os = "android")]
use crate::ui::init_picker_activities;
#[cfg(target_os = "android")]
pub(crate) mod android_interface;

use crate::{finder::FINDER, settings::SETTINGS};

pub const NAME_VERSION: &str = concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"));

pub(crate) mod finder;
pub(crate) mod notifications;
pub(crate) mod pass;
pub(crate) mod settings;
pub(crate) mod ui;

pub struct App;

impl App {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(cc: &CreationContext) -> Result<Box<dyn eframe::App>, Box<dyn Error + Send + Sync>> {
        notifications::initialize();
        {
            let mut finder = FINDER.lock().expect("finder is poisoned!");
            finder.search_routine();
        }

        settings::initialize();

        cc.egui_ctx
            .set_zoom_factor(SETTINGS.lock().expect("settings are poisoned!").zoom_factor);
        Ok(Box::new(Self {}))
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui::main(ui);
    }
}

// Android's entrypoint
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    // Log to android output
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );

    init_picker_activities().expect("error on initializing activities");

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
    .expect("cannot run application")
}
