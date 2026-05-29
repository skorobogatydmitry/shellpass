use log::info;
use std::{
    sync::{Arc, Condvar, LazyLock, Mutex},
    thread,
};

use crate::pass::{PassEntry, PassEntryImpl, PassRepository, REPOSITORY};

pub static FINDER: LazyLock<Mutex<Finder<PassEntryImpl>>> =
    LazyLock::new(|| Mutex::new(Finder::new()));

pub(crate) struct Finder<U: PassEntry> {
    pub(crate) pattern: String,
    pub(crate) change_fence: Arc<Condvar>,
    pub(crate) last_match: Vec<U>,
}

impl Finder<PassEntryImpl> {
    fn new() -> Self {
        Self {
            pattern: String::new(),
            change_fence: Arc::new(Condvar::new()),
            last_match: Vec::new(),
        }
    }

    pub(crate) fn search_routine(&mut self) {
        let change_fence = Arc::clone(&self.change_fence);

        thread::spawn(move || {
            let mut finder = FINDER.lock().expect("pattern is poisoned!");
            loop {
                finder = change_fence.wait(finder).expect("pattern is poisoned!");
                let last_seen_pattern = finder.pattern.as_str();

                // TODO: make a faster swap
                let repository = REPOSITORY.lock().expect("repository is poisoned!");
                let new_items: Vec<PassEntryImpl> = repository
                    .get_by_pattern(last_seen_pattern)
                    .into_iter()
                    .cloned()
                    .collect();
                drop(repository);

                finder.last_match.clear();
                finder.last_match.extend(new_items);
                info!("found matches: {}", finder.last_match.len());
            }
        });
    }
}
