use egui::Ui;

use crate::{
    settings::{self, SETTINGS, SettingsUpdateReq},
    ui::UI_STATE,
};

impl super::OsUi for Ui {
    /// no special spacing is needed
    fn top_padding(&mut self) {}
    fn bottom_padding(&mut self) {}

    fn to_clipboard(&self, s: String) {
        self.copy_text(s);
    }

    fn pass_root_setting(&mut self) {
        self.label("pass repository root");
        let settings = SETTINGS.lock().expect("settings are poisoned");
        let mut pass_root = settings.pass_root().unwrap_or_default();
        drop(settings);

        if self.text_edit_singleline(&mut pass_root).lost_focus() {
            settings::send_update_request(SettingsUpdateReq::PassRoot(pass_root));
        }
    }

    fn gnupg_secret_key_settings(&mut self) -> bool {
        let mut ui_state = UI_STATE.lock().expect("UI state is poisoned!");
        let digest_buf = &mut ui_state.partial_gnupg_secret_key;
        self.add(egui::TextEdit::singleline(digest_buf).hint_text("private key digest"))
            .lost_focus()
    }
}
