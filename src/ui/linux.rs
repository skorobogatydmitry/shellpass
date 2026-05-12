use egui::Ui;

use crate::{App, settings::SETTINGS};

impl super::OsUi for Ui {
    /// no special spacing is needed
    fn top_padding(&mut self) {}
    fn pass_root_setting(&mut self, _app: &mut App) {
        self.label("pass repository root");
        let mut settings = SETTINGS.lock().expect("settings are poisoned");
        let mut pass_root = settings.pass_root().unwrap_or_default();

        if self.text_edit_singleline(&mut pass_root).changed() {
            settings.set_pass_root(pass_root);
        }
    }
}
