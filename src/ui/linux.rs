use egui::{Response, Ui};

use crate::{App, settings::SETTINGS};

impl super::OsUi for Ui {
    /// no special spacing is needed
    fn top_padding(&mut self) {}
    fn pass_root_setting(&mut self, _app: &mut App) -> Response {
        self.label("pass repository root");
        let mut settings = SETTINGS.lock().expect("settings are poisoned");
        let pass_root = settings.pass_root.get_or_insert(String::new());

        self.text_edit_singleline(pass_root)
    }
}
