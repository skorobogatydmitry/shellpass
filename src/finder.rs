use log::info;
use std::{
    sync::{Arc, Condvar, LazyLock, Mutex},
    thread,
};

use crate::{
    Singleton,
    pass::{PassEntry, PassEntryImpl, PassRepository, RepositoryAccessor},
};

static FINDER: LazyLock<Mutex<Finder<PassEntryImpl>>> = LazyLock::new(|| Mutex::new(Finder::new()));

impl Singleton for Finder<PassEntryImpl> {
    fn storage() -> &'static LazyLock<Mutex<Self>> {
        &FINDER
    }
}

pub struct Finder<U: PassEntry> {
    pub pattern: String,
    pub last_match: Vec<U>,
    change_fence: Arc<Condvar>,
}

impl Finder<PassEntryImpl> {
    fn new() -> Self {
        Self {
            pattern: String::new(),
            change_fence: Arc::new(Condvar::new()),
            last_match: Vec::new(),
        }
    }

    pub fn initialize() {
        let change_fence = Arc::clone(&Finder::get().change_fence);

        thread::spawn(move || {
            let mut finder = Finder::get();
            loop {
                finder = change_fence.wait(finder).expect("pattern is poisoned!");
                let last_seen_pattern = finder.pattern.as_str();

                finder.last_match = PassRepository::get_by_pattern(last_seen_pattern);
                info!("found matches: {}", finder.last_match.len());
            }
        });
    }

    /// let the finder know that it's time to search for matches
    pub fn notify() {
        Self::get().change_fence.notify_one()
    }
}
