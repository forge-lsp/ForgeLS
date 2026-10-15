use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Default)]
pub struct Workspace {
    files: HashMap<PathBuf, String>,
}

impl Workspace {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn upsert(&mut self, path: PathBuf, text: String) {
        self.files.insert(path, text);
    }

    pub fn get(&self, path: &PathBuf) -> Option<&String> {
        self.files.get(path)
    }
}
