use egui::Ui as EguiUi;

pub(crate) struct Ui;

impl super::OsUi for Ui {
    /// no special spacing is needed
    fn top_padding(_ui: &mut EguiUi) {}
}
