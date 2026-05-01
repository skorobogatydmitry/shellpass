use egui::Ui;

use crate::App;

impl super::OsUi for Ui {
    /// there's an area in adnroid screen which is actually occupied by status bar
    /// let's keep it clean
    fn top_padding(&mut self) {
        egui::Panel::top("status_bar_space").show_inside(self, |ui| {
            ui.set_height(32.0);
        });
    }

    fn pass_root_setting(&mut self, app: &mut App) {
        // todo!()
    }
}
