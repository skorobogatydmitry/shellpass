use std::error::Error;

use eframe::CreationContext;

use crate::pass::PassRepository;

pub mod pass;

pub struct App {
    pass: PassRepository,
    pattern: String,
}

impl App {
    pub fn new(
        _cc: &CreationContext,
    ) -> Result<Box<dyn eframe::App>, Box<dyn Error + Send + Sync>> {
        Ok(Box::new(Self {
            pass: PassRepository::new().map_err(|e| eframe::Error::AppCreation(e.into()))?,
            pattern: String::new(),
        }))
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let panel = egui::CentralPanel::no_frame();
        panel.show_inside(ui, |ui| {
            ui.set_zoom_factor(2.0);
            ui.vertical_centered_justified(|ui| {
                ui.text_edit_singleline(&mut self.pattern)
                    .highlight()
                    .request_focus();
                ui.label(format!(
                    "{} entries in your pass, start typing to search",
                    self.pass.entries_count()
                ))
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
        });
    }
}
