use std::sync::{LazyLock, Mutex};

pub static SETTINGS: LazyLock<Mutex<Settings>> = LazyLock::new(|| Mutex::new(Settings::new()));

/// tunable settings
pub struct Settings {
    pub pass_root: Option<String>,
    /// whether all the settings are applied
    pub applied: bool,
}

impl Settings {
    fn new() -> Self {
        Self {
            pass_root: if cfg!(target_os = "linux") {
                std::env::home_dir()
                    .map(|dir| dir.join(".password-store"))
                    .and_then(|dir| dir.to_str().map(|s| s.to_string()))
            } else {
                None
            },
            applied: true,
        }
    }
}
