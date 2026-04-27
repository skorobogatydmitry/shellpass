pub(crate) struct PassRepository {}

impl super::PassRepository for PassRepository {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self {}
    }

    fn entries_count(&self) -> usize {
        10
    }

    fn get_by_pattern(&self, _pattern: &str) -> Vec<super::PassEntry> {
        Vec::new()
    }

    fn retrieve(&self, _entry: &super::PassEntry) -> anyhow::Result<(String, String)> {
        Ok(("dummy username".to_string(), "dummy password".to_string()))
    }

    fn refresh_entries(&mut self, pass_root: Option<std::path::PathBuf>) {
        // todo!()
    }
}
