use std::path::{Path, PathBuf};

use forgels_common::fs::{WalkCaps, list_files_with_extension};

// Large caps for CLI scans: bigger than the 5000/200 server
// defaults because a repo-wide lint run must see every file.
const WALK_CAPS: WalkCaps = WalkCaps {
    max_visited: 20000,
    max_matches: 5000,
};

fn is_sol_file(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()) == Some("sol")
}

fn push_explicit(path: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return Err(format!("fls: cannot read {}", path.display()));
    };
    if meta.file_type().is_symlink() {
        return Err(format!("fls: cannot read {}", path.display()));
    }
    if meta.is_file() {
        if !is_sol_file(path) {
            return Ok(());
        }
        if std::fs::read_to_string(path).is_err() {
            return Err(format!("fls: cannot read {}", path.display()));
        }
        out.push(path.to_path_buf());
        return Ok(());
    }
    if meta.is_dir() {
        let mut found = list_files_with_extension(path, "sol", WALK_CAPS);
        // Probe readability: unreadable subtrees stay silent in the
        // walker, so a failed root read is the only hard error here.
        if found.is_empty() && std::fs::read_dir(path).is_err() {
            return Err(format!("fls: cannot read {}", path.display()));
        }
        out.append(&mut found);
        return Ok(());
    }
    Err(format!("fls: cannot read {}", path.display()))
}

/// Collect *.sol files for check.
///
/// Empty input scans cwd recursively. Explicit paths accept files or
/// dirs. Output is sorted and deduped. Any unreadable explicit path
/// is an operational failure for the caller to report as exit 2.
pub fn collect_files(paths: &[PathBuf], cwd: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out: Vec<PathBuf> = Vec::new();
    if paths.is_empty() {
        out = list_files_with_extension(cwd, "sol", WALK_CAPS);
    } else {
        for path in paths {
            push_explicit(path, &mut out)?;
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}
