use std::{
    hint::black_box,
    sync::{Arc, Condvar, LazyLock, Mutex},
    thread,
};

use pgp::{
    composed::{DecryptionOptions, SignedSecretKey, TheRing},
    types::Password,
};

use crate::{
    finder::FINDER,
    pass::{PassRepository, REPOSITORY},
};

pub static SETTINGS: LazyLock<Mutex<Settings>> = LazyLock::new(|| Mutex::new(Settings::new()));

/// tunable settings
pub struct Settings {
    pass_root: Option<String>,
    pub gnupg_secret_key: Option<SignedSecretKey>,
    gnupg_passphrase: Option<String>, // TODO: avoid storing password
    change_fence: Arc<Condvar>,
}

impl Settings {
    fn new() -> Self {
        Self {
            // TODO: refactor-off platform-specific defaults
            pass_root: if cfg!(target_os = "linux") {
                std::env::home_dir()
                    .map(|dir| dir.join(".password-store"))
                    .and_then(|dir| dir.to_str().map(|s| s.to_string()))
            } else {
                None
            },
            change_fence: Arc::new(Condvar::new()),
            gnupg_secret_key: None,
            gnupg_passphrase: None,
        }
    }

    pub(crate) fn update_routine(&mut self) {
        let fence = Arc::clone(&self.change_fence);
        thread::spawn(move || {
            let mut current_settings = SETTINGS.lock().expect("settings are poisoned!");
            loop {
                let mut repo = REPOSITORY.lock().expect("repository is poisoned!");
                if let Some(pass_root) = current_settings.pass_root.as_ref() {
                    repo.refresh_entries(pass_root);
                    // let the finder to refresh matches
                    let finder = FINDER.lock().expect("finder is poisoned!");
                    finder.change_fence.notify_one();
                }
                drop(repo);
                current_settings = fence
                    .wait(current_settings)
                    .expect("settings are poisoned!");
            }
        });
    }

    pub(crate) fn set_pass_root(&mut self, new_pass_root: String) -> Option<String> {
        if Some(&new_pass_root) != self.pass_root.as_ref() {
            let result = self.pass_root.replace(new_pass_root);
            self.change_fence.notify_one();
            result
        } else {
            None
        }
    }

    pub(crate) fn pass_root(&self) -> Option<String> {
        self.pass_root.clone()
    }

    pub fn has_gnupg_config(&self) -> bool {
        self.gnupg_passphrase.is_some() && self.gnupg_secret_key.is_some()
    }

    /// returns currect secret wrapped
    pub(crate) fn get_gnupg_secret(&self) -> Option<GnuPGSecret<'_>> {
        Some(GnuPGSecret {
            secret_key: self.gnupg_secret_key.as_ref()?,
            passphrase: Password::from(self.gnupg_passphrase.as_ref()?.clone()),
        })
    }

    /// reset the whole passphrase to the value provided
    /// make sure we don't leak tails of strings - it's always zero-ed
    /// TODO: avoid optimization on this method
    pub(crate) fn set_gnupg_passphrase(&mut self, passphrase: String) {
        let pp = black_box(self.gnupg_passphrase.take());
        if let Some(mut pp) = pp {
            // UNSAFE: we drain the content just after the loop => no need to be valid seq
            for byte in unsafe { pp.as_bytes_mut() } {
                *byte = 0u8;
            }
            pp.clear();
        }
        self.gnupg_passphrase = Some(passphrase);
    }
    pub(crate) fn gnupg_passphrase_set(&self) -> bool {
        self.gnupg_passphrase.is_some()
    }
    pub(crate) fn gnupg_passphrase(&self) -> Option<&str> {
        self.gnupg_passphrase.as_ref().map(|p| p.as_str())
    }
}

/// struct to store filled secrets and form TheRing
pub(crate) struct GnuPGSecret<'a> {
    pub secret_key: &'a SignedSecretKey,
    pub passphrase: Password,
}

impl<'a> GnuPGSecret<'a> {
    pub fn get_ring(&'a self) -> TheRing<'a> {
        TheRing {
            secret_keys: vec![&self.secret_key],
            key_passwords: vec![&self.passphrase],
            decrypt_options: DecryptionOptions::new().enable_gnupg_aead(),
            message_password: vec![],
            session_keys: vec![],
        }
    }
}
