use std::{
    sync::{Arc, Condvar, LazyLock, Mutex},
    thread,
};

use crate::pass::{PassRepository, REPOSITORY};

pub static SETTINGS: LazyLock<Mutex<Settings>> = LazyLock::new(|| Mutex::new(Settings::new()));

/// tunable settings
pub struct Settings {
    pass_root: Option<String>,
    change_fence: Arc<Condvar>,
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
            change_fence: Arc::new(Condvar::new()),
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
}
