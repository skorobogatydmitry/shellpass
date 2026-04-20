use std::{env, path::PathBuf};

use anyhow::Context;
use walkdir::WalkDir;

/// # Desc
/// GNU pass repository read-only access manager.
pub struct PassRepository {
    entries: Vec<String>,
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

    pub(crate) fn get_by_pattern(&self, _pattern: &str) -> Vec<String> {
        vec![]
    }

    pub(crate) fn entries_count(&self) -> usize {
        self.entries.len()
    }

    fn enumerate_entries(base_dir: PathBuf) -> Vec<String> {
        WalkDir::new(base_dir)
            .into_iter()
            .filter_entry(|e| {
                e.file_type().is_dir()
                    || e.file_name()
                        .to_str()
                        .map(|name| name.ends_with(".gpg"))
                        .unwrap_or(false)
            })
            .filter_map(|e| {
                e.ok()
                    .and_then(|e| e.file_name().to_str().map(|s| s.to_string()))
            })
            .collect()
    }
}
