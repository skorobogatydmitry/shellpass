use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::notifications::{self, Message};

use super::PassEntry as _PassEntry;
use anyhow::Context;
use walkdir::WalkDir;

impl super::RepositoryAccessor<PassEntry> for super::PassRepository<PassEntry> {
    fn fetch_entries_for(&mut self, pass_root: &str) {
        let pass_root_pb = PathBuf::from(pass_root);
        self.entries = WalkDir::new(pass_root)
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
                        Some(PassEntry::from((
                            &pass_root_pb,
                            e.into_path().strip_prefix(&pass_root_pb).unwrap(),
                        )))
                    } else {
                        None
                    }
                })
            })
            .collect();
        if self.entries.is_empty() {
            // TODO: excavate & summarize all the errors
            notifications::push_message(Message::new(
                format!("no entries found for {pass_root}"),
                notifications::Kind::Warning,
            ));
        } else {
            self.last_used_root = Some(pass_root.to_string());
        }
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
}

#[derive(Clone)]
pub struct PassEntry {
    pass_root: PathBuf,
    relpath: PathBuf,
}

impl super::PassEntry for PassEntry {
    fn entry_relpath(&self) -> &PathBuf {
        &self.relpath
    }

    /// as simple as reading the file with its path
    fn read(&self) -> anyhow::Result<Vec<u8>> {
        fs::read(self.pass_root.join(self.relpath.clone()))
            .context("unable to read encrypted pass entry")
    }
}

/// contruct the entry from root (PathBuf) and full path (Path)
/// both are needed to know full path (to Self::read) and its suffix (to impl Display)
impl From<(&PathBuf, &Path)> for PassEntry {
    fn from(value: (&PathBuf, &Path)) -> Self {
        Self {
            pass_root: value.0.clone(),
            relpath: value.1.to_path_buf(),
        }
    }
}
