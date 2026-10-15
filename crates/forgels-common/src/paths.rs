use std::path::{Path, PathBuf};

pub fn normalize(path: &Path) -> PathBuf {
    path.to_path_buf()
}

pub fn resolve_root(cwd: &Path, root: Option<&Path>) -> PathBuf {
    root.map(|p| p.to_path_buf())
        .unwrap_or_else(|| cwd.to_path_buf())
}
