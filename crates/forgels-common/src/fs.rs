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

/// Build dirs skipped at any depth during recursive walks.
///
/// Exact dir names only, no patterns. Covers common build outputs
/// that would otherwise starve the walker budget.
pub const DEFAULT_SKIP_DIRS: &[&str] = &["node_modules", "out", "cache", "artifacts", "coverage"];

/// True for exact skip-dir names. Dot check stays at the call site.
pub fn is_skipped_dir(name: &str) -> bool {
    DEFAULT_SKIP_DIRS.contains(&name)
}

/// Outcome of `list_files_with_extension`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalkOutcome {
    pub files: Vec<PathBuf>,
    pub truncated: bool,
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

/// Names directly inside `dir`: `(name, is_dir)`, sorted by name.
///
/// Skips dot entries, never follows symlinks, stops at `cap`.
/// Missing or unreadable dirs yield an empty list.
pub fn list_dir_entries(dir: &Path, cap: usize) -> Vec<(String, bool)> {
    if cap == 0 {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<(String, bool)> = Vec::new();
    for entry in entries.flatten() {
        if out.len() >= cap {
            break;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.is_empty() || name.starts_with('.') {
            continue;
        }
        out.push((name.to_string(), kind.is_dir()));
    }
    out.sort_by(|left, right| left.0.cmp(&right.0));
    out.truncate(cap);
    out
}

/// Names in `dir` starting with `prefix`: `(name, is_dir)`, sorted.
///
/// Same filters as `list_dir_entries` (skips dots, symlinks, non-utf8)
/// but the prefix applies during collection so filesystem order never
/// hides a match. Sorts, then truncates to `cap`.
pub fn list_dir_entries_matching(dir: &Path, prefix: &str, cap: usize) -> Vec<(String, bool)> {
    if cap == 0 {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<(String, bool)> = Vec::new();
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.is_empty() || name.starts_with('.') {
            continue;
        }
        if !name.starts_with(prefix) {
            continue;
        }
        out.push((name.to_string(), kind.is_dir()));
    }
    out.sort_by(|left, right| left.0.cmp(&right.0));
    out.truncate(cap);
    out
}

/// List files with `extension` under `root`, recursively.
///
/// Skips dot dirs and default build dirs at any depth, never follows
/// symlinks, sorts output. Prunes a dir when a raw exclude equals its
/// walker-relative posix path or that path plus `/**`. Invalid patterns
/// never match and stay silent here. Stops at `caps.max_visited`
/// entries or `caps.max_matches` hits. Missing or unreadable dirs
/// yield an empty list. A root file is always honored.
pub fn list_files_with_extension(
    root: &Path,
    extension: &str,
    caps: WalkCaps,
    excludes: &[String],
) -> WalkOutcome {
    let want = extension.trim_start_matches('.');
    if want.is_empty() {
        return WalkOutcome {
            files: Vec::new(),
            truncated: false,
        };
    }
    if caps.max_visited == 0 || caps.max_matches == 0 {
        return WalkOutcome {
            files: Vec::new(),
            truncated: false,
        };
    }
    let Ok(meta) = std::fs::symlink_metadata(root) else {
        return WalkOutcome {
            files: Vec::new(),
            truncated: false,
        };
    };
    if meta.file_type().is_symlink() {
        return WalkOutcome {
            files: Vec::new(),
            truncated: false,
        };
    }
    if meta.is_file() {
        let hits = root.extension().and_then(|ext| ext.to_str()) == Some(want);
        if hits {
            return WalkOutcome {
                files: vec![root.to_path_buf()],
                truncated: false,
            };
        }
        return WalkOutcome {
            files: Vec::new(),
            truncated: false,
        };
    }
    if !meta.is_dir() {
        return WalkOutcome {
            files: Vec::new(),
            truncated: false,
        };
    }
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    let mut out: Vec<PathBuf> = Vec::new();
    let mut visited: usize = 0;
    let mut truncated = false;
    while let Some(dir) = stack.pop() {
        if visited >= caps.max_visited || out.len() >= caps.max_matches {
            truncated = true;
            break;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if visited >= caps.max_visited || out.len() >= caps.max_matches {
                truncated = true;
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
                let name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default();
                if name.is_empty() || name.starts_with('.') {
                    continue;
                }
                if is_skipped_dir(name) {
                    continue;
                }
                if is_pruned(root, &path, excludes) {
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
    if out.len() > caps.max_matches {
        truncated = true;
        out.truncate(caps.max_matches);
    }
    WalkOutcome {
        files: out,
        truncated,
    }
}

/// True when dir `path` is pruned by a raw exclude.
///
/// `rel` is the walker-relative posix path. A prune happens when some
/// pattern equals `rel` or `rel` plus `/**`. All other patterns,
/// including invalid ones, never match here.
fn is_pruned(root: &Path, path: &Path, excludes: &[String]) -> bool {
    if excludes.is_empty() {
        return false;
    }
    let Ok(rel) = path.strip_prefix(root) else {
        return false;
    };
    if rel.as_os_str().is_empty() {
        return false;
    }
    let rel = rel.to_string_lossy().replace('\\', "/");
    if rel.is_empty() {
        return false;
    }
    let suffixed = format!("{rel}/**");
    for pattern in excludes {
        if pattern == &rel || pattern == &suffixed {
            return true;
        }
    }
    false
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
        let found = list_files_with_extension(&root, "sol", WalkCaps::default(), &[]).files;
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
        let one = list_files_with_extension(&root, "sol", WalkCaps::new(5000, 1), &[]).files;
        assert_eq!(one.len(), 1);
        let missing = root.join("nope");
        assert!(
            list_files_with_extension(&missing, "sol", WalkCaps::default(), &[])
                .files
                .is_empty()
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn dot_extension_and_zero_caps_are_empty() {
        let root = fresh_dir("forgels-fs-edge");
        write(&root, "A.sol");
        assert!(
            list_files_with_extension(&root, "", WalkCaps::default(), &[])
                .files
                .is_empty()
        );
        assert!(
            list_files_with_extension(&root, "sol", WalkCaps::new(0, 10), &[])
                .files
                .is_empty()
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn skips_build_dirs_at_depth_but_keeps_lib() {
        let root = fresh_dir("forgels-fs-skip");
        write(&root, "src/A.sol");
        write(&root, "lib/B.sol");
        for dir in DEFAULT_SKIP_DIRS {
            write(&root, &format!("sub/{dir}/Junk.sol"));
            write(&root, &format!("{dir}/Top.sol"));
        }
        let outcome = list_files_with_extension(&root, "sol", WalkCaps::default(), &[]);
        let mut names: Vec<String> = outcome
            .files
            .iter()
            .map(|path| {
                path.strip_prefix(&root)
                    .expect("under root")
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        names.sort();
        assert_eq!(names, vec!["lib/B.sol", "src/A.sol"]);
        assert!(!outcome.truncated);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn prunes_exact_and_star_star_excludes() {
        let root = fresh_dir("forgels-fs-prune");
        write(&root, "src/A.sol");
        write(&root, "lib/B.sol");
        let exact =
            list_files_with_extension(&root, "sol", WalkCaps::default(), &[String::from("lib")]);
        assert_eq!(exact.files.len(), 1);
        let star =
            list_files_with_extension(&root, "sol", WalkCaps::default(), &[String::from("lib/**")]);
        assert_eq!(star.files.len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn invalid_exclude_pattern_never_prunes() {
        let root = fresh_dir("forgels-fs-invalid");
        write(&root, "src/A.sol");
        write(&root, "lib/B.sol");
        let outcome = list_files_with_extension(
            &root,
            "sol",
            WalkCaps::default(),
            &[String::from("[invalid")],
        );
        assert_eq!(outcome.files.len(), 2);
        assert!(!outcome.truncated);
        let glob = list_files_with_extension(
            &root,
            "sol",
            WalkCaps::default(),
            &[String::from("**/*.sol")],
        );
        assert_eq!(glob.files.len(), 2);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn truncated_flag_tracks_caps() {
        let root = fresh_dir("forgels-fs-truncated");
        write(&root, "A.sol");
        write(&root, "B.sol");
        let small = list_files_with_extension(&root, "sol", WalkCaps::new(5000, 1), &[]);
        assert!(small.truncated);
        assert_eq!(small.files.len(), 1);
        let normal = list_files_with_extension(&root, "sol", WalkCaps::default(), &[]);
        assert!(!normal.truncated);
        assert_eq!(normal.files.len(), 2);
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

    #[test]
    fn entries_list_sorted_skip_dots_symlinks_and_cap() {
        let root = fresh_dir("forgels-fs-entries");
        write(&root, "B.sol");
        write(&root, "A.sol");
        write(&root, ".hidden");
        std::fs::create_dir_all(root.join("pkg")).expect("dir");
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.join("A.sol"), root.join("Link.sol")).expect("symlink");
        let found = list_dir_entries(&root, 128);
        assert_eq!(
            found,
            vec![
                ("A.sol".to_string(), false),
                ("B.sol".to_string(), false),
                ("pkg".to_string(), true),
            ]
        );
        assert_eq!(list_dir_entries(&root, 2).len(), 2);
        assert!(list_dir_entries(&root, 0).is_empty());
        assert!(list_dir_entries(&root.join("nope"), 128).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn entries_matching_filters_before_cap() {
        let root = fresh_dir("forgels-fs-matching");
        for index in 0..200 {
            std::fs::create_dir_all(root.join(format!("pkg{index:03}"))).expect("dir");
        }
        std::fs::create_dir_all(root.join("@openzeppelin")).expect("dir");
        let found = list_dir_entries_matching(&root, "@open", 128);
        assert_eq!(found, vec![("@openzeppelin".to_string(), true)]);
        assert!(list_dir_entries_matching(&root, "@open", 0).is_empty());
        assert!(list_dir_entries_matching(&root.join("nope"), "@open", 128).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }
}
