use std::{
    env,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use anyhow::Context;
use log::warn;
use walkdir::WalkDir;

/// # Desc
/// GNU pass repository read-only access manager.
pub struct PassRepository {
    entries: Vec<PassEntry>,
}

impl PassRepository {
    pub(crate) fn new() -> anyhow::Result<Self> {
        let pass_home = env::home_dir()
            .context("cannot get home folder")?
            .join(".password-store");
        Ok(Self {
            entries: Self::enumerate_entries(pass_home),
        })
    }

    pub(crate) fn get_by_pattern(&self, pattern: &str) -> Vec<PassEntry> {
        self.entries
            .iter()
            .filter(|e| e.contains(pattern))
            .cloned()
            .collect()
    }

    pub(crate) fn entries_count(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn retrieve(&self, entry: &PassEntry) -> anyhow::Result<(String, String)> {
        let output = Command::new("pass")
            .arg(entry.to_string())
            .output()
            .context("cannot retrieve entry from pass")?;
        let password =
            str::from_utf8(&output.stdout).context("unable to interpret output as UTF-8 string")?;
        Ok((entry.username(), password.trim().to_string()))
    }

    fn enumerate_entries(base_dir: PathBuf) -> Vec<PassEntry> {
        WalkDir::new(&base_dir)
            .into_iter()
            .filter_map(|e| {
                e.ok().and_then(|e| {
                    if e.file_type().is_file()
                        && e.file_name()
                            .to_str()
                            .filter(|s| s.ends_with(".gpg"))
                            .is_some()
                    {
                        // UNWRAP: all paths have base_dir as prefix
                        Some(PassEntry::from(
                            e.into_path().strip_prefix(&base_dir).unwrap(),
                        ))
                    } else {
                        None
                    }
                })
            })
            .collect()
    }
}

/// a single entry in the pass repository
/// it reflests the path to entry within the pass repository
#[derive(Clone)]
pub(crate) struct PassEntry {
    path_components: Vec<String>,
}

impl PassEntry {
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

impl ToString for PassEntry {
    fn to_string(&self) -> String {
        self.path_components.join("/")
    }
}
