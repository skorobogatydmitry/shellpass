use std::{
    fs::File,
    hint::black_box,
    io::Write,
    path::PathBuf,
    sync::{Arc, Condvar, LazyLock, Mutex},
    thread,
};

use anyhow::{Context, bail};
use pgp::{
    composed::{DecryptionOptions, SignedSecretKey, TheRing},
    types::Password,
};
use serde::{Deserialize, Serialize};

use crate::{
    finder::FINDER,
    pass::{PassRepository, REPOSITORY},
};

static STORED_SETTINGS_FILE_NAME: &str = concat!(env!("CARGO_PKG_NAME"), "-settings.bson");

pub static SETTINGS: LazyLock<Mutex<Settings>> = LazyLock::new(|| {
    Mutex::new(match Settings::try_load() {
        Ok(stored_settings) => stored_settings,
        Err(e) => {
            log::warn!("can't load settings, fallback to defaults: {e:#}"); // TODO: show to the end-user
            Settings::new()
        }
    })
});

/// tunable settings
#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    pass_root: Option<String>,
    #[serde(with = "with_bytes")]
    pub gnupg_secret_key: Option<SignedSecretKey>,
    #[serde(skip)] // don't store password longer that the app's lifetime
    gnupg_passphrase: Option<String>,
    #[serde(skip)] // purely internal structure - no need to preserve
    change_fence: Arc<Condvar>,
}

impl Settings {
    /// try to pickup previously stored settings
    fn try_load() -> anyhow::Result<Self> {
        let storing_file_path = Self::storing_file_path()?;
        if storing_file_path.exists() {
            bson::deserialize_from_reader(File::open(storing_file_path)?)
                .context("settings deserialization error")
        } else {
            bail!(
                "no config file found at {}",
                storing_file_path.to_string_lossy()
            )
        }
    }

    /// try to save settings
    pub(crate) fn try_save(&self) -> anyhow::Result<()> {
        let storing_file_path = Self::storing_file_path()?;
        let mut settings_file =
            File::create(storing_file_path).context("unable to (re)create settings file")?;
        settings_file
            .write_all(
                bson::serialize_to_vec(self)
                    .context("error on convesion settings to bson")?
                    .as_slice(),
            )
            .context("unable to write settings content to file")
    }

    /// define path to store settings at
    fn storing_file_path() -> anyhow::Result<PathBuf> {
        // TODO: android
        let config_dir = std::env::home_dir()
            .context("no home directory available to load settings")?
            .join(".config");
        if !config_dir.exists() {
            bail!("cannot lookup settings: ~/.config directory doesn't exist");
        }
        Ok(config_dir.join(STORED_SETTINGS_FILE_NAME))
    }

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
    #[allow(dead_code)] // android only
    pub(crate) fn gnupg_passphrase(&self) -> Option<&str> {
        self.gnupg_passphrase.as_deref()
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

/// serialize and deserialize SignedSecretKey as Vec<u8>
mod with_bytes {
    use pgp::{
        composed::{Deserializable, SignedSecretKey},
        ser::Serialize as _,
    };
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(key: &Option<SignedSecretKey>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match key {
            Some(key) => {
                let bytes = key.to_bytes().map_err(serde::ser::Error::custom)?;
                bytes.serialize(serializer)
            }
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<SignedSecretKey>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt: Option<Vec<u8>> = Option::deserialize(deserializer)?;
        match opt {
            Some(bytes) => SignedSecretKey::from_bytes(bytes.as_slice())
                .map(Some)
                .map_err(serde::de::Error::custom),
            None => Ok(None),
        }
    }
}
