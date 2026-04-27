use std::env;
// use std::path::PathBuf;
use std::sync::RwLock;

pub static SETTINGS: RwLock<Settings> = RwLock::new(Settings::new());

/// tunable settings
pub struct Settings {
    pub pass_root: Option<String>,
    /// whether all the settings are applied
    pub applied: bool,
}

impl Settings {
    const fn new() -> Self {
        Self {
            pass_root: None,
            applied: true,
        }
    }

    /// read statically stored settings merged with defaults`
    pub(crate) fn load(&mut self) -> anyhow::Result<()> {
        // TODO: keep it None for android
        self.pass_root = env::home_dir()
            .map(|dir| dir.join(".password-store"))
            .and_then(|dir| dir.to_str().map(|s| s.to_string()));
        // TODO: store settings permanently
        Ok(())
    }

    /// save settings to a permanent storage
    pub(crate) fn save(&mut self) {
        if self.applied {
        }
        // todo!()
    }
}
