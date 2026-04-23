use std::{env, path::PathBuf, process::Command};

use anyhow::Context;
use walkdir::WalkDir;

/// Simple implementation for Linux
/// - list .gpg files in ~/.password-store to find all entries
/// - use `pass ...` command to get passwords
pub struct PassRepository {
    entries: Vec<super::PassEntry>,
}

impl PassRepository {
    fn enumerate_entries(base_dir: PathBuf) -> Vec<super::PassEntry> {
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
                        Some(super::PassEntry::from(
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

impl super::PassRepository for PassRepository {
    /// keep it return result for the time-being
    #[allow(clippy::new_ret_no_self)]
    fn new() -> anyhow::Result<Self> {
        let pass_home = env::home_dir()
            .context("cannot get home folder")?
            .join(".password-store");
        Ok(Self {
            entries: Self::enumerate_entries(pass_home),
        })
    }

    fn get_by_pattern(&self, pattern: &str) -> Vec<super::PassEntry> {
        self.entries
            .iter()
            .filter(|e| e.contains(pattern))
            .cloned()
            .collect()
    }

    fn entries_count(&self) -> usize {
        self.entries.len()
    }

    fn retrieve(&self, entry: &super::PassEntry) -> anyhow::Result<(String, String)> {
        let output = Command::new("pass")
            .arg(entry.to_string())
            .output()
            .context("cannot retrieve entry from pass")?;
        let password =
            str::from_utf8(&output.stdout).context("unable to interpret output as UTF-8 string")?;
        Ok((entry.username(), password.trim().to_string()))
    }
}
