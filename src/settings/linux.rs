use std::{io::Write, process::Stdio};

use anyhow::{Context, anyhow};
use pgp::composed::{Deserializable, SignedSecretKey};

impl super::OsSettings for super::Settings {
    fn read_secret_key(digest: String) -> anyhow::Result<SignedSecretKey> {
        let settings = super::SETTINGS.lock().expect("settings are poisoned!");
        if settings.gnupg_passphrase.is_none() {
            return Err(anyhow!(
                "can't load key with just digest, without a passphrase"
            ));
        }
        let passphrase = settings.gnupg_passphrase.as_ref().unwrap();

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
            .context("error on sending pasphrase to stdin")?;

        // TODO: timeout
        log::info!("waiting for the gpg command to finish");
        let output = gpg_secret_key_export_cmd
            .wait_with_output()
            .context("unable to retrieve the secret key")?;
        let stdout_bytes = output.stdout.as_slice();
        SignedSecretKey::from_bytes(stdout_bytes).context("error on loading secret key bytes")
    }
}
