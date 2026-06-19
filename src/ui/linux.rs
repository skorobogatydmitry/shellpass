use std::{io::Write, process::Stdio};

use anyhow::{Context, anyhow};
use egui::Ui;
use log::{error, info};
use pgp::{
    composed::{Deserializable, SignedSecretKey},
    types::KeyDetails,
};

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
    fn gnupg_settings(&mut self) {
        let mut settings = SETTINGS.lock().expect("settings are poisoned!");
        let mut ui_state = UI_STATE.lock().expect("UI state is poisoned!");

        let settings_key_digest = settings
            .gnupg_secret_key
            .as_ref()
            .map(|k| k.primary_key.fingerprint().to_string());

        self.label(if settings.gnupg_passphrase_set() {
            "key passphrase is set"
        } else {
            "no passphrase set"
        });

        let passphrase_ui_buf = &mut ui_state.partial_gnupg_passphrase;
        let passphrase_edit = self.add(
            egui::TextEdit::singleline(passphrase_ui_buf)
                .hint_text("passphrase for secret key")
                .password(true),
        );
        if passphrase_edit.lost_focus() {
            let mut pp = String::new();
            std::mem::swap(passphrase_ui_buf, &mut pp);
            settings.set_gnupg_passphrase(pp);
        }

        self.label(match settings_key_digest {
            None => "no secret key loaded".to_string(),
            Some(settings_digest) => format!("current key digest: {settings_digest}"),
        });

        let digest_buf = &mut ui_state.partial_gnupg_secret_key;
        let digest_edit =
            self.add(egui::TextEdit::singleline(digest_buf).hint_text("private key digest"));

        // try to initialize the key using digest and passphrase
        // digest in the settings can't be used here, as it can only be set if the previous load succeeded
        // so, even for passphrase change we rely on that the buffer has digest to load
        if digest_edit.lost_focus() || passphrase_edit.lost_focus() {
            info!("start loading secret key with digest {digest_buf} and passphrase");
            match load_secret_key(Some(digest_buf.as_str()), settings.gnupg_passphrase()) {
                Ok(secret_key) => {
                    info!("moving secret key to settings");
                    settings.gnupg_secret_key = Some(secret_key);
                }
                Err(e) => {
                    // TODO: show to the end-user
                    error!("could not load key by digest {digest_buf}: {e}");
                }
            }
        }
    }
}

fn load_secret_key(
    digest: Option<&str>,
    passphrase: Option<&str>,
) -> anyhow::Result<SignedSecretKey> {
    if digest.is_none() {
        return Err(anyhow!("no digest to load"));
    }
    if passphrase.is_none() {
        return Err(anyhow!(
            "can't load key with just digest, without a passphrase"
        ));
    }
    let (digest, passphrase) = (digest.unwrap(), passphrase.unwrap());
    let gpg_secret_key_export_cmd = std::process::Command::new("gpg")
        .args([
            "--pinentry-mode",
            "loopback",
            "--passphrase-fd",
            "0",
            "--export-secret-keys",
            digest, //"AF0E12DF50A47F57522FDB5346B290E986B754D8",
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
