use std::{io::Write, process::Stdio};

use anyhow::{Context, anyhow};
use egui::{Response, Ui};
use log::info;
use pgp::composed::{Deserializable, SignedSecretKey};

use crate::{settings::SETTINGS, ui::UI_STATE};

impl super::OsUi for Ui {
    /// no special spacing is needed
    fn top_padding(&mut self) {}

    fn pass_root_setting(&mut self) {
        self.label("pass repository root");
        let mut settings = SETTINGS.lock().expect("settings are poisoned");
        let mut pass_root = settings.pass_root().unwrap_or_default();

        if self.text_edit_singleline(&mut pass_root).changed() {
            settings.set_pass_root(pass_root);
        }
    }
    fn gnupg_secret_key_settings(&mut self) -> Response {
        let mut ui_state = UI_STATE.lock().expect("UI state is poisoned!");
        let digest_buf = &mut ui_state.partial_gnupg_secret_key;
        self.add(egui::TextEdit::singleline(digest_buf).hint_text("private key digest"))
    }

    fn load_secret_key(passphrase: Option<&str>) -> anyhow::Result<SignedSecretKey> {
        let ui_state = UI_STATE.lock().expect("UI state is poisoned!");
        if passphrase.is_none() {
            return Err(anyhow!(
                "can't load key with just digest, without a passphrase"
            ));
        }
        let passphrase = passphrase.unwrap();

        let gpg_secret_key_export_cmd = std::process::Command::new("gpg")
            .args([
                "--pinentry-mode",
                "loopback",
                "--passphrase-fd",
                "0",
                "--export-secret-keys",
                ui_state.partial_gnupg_secret_key.as_str(), //"AF0E12DF50A47F57522FDB5346B290E986B754D8",
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
            .context("error on sending pasphrase to stdin")?;

        // TODO: timeout
        info!("waiting for the gpg command to finish");
        let output = gpg_secret_key_export_cmd
            .wait_with_output()
            .context("unable to retrieve the secret key")?;
        let stdout_bytes = output.stdout.as_slice();
        SignedSecretKey::from_bytes(stdout_bytes).context("error on loading secret key bytes")
    }
}
