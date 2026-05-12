use std::{path::PathBuf, process::Command};

use anyhow::Context;
use walkdir::WalkDir;

/// Simple implementation for Linux
/// - list .gpg files in ~/.password-store to find all entries
/// - use `pass ...` command to get passwords
pub struct PassRepository {
    entries: Vec<super::PassEntry>,
    last_used_root: Option<PathBuf>,
}

impl super::PassRepository for PassRepository {
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
                        Some(super::PassEntry::from(
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
