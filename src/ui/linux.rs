use egui::{Response, Ui};

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

    fn gnupg_secret_key_settings(&mut self, passphrase_setting: Response) -> bool {
        let mut ui_state = UI_STATE.lock().expect("UI state is poisoned!");
        let digest_buf = &mut ui_state.partial_gnupg_secret_key;
        let secret_key_setting =
            self.add(egui::TextEdit::singleline(digest_buf).hint_text("private key digest"));
        // the key is ready to load if
        // (1) passphrase lost focus (request has been send) + digest is not empty
        // (2) digest lost focus + there's password in settings
        let settings = SETTINGS.lock().expect("settings are poisoned!");
        (passphrase_setting.lost_focus() && !digest_buf.is_empty())
            || (secret_key_setting.lost_focus() && settings.gnupg_passphrase_set())
    }
}
