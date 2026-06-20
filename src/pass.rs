use std::{
    fmt::Display,
    sync::{LazyLock, Mutex},
};

use anyhow::Context;
use pgp::composed::Message;

use crate::settings::GnuPGSecret;

pub static REPOSITORY: LazyLock<Mutex<PassRepositoryImpl>> =
    LazyLock::new(|| Mutex::new(PassRepositoryImpl::new()));

#[cfg(target_os = "android")]
pub(crate) mod android;
#[cfg(target_os = "linux")]
pub(crate) mod linux;

/// Required interface for pass repository
pub(crate) trait PassRepository<Y: PassEntry> {
    /// create new repository for the UI to access
    fn new() -> Self
    where
        Self: Sized;
    /// get all entries matching a given pattern
    fn get_by_pattern(&self, pattern: &str) -> Vec<&Y>;
    /// number of entries in the pass
    fn entries_count(&self) -> usize;
    /// update list of entries within the provided pass repository root
    fn refresh_entries(&mut self, pass_root: &str);
    /// get (username, password) of the given entry
    /// TODO: it's not a part of the trait, it seems
    fn retrieve(&self, entry: &Y, secret: GnuPGSecret) -> anyhow::Result<(String, String)> {
        let encrypted_data = entry.read()?;
        let msg = Message::from_bytes(encrypted_data.as_slice())
            .context("error on constructing encrypted message")?;
        // TODO: check if / how to make a decryption faster in debug with TheRing
        let (mut decrypted, _) = msg
            .decrypt_the_ring(secret.get_ring(), true)
            .context("error on decrypting the message")?;
        Ok((
            entry.username(),
            decrypted.as_data_string()?.trim().to_string(),
        ))
    }
}

/// a single entry in the pass repository
/// it reflests the path to entry within the pass repository
pub(crate) trait PassEntry: Display + Clone {
    fn contains(&self, pattern: &str) -> bool;
    fn username(&self) -> String;
    fn read(&self) -> anyhow::Result<Vec<u8>>;
}

#[cfg(target_os = "linux")]
pub(crate) type PassRepositoryImpl = linux::PassRepository;
#[cfg(target_os = "android")]
pub(crate) type PassRepositoryImpl = android::PassRepository;

#[cfg(target_os = "linux")]
pub(crate) type PassEntryImpl = linux::PassEntry;
#[cfg(target_os = "android")]
pub(crate) type PassEntryImpl = android::PassEntry;
