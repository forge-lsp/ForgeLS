use std::path::{Path, PathBuf};

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

/// True when `path` is a trackable regular file.
///
/// Uses `symlink_metadata` so symlinks never count, even when they
/// point at real files. This keeps V1 pnpm layouts out on purpose.
pub fn is_trackable_file(path: &Path) -> bool {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return false;
    };
    if meta.file_type().is_symlink() {
        return false;
    }
    meta.is_file()
}

/// Bounds for `list_files_with_extension`. Both caps stop the walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalkCaps {
    pub max_visited: usize,
    pub max_matches: usize,
}

impl Default for WalkCaps {
    fn default() -> Self {
        Self {
            max_visited: 5000,
            max_matches: 200,
        }
    }
}

impl WalkCaps {
    pub fn new(max_visited: usize, max_matches: usize) -> Self {
        Self {
            max_visited,
            max_matches,
        }
    }
}

/// List files with `extension` under `root`, recursively.
///
/// Skips dot dirs, never follows symlinks, sorts output. Stops at
/// `caps.max_visited` entries or `caps.max_matches` hits. Missing or
/// unreadable dirs yield an empty list.
pub fn list_files_with_extension(root: &Path, extension: &str, caps: WalkCaps) -> Vec<PathBuf> {
    let want = extension.trim_start_matches('.');
    if want.is_empty() {
        return Vec::new();
    }
    if caps.max_visited == 0 || caps.max_matches == 0 {
        return Vec::new();
    }
    let Ok(meta) = std::fs::symlink_metadata(root) else {
        return Vec::new();
    };
    if meta.file_type().is_symlink() {
        return Vec::new();
    }
    if meta.is_file() {
        let hits = root.extension().and_then(|ext| ext.to_str()) == Some(want);
        if hits {
            return vec![root.to_path_buf()];
        }
        return Vec::new();
    }
    if !meta.is_dir() {
        return Vec::new();
    }
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    let mut out: Vec<PathBuf> = Vec::new();
    let mut visited: usize = 0;
    while let Some(dir) = stack.pop() {
        if visited >= caps.max_visited || out.len() >= caps.max_matches {
            break;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if visited >= caps.max_visited || out.len() >= caps.max_matches {
                break;
            }
            visited += 1;
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            let path = entry.path();
            if kind.is_dir() {
                let hidden = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with('.'));
                if hidden {
                    continue;
                }
                stack.push(path);
            } else if kind.is_file() {
                let hits = path.extension().and_then(|ext| ext.to_str()) == Some(want);
                if hits {
                    out.push(path);
                }
            }
        }
    }
    out.sort();
    out.truncate(caps.max_matches);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_dir(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("temp root");
        root
    }

    fn write(root: &Path, rel: &str) {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("parent");
        }
        std::fs::write(&path, "contract A {}").expect("write");
    }

    #[test]
    fn lists_sol_sorted_and_skips_dot_dirs() {
        let root = fresh_dir("forgels-fs-list");
        write(&root, "B.sol");
        write(&root, "sub/C.sol");
        write(&root, "A.sol");
        write(&root, "skip.txt");
        write(&root, ".hidden/D.sol");
        let found = list_files_with_extension(&root, "sol", WalkCaps::default());
        let names: Vec<String> = found
            .iter()
            .map(|path| {
                path.strip_prefix(&root)
                    .expect("under root")
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(names, vec!["A.sol", "B.sol", "sub/C.sol"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn caps_matches_and_missing_root_is_empty() {
        let root = fresh_dir("forgels-fs-caps");
        write(&root, "A.sol");
        write(&root, "B.sol");
        let one = list_files_with_extension(&root, "sol", WalkCaps::new(5000, 1));
        assert_eq!(one.len(), 1);
        let missing = root.join("nope");
        assert!(list_files_with_extension(&missing, "sol", WalkCaps::default()).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn dot_extension_and_zero_caps_are_empty() {
        let root = fresh_dir("forgels-fs-edge");
        write(&root, "A.sol");
        assert!(list_files_with_extension(&root, "", WalkCaps::default()).is_empty());
        assert!(list_files_with_extension(&root, "sol", WalkCaps::new(0, 10)).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn trackable_accepts_file_rejects_missing_and_dir() {
        let root = fresh_dir("forgels-fs-trackable");
        write(&root, "A.sol");
        assert!(is_trackable_file(&root.join("A.sol")));
        assert!(!is_trackable_file(&root.join("Missing.sol")));
        assert!(!is_trackable_file(&root));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn trackable_rejects_symlink() {
        let root = fresh_dir("forgels-fs-symlink");
        write(&root, "Real.sol");
        let link = root.join("Link.sol");
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.join("Real.sol"), &link).expect("symlink");
        #[cfg(unix)]
        assert!(!is_trackable_file(&link));
        #[cfg(unix)]
        assert!(is_trackable_file(&root.join("Real.sol")));
        let _ = std::fs::remove_dir_all(&root);
    }
}
