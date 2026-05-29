use std::{
    fmt::Display,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, anyhow};
use gpgme::Protocol;
use walkdir::WalkDir;

use super::PassEntry as _PassEntry;

/// Simple implementation for Linux
/// - list .gpg files in ~/.password-store to find all entries
/// - use `pass ...` command to get passwords
pub struct PassRepository {
    entries: Vec<PassEntry>,
    last_used_root: Option<PathBuf>,
}

impl super::PassRepository<PassEntry> for PassRepository {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            last_used_root: None,
        }
    }

    fn refresh_entries(&mut self, pass_root: &str) {
        let pass_root = PathBuf::from(pass_root);
        // TODO: return if any entries were updated
        self.entries = WalkDir::new(&pass_root)
            .into_iter()
            .filter_map(|e| {
                e.ok().and_then(|e| {
                    if e.file_type().is_file()
                        && e.file_name()
                            .to_str()
                            .filter(|s| s.ends_with(".gpg"))
                            .is_some()
                    {
                        // UNWRAP: all paths have pass_root as prefix
                        Some(PassEntry::from(
                            e.into_path().strip_prefix(&pass_root).unwrap(),
                        ))
                    } else {
                        None
                    }
                })
            })
            .collect();
        // TODO: do this only on success
        self.last_used_root = Some(pass_root);
    }

    fn get_by_pattern(&self, pattern: &str) -> Vec<&PassEntry> {
        self.entries
            .iter()
            .filter(|e| e.contains(pattern))
            .collect()
    }

    fn entries_count(&self) -> usize {
        self.entries.len()
    }

    fn retrieve(&self, entry: &PassEntry) -> anyhow::Result<(String, String)> {
        // let key_file = PathBuf::from("/home/dk/.gnupg/pubring.kbx");
        let mut ctx = gpgme::Context::from_protocol(Protocol::OpenPgp)
            .context("cannot initialize context for GnuPG")?;
        let root = self
            .last_used_root
            .as_ref()
            .ok_or(anyhow!(
                "no root to retrieve entries (it's the program state consistency problem)"
            ))?
            .clone();
        let full_entry_path = root.join(entry.path());
        let mut input = fs::File::open(full_entry_path).context(entry.clone())?;
        let mut output = Vec::new();

        ctx.decrypt(&mut input, &mut output)
            .context("error decrypting an entry")?;

        Ok((
            entry.username(),
            String::from_utf8(output)?.trim().to_string(),
        ))
    }
}

#[derive(Clone)]
pub(crate) struct PassEntry {
    path_components: Vec<String>,
}

impl PassEntry {
    fn path(&self) -> String {
        format!("{}.gpg", self.path_components.join("/"))
    }
}

impl super::PassEntry for PassEntry {
    fn contains(&self, pattern: &str) -> bool {
        self.to_string().contains(pattern)
    }

    /// expect the last part of the entry to be username
    fn username(&self) -> String {
        // TODO: make sure all entries have >=1 components
        self.path_components
            .last()
            .expect("no last component!")
            .clone()
    }
}

impl From<&Path> for PassEntry {
    fn from(value: &Path) -> Self {
        let mut path_components = value
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect::<Vec<String>>();
        // strip extension
        if let Some(file_name) = path_components.last_mut() {
            file_name.truncate(file_name.len() - 4)
        }

        Self { path_components }
    }
}

impl Display for PassEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path_components.join("/"))
    }
}
