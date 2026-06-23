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
        let settings = SETTINGS.lock().expect("settings are poisoned!");
        self.label(match settings.pass_root() {
            Some(current_pass_root) => format!("is set to '{}'", current_pass_root),
            None => "is not set".to_string(),
        });
        drop(settings);

        let mut ui_state = UI_STATE.lock().expect("UI state is poisoned!");
        let pass_root_buf = &mut ui_state.partial_pass_root;

        if self.text_edit_singleline(pass_root_buf).lost_focus() {
            settings::send_update_request(SettingsUpdateReq::PassRoot(pass_root_buf.clone()));
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
