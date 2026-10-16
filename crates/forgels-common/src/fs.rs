use std::path::Path;

pub fn read_to_string(path: &Path) -> std::io::Result<String> {
    std::fs::read_to_string(path)
}

pub fn write_string(path: &Path, content: &str) -> std::io::Result<()> {
    std::fs::write(path, content)
}

/// True when `path` is a regular file on disk.
pub fn exists(path: &Path) -> bool {
    path.is_file()
}
