use std::path::PathBuf;

#[derive(Debug, Default, Clone)]
pub struct Config {
    pub root: Option<PathBuf>,
}

impl Config {
    pub fn new() -> Self {
        Self::default()
    }
}
