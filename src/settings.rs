use std::{
    fs::File,
    hint::black_box,
    io::Write,
    path::PathBuf,
    sync::{
        LazyLock, Mutex, OnceLock,
        mpsc::{self, Sender},
    },
    thread,
    time::Duration,
};

use anyhow::{Context, bail};
use pgp::{
    composed::{DecryptionOptions, SignedSecretKey, TheRing},
    types::{KeyDetails, Password},
};
use serde::{Deserialize, Serialize};

use crate::{
    Singleton,
    finder::Finder,
    notifications::{self, Kind, Message},
    pass::{PassRepository, RepositoryAccessor, clear_string},
};

const DEFAULT_ZOOM_FACTOR: f32 = 1.7;
static STORED_SETTINGS_FILE_NAME: &str = concat!(env!("CARGO_PKG_NAME"), "-settings.bson");
static SETTINGS_UPDATE_EVENT_QUEUE: OnceLock<Sender<SettingsUpdateReq>> = OnceLock::new();

fn default_zoom_factor() -> f32 {
    DEFAULT_ZOOM_FACTOR
}

static SETTINGS: LazyLock<Mutex<Settings>> = LazyLock::new(|| {
    Mutex::new(match Settings::try_load() {
        Ok(stored_settings) => stored_settings,
        Err(e) => {
            notifications::push_message(
                Message::new(
                    format!("no saved settings: {e:#}, loading defaults"),
                    Kind::Warning,
                )
                .with_duration(Duration::from_secs(20)),
            );
            Settings::new()
        }
    })
});

impl Singleton for Settings {
    fn storage() -> &'static LazyLock<Mutex<Self>> {
        &SETTINGS
    }
}

/// runtime settings update request types
pub enum SettingsUpdateReq {
    PassRoot(Box<dyn FnOnce() -> anyhow::Result<String> + Send>),
    GnuPGPassphrase(String),
    GnuPGSecretKey(GnuPGSecretKeyProvider),
    ZoomFactor(f32),
    Reset,
}

pub type GnuPGSecretKeyProvider = Box<dyn FnOnce() -> anyhow::Result<SignedSecretKey> + Send>;

impl SettingsUpdateReq {
    /// public API to send update requests to settings
    pub fn send(self) {
        let settings_event_queue = SETTINGS_UPDATE_EVENT_QUEUE
            .get()
            .expect("settings update queue is not ready");
        settings_event_queue
            .send(self)
            .expect("error on sending settings update request");
    }

    // apply the request right away
    // returns whether the setting was applied
    pub fn apply(self) -> bool {
        match self {
            SettingsUpdateReq::PassRoot(new_pass_root_provider) => {
                match new_pass_root_provider() {
                    Ok(new_pass_root) => {
                        PassRepository::fetch_entries_for(new_pass_root.as_str());
                        // let the finder refresh matches
                        Finder::notify();
                        Settings::get().pass_root.replace(new_pass_root);
                        return true;
                    }
                    Err(e) => {
                        notifications::push_message(Message::new(
                            format!("cannot set new pass root: {e:#}"),
                            Kind::Error,
                        ));
                    }
                }
            }
            SettingsUpdateReq::GnuPGPassphrase(new_pp) => {
                // reset the whole passphrase to the new value
                // make sure we don't leak tails of strings - it's always zero-ed
                // TODO: disable optimization for the following code
                let pp = black_box(Settings::get().gnupg_passphrase.replace(new_pp));
                if let Some(pp) = pp {
                    clear_string(pp);
                }
                // there's no need to save settings - passphrase is ephemeral
            }
            SettingsUpdateReq::GnuPGSecretKey(secret_key_provider) => match secret_key_provider() {
                Ok(key) => {
                    Settings::get().gnupg_secret_key.replace(key);
                    return true;
                }
                Err(e) => {
                    notifications::push_message(Message::new(
                        format!("unable to read secret key file: {e:#}"),
                        Kind::Error,
                    ));
                }
            },
            SettingsUpdateReq::Reset => {
                {
                    let mut settings = Settings::get();
                    settings.reset_passphrase();
                    settings.gnupg_secret_key = None;
                    settings.pass_root = None;
                }
                PassRepository::reset();
                Finder::notify();

                // flush the saved settings
                return true;
            }
            SettingsUpdateReq::ZoomFactor(new_zoom) => {
                let mut settings = Settings::get();
                if settings.zoom_factor != new_zoom {
                    settings.zoom_factor = new_zoom;
                    return true;
                }
            }
        }
        return false;
    }
}

/// tunable settings
#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    pass_root: Option<String>,
    #[serde(default = "default_zoom_factor")]
    pub zoom_factor: f32,
    #[serde(with = "with_bytes")]
    gnupg_secret_key: Option<SignedSecretKey>,
    #[serde(skip)] // don't store password longer that the app's lifetime
    gnupg_passphrase: Option<String>,
}

impl Settings {
    fn new() -> Self {
        // TODO: refactor-off platform-specific defaults
        Self {
            pass_root: if cfg!(target_os = "linux") {
                std::env::home_dir()
                    .map(|dir| dir.join(".password-store"))
                    .and_then(|dir| dir.to_str().map(|s| s.to_string()))
            } else {
                None
            },
            zoom_factor: DEFAULT_ZOOM_FACTOR,
            gnupg_secret_key: None,
            gnupg_passphrase: None,
        }
    }

    /// initializes settings update events queue and
    /// spawns a thread to do heavy-lifting settings updates
    /// the thread keeps runtime state in-sync with settings by propagating new vaues to respecitve componets
    pub fn initialize() {
        let (tx, rx) = mpsc::channel();

        if let Some(loaded_pass_root) = Settings::pass_root() {
            tx.send(SettingsUpdateReq::PassRoot(Box::new(|| {
                Ok(loaded_pass_root)
            })))
            .expect("cannot send pass root initialization");
        }

        SETTINGS_UPDATE_EVENT_QUEUE
            .set(tx)
            .expect("settings update queue initialization failed");

        thread::spawn(move || {
            loop {
                let request = rx
                    .recv()
                    .context("settings update event channel is closed")
                    .unwrap();

                let request_applied = request.apply();

                if request_applied && let Err(e) = Self::try_save() {
                    // decided not to crash, as it's not fatal and causes the user to re-enter settings on restart
                    notifications::push_message(Message::new(
                        format!("error on saving settings: {e:#}"),
                        Kind::Error,
                    ));
                }
            }
        });
    }

    /// platform-specific folders to store settings
    #[cfg(target_os = "android")]
    fn settings_store_dir() -> anyhow::Result<PathBuf> {
        use jni_min_helper::android_app_files_dir;

        let config_dir = android_app_files_dir();
        if !config_dir.exists() {
            bail!("cannot load/store settings: config directory doesn't exist");
        }
        Ok(config_dir.to_path_buf())
    }

    #[cfg(target_os = "linux")]
    fn settings_store_dir() -> anyhow::Result<PathBuf> {
        let config_dir = std::env::home_dir()
            .context("no home directory available to load/store settings")?
            .join(".config");
        if !config_dir.exists() {
            bail!("cannot load/store settings: ~/.config directory doesn't exist");
        }
        Ok(config_dir)
    }

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
    fn try_save() -> anyhow::Result<()> {
        let storing_file_path = Self::storing_file_path()?;
        let mut settings_file =
            File::create(storing_file_path).context("unable to (re)create settings file")?;
        settings_file
            .write_all(
                bson::serialize_to_vec(&*Self::get())
                    .context("error on convesion settings to bson")?
                    .as_slice(),
            )
            .context("unable to write settings content to file")
    }

    /// define path to store settings at
    fn storing_file_path() -> anyhow::Result<PathBuf> {
        Ok(Self::settings_store_dir()?.join(STORED_SETTINGS_FILE_NAME))
    }

    pub fn pass_root() -> Option<String> {
        Self::get().pass_root.clone()
    }

    /// returns currect secret wrapped
    pub fn get_gnupg_secret(&self) -> Option<GnuPGSecret<'_>> {
        Some(GnuPGSecret {
            secret_key: self.gnupg_secret_key.as_ref()?,
            passphrase: Password::from(self.gnupg_passphrase.as_ref()?.clone()),
        })
    }

    pub fn gnupg_passphrase_set(&self) -> bool {
        self.gnupg_passphrase.is_some()
    }

    #[allow(dead_code)] // linux only
    pub fn with_gnupg_passphrase(
        &self,
        payload: impl FnOnce(Option<&str>) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        payload(self.gnupg_passphrase.as_deref())
    }

    pub fn gnupg_secret_key_digest(&self) -> Option<String> {
        self.gnupg_secret_key
            .as_ref()
            .map(|k| k.primary_key.fingerprint().to_string())
    }

    // TODO: disallow optimizing out the method
    pub fn reset_passphrase(&mut self) {
        let old_pp = std::hint::black_box(self.gnupg_passphrase.take());
        if let Some(old_pp) = old_pp {
            clear_string(old_pp);
        }
    }
}

/// struct to store filled secrets and form TheRing
pub struct GnuPGSecret<'a> {
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
