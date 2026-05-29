use eframe::CreationContext;
use log::info;
use std::{
    error::Error,
    sync::{Arc, Condvar, Mutex, RwLock},
    thread,
};

#[cfg(target_os = "android")]
use crate::ui::android::load_file_picker_activity;
#[cfg(target_os = "android")]
pub(crate) mod android_interface;

use crate::{
    pass::{PassEntry, PassRepository, REPOSITORY},
    settings::SETTINGS,
};

pub(crate) mod pass;
pub(crate) mod settings;
pub(crate) mod ui;

pub(crate) struct App<U: PassEntry> {
    pattern: Arc<Mutex<String>>,
    pattern_change_fence: Arc<Condvar>,
    last_match: Arc<RwLock<Vec<U>>>,
}

impl App<pass::PassEntryImpl> {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        _cc: &CreationContext,
    ) -> Result<Box<dyn eframe::App>, Box<dyn Error + Send + Sync>> {
        let mut result = Self {
            pattern: Arc::new(Mutex::new(String::new())),
            pattern_change_fence: Arc::new(Condvar::new()),
            last_match: Arc::new(RwLock::new(Vec::new())),
        };

        result.search_routine();
        let mut settings = SETTINGS.lock().expect("settings are poisoned!");
        settings.update_routine();
        Ok(Box::new(result))
    }

    fn search_routine(&mut self) {
        let pattern = Arc::clone(&self.pattern);
        let pattern_change_fence = Arc::clone(&self.pattern_change_fence);
        let last_match = Arc::clone(&self.last_match);

        thread::spawn(move || {
            let mut current_pattern = pattern.lock().expect("pattern is poisoned!");
            loop {
                current_pattern = pattern_change_fence
                    .wait(current_pattern)
                    .expect("pattern is poisoned!");
                let last_seen_pattern = current_pattern.clone();

                // TODO: make a faster swap
                let repository = REPOSITORY.lock().expect("repository is poisoned!");
                let new_items = repository.get_by_pattern(last_seen_pattern.as_str());
                drop(repository);

                let mut last_match = last_match.write().expect("last match is poisoned!");
                last_match.clear();
                last_match.extend(new_items);
                info!("found matches: {}", last_match.len());
            }
        });
    }
}

impl eframe::App for App<pass::PassEntryImpl> {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui::main(self, ui);
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
        "shellpass",
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
        "unused",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            App::new(cc)
        }),
    )
}
