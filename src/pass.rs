use std::path::Path;

#[cfg(target_os = "android")]
pub(crate) mod android;
#[cfg(target_os = "linux")]
pub(crate) mod linux;

/// Required interface for pass repository
/// TODO: why Arc<RwLock<...>> require this ?
pub(crate) trait PassRepository: Send + Sync {
    /// create new repository for the UI to access
    fn new() -> anyhow::Result<Self>
    where
        Self: Sized;
    /// get all entries matching a given pattern
    fn get_by_pattern(&self, pattern: &str) -> Vec<PassEntry>;
    /// number of entries in the pass
    fn entries_count(&self) -> usize;
    /// get (username, password) of the given entry
    fn retrieve(&self, entry: &PassEntry) -> anyhow::Result<(String, String)>;
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

#[allow(clippy::to_string_trait_impl)]
impl ToString for PassEntry {
    fn to_string(&self) -> String {
        self.path_components.join("/")
    }
}
