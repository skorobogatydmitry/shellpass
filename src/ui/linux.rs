use std::{io::Write, process::Stdio};

use anyhow::{Context, anyhow};
use egui::Ui;
use pgp::composed::{Deserializable, SignedSecretKey};

use crate::{
    settings::{self, GnuPGSecretKeyProvider, SETTINGS, SettingsUpdateReq},
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
            let new_pass_root = pass_root_buf.clone();
            settings::send_update_request(SettingsUpdateReq::PassRoot(Box::new(|| {
                Ok(new_pass_root)
            })));
        }
    }

    fn gnupg_secret_key_settings(
        &mut self,
        passphrase_update_issued: bool,
    ) -> Option<GnuPGSecretKeyProvider> {
        let mut ui_state = UI_STATE.lock().expect("UI state is poisoned!");
        let digest_buf = &mut ui_state.partial_gnupg_secret_key;
        let secret_key_setting =
            self.add(egui::TextEdit::singleline(digest_buf).hint_text("private key digest"));
        // the key is ready to load if
        // (1) passphrase lost focus (request has been send) + digest is not empty
        // (2) digest lost focus + there's password in settings
        let settings = SETTINGS.lock().expect("settings are poisoned!");
        ((passphrase_update_issued && !digest_buf.is_empty())
            || (secret_key_setting.lost_focus() && settings.gnupg_passphrase_set()))
        .then(|| Box::new(read_secret_key) as GnuPGSecretKeyProvider)
    }
}

/// secret key readed for linux by digest from UI
fn read_secret_key() -> anyhow::Result<SignedSecretKey> {
    let ui_state = UI_STATE.lock().expect("UI state is poisoned!");
    let digest = ui_state.partial_gnupg_secret_key.clone();
    drop(ui_state);
    let settings = super::SETTINGS.lock().expect("settings are poisoned!");
    match settings.gnupg_passphrase() {
        Some(passphrase) => {
            let gpg_secret_key_export_cmd = std::process::Command::new("gpg")
                .args([
                    "--pinentry-mode",
                    "loopback",
                    "--passphrase-fd",
                    "0",
                    "--export-secret-keys",
                    digest.as_str(),
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()?;

            let mut stdin = gpg_secret_key_export_cmd
                .stdin
                .as_ref()
                .ok_or(anyhow!("can't get stdout of the secret key fetch command"))?;
            stdin
                .write_all(passphrase.as_bytes()) // TODO: security?
                .context("error on sending passphrase to stdin")?;

            // TODO: timeout
            log::info!("waiting for the gpg command to finish");
            let output = gpg_secret_key_export_cmd
                .wait_with_output()
                .context("unable to retrieve the secret key")?;
            let stdout_bytes = output.stdout.as_slice();
            SignedSecretKey::from_bytes(stdout_bytes).context("error on loading secret key bytes")
        }
        None => Err(anyhow!(
            "can't load key with just digest, without a passphrase"
        )),
    }
}
