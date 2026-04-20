use std::{
    error::Error,
    ops::DerefMut,
    sync::{Arc, Condvar, Mutex, RwLock},
    thread,
};

use eframe::CreationContext;
use log::info;

use crate::pass::{PassEntry, PassRepository};

pub mod pass;

pub struct App {
    repository: Arc<RwLock<PassRepository>>,
    pattern: Arc<Mutex<String>>,
    pattern_change_fence: Arc<Condvar>,
    last_match: Arc<RwLock<Vec<PassEntry>>>,
}

impl App {
    pub fn new(
        _cc: &CreationContext,
    ) -> Result<Box<dyn eframe::App>, Box<dyn Error + Send + Sync>> {
        let mut result = Self {
            repository: Arc::new(RwLock::new(
                PassRepository::new().map_err(|e| eframe::Error::AppCreation(e.into()))?,
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

                let mut last_match = last_match.write().expect("last match is poisoned!");
                // TODO: make a faster swap
                last_match.clear();
                let repository = repository.read().expect("repository is poisoned!");
                repository
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
        ui.set_zoom_factor(1.5);
        ui.vertical_centered_justified(|ui| {
            let mut pattern = self.pattern.lock().expect("pattern is poisoned!");
            let resp = ui.text_edit_singleline(pattern.deref_mut()).highlight();
            drop(pattern);
            if resp.changed() {
                self.pattern_change_fence.notify_one();
            }
            resp.request_focus();

            let repository = self.repository.read().expect("repository is poisoned!");
            ui.label(format!(
                "{} entries in your pass, start typing to search",
                repository.entries_count()
            ));
        });

        let last_match = self.last_match.read().expect("last match is poisoned!");
        last_match.iter().for_each(|entry| {
            ui.horizontal(|ui| {
                // TODO: color on click
                if ui.small_button("usr:pwd").clicked() {
                    let repository = self.repository.read().expect("repository is poisoned!");
                    if let Some(data) = repository.retrieve(entry) {
                        ui.copy_text(format!("{}:{}", data.0, data.1));
                    } else {
                        todo!("show notification")
                    }
                }
                if ui.small_button("pwd").clicked() {
                    let repository = self.repository.read().expect("repository is poisoned!");
                    if let Some(data) = repository.retrieve(entry) {
                        ui.copy_text(data.1);
                    } else {
                        todo!("show notification")
                    }
                }
                ui.label(entry.to_string());
            });
        });
    }
}
