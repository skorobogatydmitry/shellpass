use std::{
    error::Error,
    ops::DerefMut,
    sync::{Arc, RwLock},
    thread,
    time::Duration,
};

use eframe::CreationContext;
use log::{info, warn};

use crate::pass::PassRepository;

pub mod pass;

pub struct App {
    repository: Arc<RwLock<PassRepository>>,
    pattern: Arc<RwLock<String>>,
    last_match: Arc<RwLock<Vec<String>>>,
}

impl App {
    pub fn new(
        _cc: &CreationContext,
    ) -> Result<Box<dyn eframe::App>, Box<dyn Error + Send + Sync>> {
        let mut result = Self {
            repository: Arc::new(RwLock::new(
                PassRepository::new().map_err(|e| eframe::Error::AppCreation(e.into()))?,
            )),
            pattern: Arc::new(RwLock::new(String::new())),
            last_match: Arc::new(RwLock::new(Vec::new())),
        };

        result.update_routine();
        Ok(Box::new(result))
    }

    pub fn update_routine(&mut self) {
        let pattern = Arc::clone(&self.pattern);
        let last_match = Arc::clone(&self.last_match);
        let repository = Arc::clone(&self.repository);

        thread::spawn(move || {
            // TODO: notify on changes instead
            let mut last_seen_pattern = String::new();
            loop {
                thread::sleep(Duration::from_millis(500));
                let current_pattern = pattern.read().expect("pattern is poisoned!");
                if current_pattern.is_empty() || last_seen_pattern == current_pattern.as_ref() {
                    warn!("pattern is empty or hasn't changed");
                    continue;
                }
                last_seen_pattern = current_pattern.clone();
                drop(current_pattern);

                let mut last_match = last_match.write().expect("last match is poisoned!");
                // TODO: make a faster swap
                last_match.clear();
                repository
                    .read()
                    .expect("repository is poisoned!")
                    .get_by_pattern(last_seen_pattern.as_str())
                    .into_iter()
                    .for_each(|entry| last_match.push(entry));
                info!("found matches: {}", last_match.len());
            }
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // let panel = egui::CentralPanel::no_frame();
        // panel.show_inside(ui, |ui| {
        ui.set_zoom_factor(1.5);
        ui.vertical_centered_justified(|ui| {
            let mut pattern = self.pattern.write().expect("pattern is poisoned!");
            ui.text_edit_singleline(pattern.deref_mut())
                .highlight()
                .request_focus();
            drop(pattern);

            let repository = self.repository.read().expect("repository is poisoned!");
            ui.label(format!(
                "{} entries in your pass, start typing to search",
                repository.entries_count()
            ));
        });

        let last_match = self.last_match.read().expect("last match is poisoned!");
        last_match.iter().for_each(|s| {
            ui.label(s);
        });
        // if !self.pattern.is_empty() {
        //     for entry in self.pass.get_by_pattern(self.pattern.as_str()) {}
        // }
        // ui.add(egui::Slider::new(&mut self.age, 0..=120).text("age"));
        // if ui.button("Increment").clicked() {
        //     self.age += 1;
        // }

        // ui.image(egui::include_image!(
        //     "../../../crates/egui/assets/ferris.png"
        // ));
        // });
    }
}
