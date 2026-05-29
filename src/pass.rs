use std::{
    fmt::Display,
    path::Path,
    sync::{LazyLock, Mutex},
};

pub static REPOSITORY: LazyLock<Mutex<PassRepositoryImpl>> =
    LazyLock::new(|| Mutex::new(PassRepositoryImpl::new()));

#[cfg(target_os = "android")]
pub(crate) mod android;
#[cfg(target_os = "linux")]
pub(crate) mod linux;

/// Required interface for pass repository
/// TODO: why Arc<RwLock<...>> require Send + Sync for the inner ?
pub(crate) trait PassRepository<Y: PassEntry>: Send + Sync {
    /// create new repository for the UI to access
    fn new() -> Self
    where
        Self: Sized;
    /// get all entries matching a given pattern
    fn get_by_pattern(&self, pattern: &str) -> Vec<&Y>;
    /// number of entries in the pass
    fn entries_count(&self) -> usize;
    /// get (username, password) of the given entry
    fn retrieve(&self, entry: &Y) -> anyhow::Result<(String, String)>;
    /// update list of entries within the provided pass repository root
    fn refresh_entries(&mut self, pass_root: &str);
}

/// a single entry in the pass repository
/// it reflests the path to entry within the pass repository
pub(crate) trait PassEntry: Display + Clone {
    fn contains(&self, pattern: &str) -> bool;
    fn username(&self) -> String;
}

#[cfg(target_os = "linux")]
pub(crate) type PassRepositoryImpl = linux::PassRepository;
#[cfg(target_os = "android")]
pub(crate) type PassRepositoryImpl = android::PassRepository;

#[cfg(target_os = "linux")]
pub(crate) type PassEntryImpl = linux::PassEntry;
#[cfg(target_os = "android")]
pub(crate) type PassEntryImpl = android::PassEntry;
