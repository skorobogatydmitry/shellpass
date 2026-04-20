use std::{
    env,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

use anyhow::Context;
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

    pub(crate) fn get_by_pattern(&self, pattern: &str) -> Vec<String> {
        self.entries
            .iter()
            .filter_map(|e| e.contains(pattern).then(|| e.to_string()))
            .collect()
    }

    pub(crate) fn entries_count(&self) -> usize {
        self.entries.len()
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
pub(crate) struct PassEntry {
    path_components: Vec<String>,
}

impl PassEntry {
    fn contains(&self, pattern: &str) -> bool {
        self.to_string().contains(pattern)
    }
}

impl From<&Path> for PassEntry {
    fn from(value: &Path) -> Self {
        let mut path_components = value
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect::<Vec<String>>();
        // strip extension
        path_components
            .last_mut()
            .map(|file_name| file_name.truncate(file_name.len() - 4));

        Self { path_components }
    }
}

impl ToString for PassEntry {
    fn to_string(&self) -> String {
        self.path_components.join("/")
    }
}
