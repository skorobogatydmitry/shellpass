use std::{
    error::Error,
    ops::DerefMut,
    sync::{Arc, Condvar, Mutex, RwLock},
    thread,
};

use eframe::CreationContext;
use egui::{Layout, ScrollArea};
use log::info;

use crate::pass::{PassEntry, PassRepository};

pub mod pass;

pub struct App {
    repository: Arc<RwLock<dyn pass::PassRepository>>,
    pattern: Arc<Mutex<String>>,
    pattern_change_fence: Arc<Condvar>,
    last_match: Arc<RwLock<Vec<PassEntry>>>,
}

#[cfg(target_os = "linux")]
type PassRepositoryImpl = pass::linux::PassRepository;
#[cfg(target_os = "android")]
type PassRepositoryImpl = pass::android::PassRepository;

impl App {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        _cc: &CreationContext,
    ) -> Result<Box<dyn eframe::App>, Box<dyn Error + Send + Sync>> {
        let mut result = Self {
            repository: Arc::new(RwLock::new(
                PassRepositoryImpl::new().map_err(|e| eframe::Error::AppCreation(e.into()))?,
            )),
            pattern: Arc::new(Mutex::new(String::new())),
            pattern_change_fence: Arc::new(Condvar::new()),
            last_match: Arc::new(RwLock::new(Vec::new())),
        };

        result.update_routine();
        Ok(Box::new(result))
    }

    pub fn update_routine(&mut self) {
        let pattern = Arc::clone(&self.pattern);
        let pattern_change_fence = Arc::clone(&self.pattern_change_fence);
        let last_match = Arc::clone(&self.last_match);
        let repository = Arc::clone(&self.repository);

        thread::spawn(move || {
            let mut current_pattern = pattern.lock().expect("pattern is poisoned!");
            loop {
                current_pattern = pattern_change_fence
                    .wait(current_pattern)
                    .expect("pattern is poisoned!");
                let last_seen_pattern = current_pattern.clone();

                // TODO: make a faster swap
                let repository = repository.read().expect("repository is poisoned!");
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

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.set_zoom_factor(1.5);
        ui.vertical_centered_justified(|ui| {
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    let image = egui::include_image!("../assets/cog.png");
                    ui.menu_image_button(image, |ui| ui.label("text"));
                    ui.centered_and_justified(|ui| {
                        let mut pattern = self.pattern.lock().expect("pattern is poisoned!");
                        let resp = ui.text_edit_singleline(pattern.deref_mut()).highlight();
                        drop(pattern);
                        if resp.changed() {
                            self.pattern_change_fence.notify_one();
                        }
                        resp.request_focus();
                    });
                });
            });

            let repository = self.repository.read().expect("repository is poisoned!");
            ui.label(format!(
                "{} entries in your pass, start typing to search",
                repository.entries_count()
            ));
        });

        let last_match = self.last_match.read().expect("last match is poisoned!");
        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            last_match.iter().for_each(|entry| {
                // TODO: color on click
                if ui.selectable_label(false, entry.to_string()).clicked() {
                    let repository = self.repository.read().expect("repository is poisoned!");
                    if let Ok(data) = repository.retrieve(entry) {
                        ui.copy_text(format!("{}:{}", data.0, data.1));
                    } else {
                        todo!("show notification")
                    }
                }
            });
        });
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
            // .with_close_button(true) // TODO: make x visible
            .with_decorations(false) // Hide the OS-specific "chrome" around the window
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
