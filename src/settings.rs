use std::{
    path::PathBuf,
    sync::{Arc, Condvar, LazyLock, Mutex, RwLock},
    thread,
};

use super::PassRepository;

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

    pub(crate) fn update_routine(&mut self, repository: Arc<RwLock<dyn PassRepository>>) {
        let fence = Arc::clone(&self.change_fence);
        thread::spawn(move || {
            let mut current_settings = SETTINGS.lock().expect("settings are poisoned!");
            loop {
                let mut repo = repository.write().expect("repository is poisoned!");
                // TODO: may not be a PathBuf for Android...
                repo.refresh_entries(current_settings.pass_root.as_ref().map(PathBuf::from));
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
