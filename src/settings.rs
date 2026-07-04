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
    finder::FINDER,
    notifications::{self, Kind, Message},
    pass::{REPOSITORY, RepositoryAccessor, clear_string},
};

const DEFAULT_ZOOM_FACTOR: f32 = 1.7;
static STORED_SETTINGS_FILE_NAME: &str = concat!(env!("CARGO_PKG_NAME"), "-settings.bson");
static SETTINGS_UPDATE_EVENT_QUEUE: OnceLock<Sender<SettingsUpdateReq>> = OnceLock::new();

fn default_zoom_factor() -> f32 {
    DEFAULT_ZOOM_FACTOR
}

pub static SETTINGS: LazyLock<Mutex<Settings>> = LazyLock::new(|| {
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

pub enum SettingsUpdateReq {
    PassRoot(Box<dyn FnOnce() -> anyhow::Result<String> + Send>),
    GnuPGPassphrase(String),
    GnuPGSecretKey(GnuPGSecretKeyProvider),
    ZoomFactor(f32),
    Reset,
}

pub type GnuPGSecretKeyProvider = Box<dyn FnOnce() -> anyhow::Result<SignedSecretKey> + Send>;

/// initializes settings update events queue and
/// spawns a thread to do heavy-lifting settings updates
pub fn initialize() {
    let (tx, rx) = mpsc::channel();

    let settings = SETTINGS.lock().expect("settings are poisoned!");
    if let Some(loaded_pass_root) = settings.pass_root.as_ref() {
        let new_pass_root = loaded_pass_root.clone();
        tx.send(SettingsUpdateReq::PassRoot(Box::new(|| Ok(new_pass_root))))
            .expect("cannot send pass root initialization");
    }

    SETTINGS_UPDATE_EVENT_QUEUE
        .set(tx)
        .expect("settings update queue initialization failed");

    thread::spawn(move || {
        loop {
            match rx.recv() {
                Err(e) => {
                    // TODO: crash the app
                    notifications::push_message(Message::new(
                        format!("settings update event channel is closed: {e:#}"),
                        Kind::Error,
                    ));
                }

                Ok(request) => {
                    let mut settings_updated = false;
                    match request {
                        SettingsUpdateReq::PassRoot(new_pass_root_provider) => {
                            match new_pass_root_provider() {
                                Ok(new_pass_root) => {
                                    let mut settings =
                                        SETTINGS.lock().expect("settings are poisoned!");
                                    let mut repo =
                                        REPOSITORY.lock().expect("repository is poisoned!");
                                    repo.fetch_entries_for(new_pass_root.as_str());
                                    drop(repo);
                                    // let the finder refresh matches
                                    let finder = FINDER.lock().expect("finder is poisoned!");
                                    finder.change_fence.notify_one();
                                    settings.pass_root.replace(new_pass_root);
                                    settings_updated = true;
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
                            let mut settings = SETTINGS.lock().expect("settings are poisoned!");
                            let pp = black_box(settings.gnupg_passphrase.replace(new_pp));
                            drop(settings);
                            if let Some(pp) = pp {
                                clear_string(pp);
                            }
                            // there's no need to save settings here
                        }
                        SettingsUpdateReq::GnuPGSecretKey(secret_key_provider) => {
                            match secret_key_provider() {
                                Ok(key) => {
                                    let mut settings =
                                        SETTINGS.lock().expect("settings are poisoned!");
                                    settings.gnupg_secret_key = Some(key);
                                }
                                Err(e) => {
                                    notifications::push_message(Message::new(
                                        format!("unable to read secret key file: {e:#}"),
                                        Kind::Error,
                                    ));
                                }
                            }
                            settings_updated = true;
                        }
                        SettingsUpdateReq::Reset => {
                            let mut settings = SETTINGS.lock().expect("settings are poisoned!");
                            let old_pp = settings.gnupg_passphrase.take();
                            if let Some(old_pp) = old_pp {
                                clear_string(old_pp);
                            }
                            settings.gnupg_secret_key = None;
                            settings.pass_root = None;
                            let mut repo = REPOSITORY.lock().expect("repository is poisoned!");
                            repo.clear();
                            drop(repo);
                            let finder = FINDER.lock().expect("finder is poisoned!");
                            finder.change_fence.notify_one();

                            // flush the saved settings
                            settings_updated = true;
                        }
                        SettingsUpdateReq::ZoomFactor(new_zoom) => {
                            let mut settings = SETTINGS.lock().expect("settings are poisoned!");
                            if settings.zoom_factor != new_zoom {
                                settings.zoom_factor = new_zoom;
                                settings_updated = true;
                            }
                        }
                    }
                    if settings_updated {
                        let settings = SETTINGS.lock().expect("settings are poisoned!");
                        if let Err(e) = settings.try_save() {
                            // decided not to crash, as it's not fatal and causes the user to re-enter settings on restart
                            notifications::push_message(Message::new(
                                format!("error on saving settings: {e:#}"),
                                Kind::Error,
                            ));
                        }
                    }
                }
            }
        }
    });
}

/// public API to send update requests to settings
pub fn send_update_request(request: SettingsUpdateReq) {
    let settings_event_queue = SETTINGS_UPDATE_EVENT_QUEUE
        .get()
        .expect("settings update queue is not ready");
    settings_event_queue
        .send(request)
        .expect("error on sending settings update request");
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
        // TODO: refactor-off platforsm-specific defaults
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
    fn try_save(&self) -> anyhow::Result<()> {
        let storing_file_path = Self::storing_file_path()?;
        let mut settings_file =
            File::create(storing_file_path).context("unable to (re)create settings file")?;
        settings_file
            .write_all(
                bson::serialize_to_vec(&self)
                    .context("error on convesion settings to bson")?
                    .as_slice(),
            )
            .context("unable to write settings content to file")
    }

    /// define path to store settings at
    fn storing_file_path() -> anyhow::Result<PathBuf> {
        Ok(Self::settings_store_dir()?.join(STORED_SETTINGS_FILE_NAME))
    }

    pub fn pass_root(&self) -> Option<String> {
        self.pass_root.clone()
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

    #[allow(dead_code)] // android only
    pub fn gnupg_passphrase(&self) -> Option<&str> {
        self.gnupg_passphrase.as_deref()
    }

    pub fn gnupg_secret_key_digest(&self) -> Option<String> {
        self.gnupg_secret_key
            .as_ref()
            .map(|k| k.primary_key.fingerprint().to_string())
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
