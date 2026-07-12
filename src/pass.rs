use std::{
    fmt::Display,
    hint,
    io::Read,
    path::PathBuf,
    sync::{LazyLock, Mutex},
};

use anyhow::Context;
use pgp::composed::Message;

use crate::{Singleton, notifications, settings::GnuPGSecret};

static REPOSITORY: LazyLock<Mutex<PassRepository<PassEntryImpl>>> =
    LazyLock::new(|| Mutex::new(PassRepository::new()));

impl Singleton for PassRepository<PassEntryImpl> {
    fn storage() -> &'static LazyLock<Mutex<Self>> {
        &REPOSITORY
    }
}

#[cfg(target_os = "android")]
pub(crate) mod android;
#[cfg(target_os = "linux")]
pub(crate) mod linux;

/// Required interface for pass repository
pub(crate) trait RepositoryAccessor<Y: PassEntry> {
    /// get all entries matching a given pattern
    fn get_by_pattern(pattern: &str) -> Vec<Y>;
    /// number of entries in the pass
    fn entries_count() -> usize;
    /// fetch list of entries within the provided pass repository root
    fn fetch_entries_for(pass_root: &str);
}

/// a single entry in the pass repository
/// it reflests the path to entry within the pass repository
pub(crate) trait PassEntry: Display + Clone {
    /// search for invocations only in pass entry's displayed to the user
    fn contains(&self, pattern: &str) -> bool {
        self.to_string().contains(pattern)
    }
    /// it's my personal convention: file name is a username
    fn username(&self) -> String {
        self.entry_relpath()
            .file_stem()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or("n/a".to_string())
    }
    /// file path relative to the root
    fn entry_relpath(&self) -> &PathBuf;
    /// read entry's file content
    fn read(&self) -> anyhow::Result<Vec<u8>>;

    /// get (username, password) of the given entry
    fn retrieve(&self, secret: GnuPGSecret) -> anyhow::Result<(String, String)> {
        let encrypted_data = self.read()?;
        let msg = Message::from_bytes(encrypted_data.as_slice())
            .context("error on constructing encrypted message")?;
        // TODO: check if / how to make a decryption faster in debug with TheRing
        let (mut decrypted, _) = msg
            .decrypt_the_ring(secret.get_ring(), true)
            .context("cannot decrypt the message")?;
        // TODO: potentially the messages leaks sensitive data here
        // but it's not in our control
        let mut password = String::new();
        decrypted.read_to_string(&mut password)?;
        // pass entries typically end with \n
        if let Some(last_char) = password.chars().last()
            && last_char == '\n'
        {
            password.pop();
        }
        Ok((self.username(), password))
    }
}

#[cfg(target_os = "linux")]
pub(crate) type PassEntryImpl = linux::PassEntry;
#[cfg(target_os = "android")]
pub(crate) type PassEntryImpl = android::PassEntry;

impl Display for PassEntryImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut relpath = self.entry_relpath().clone();
        relpath.set_extension("");
        write!(f, "{}", relpath.to_string_lossy())
    }
}

pub(crate) struct PassRepository<Y: PassEntry> {
    entries: Vec<Y>,
    last_used_root: Option<String>,
}

// Non paltform-specific functionality
impl PassRepository<PassEntryImpl> {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            last_used_root: None,
        }
    }

    /// reset the respository metadata
    pub fn reset() {
        let mut repo = Self::get();
        repo.entries.clear();
        repo.last_used_root = None;
    }

    /// just refresh list of entries for the current pass root
    pub fn refresh_entries() {
        let last_used_root = Self::get().last_used_root.as_ref().cloned();
        match last_used_root {
            Some(last_pass_root) => {
                Self::fetch_entries_for(last_pass_root.as_str());
            }
            None => notifications::push_message(notifications::Message::new(
                "pass root is not set".to_string(),
                notifications::Kind::Warning,
            )),
        }
    }
}

/// a method to clear sensitive strings after use
// TODO: disallow optimizing-out the call
pub fn clear_string(s: String) {
    let mut s = hint::black_box(s);
    for byte in unsafe { s.as_bytes_mut() } {
        *byte = 0u8;
    }
    s.clear();
}
