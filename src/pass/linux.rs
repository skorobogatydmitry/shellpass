use std::{
    fmt::Display,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::Context;
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
        let output = Command::new("pass")
            .arg(entry.as_path())
            .output()
            .context("cannot retrieve entry from pass")?;
        let password =
            str::from_utf8(&output.stdout).context("unable to interpret output as UTF-8 string")?;
        Ok((entry.username(), password.trim().to_string()))
    }
}

#[derive(Clone)]
pub(crate) struct PassEntry {
    path_components: Vec<String>,
}

impl PassEntry {
    // TODO:  refactor away from calling pass command
    fn as_path(&self) -> String {
        self.path_components.join("/")
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
        write!(f, "{}", self.as_path())
    }
}
