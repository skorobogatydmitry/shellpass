use egui::Ui as EguiUi;

pub(crate) struct Ui;

impl super::OsUi for Ui {
    /// there's an area in adnroid screen which is actually occupied by status bar
    /// let's keep it clean
    fn top_padding(ui: &mut EguiUi) {
        egui::Panel::top("status_bar_space").show_inside(ui, |ui| {
            ui.set_height(32.0);
        });
    }
}
