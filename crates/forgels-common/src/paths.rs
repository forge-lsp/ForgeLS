use std::path::{Component, Path, PathBuf};

pub fn normalize(path: &Path) -> PathBuf {
    path.to_path_buf()
}

pub fn resolve_root(cwd: &Path, root: Option<&Path>) -> PathBuf {
    root.map(|p| p.to_path_buf())
        .unwrap_or_else(|| cwd.to_path_buf())
}

/// Resolve a relative import to a path next to the importing file.
///
/// Only `./` and `../` count for V1. Bare specifiers, absolute paths,
/// and empty strings return `None`. Join is lexical only, no disk access.
pub fn resolve_import(from_file: &Path, raw: &str) -> Option<PathBuf> {
    if raw.is_empty() {
        return None;
    }
    let relative = raw == "." || raw == ".." || raw.starts_with("./") || raw.starts_with("../");
    if !relative {
        return None;
    }
    let base = from_file.parent().unwrap_or(Path::new(""));
    Some(normalize_lexically(&base.join(raw)))
}

fn normalize_lexically(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::Prefix(prefix) => out.push(prefix.as_os_str()),
            Component::RootDir => out.push(Component::RootDir.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                let can_pop = matches!(out.components().next_back(), Some(Component::Normal(_)));
                if can_pop {
                    out.pop();
                } else if !out.has_root() {
                    out.push("..");
                }
            }
            Component::Normal(name) => out.push(name),
        }
    }
    if out.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dot_slash_joins_next_to_file() {
        assert_eq!(
            resolve_import(Path::new("/repo/contracts/A.sol"), "./B.sol"),
            Some(PathBuf::from("/repo/contracts/B.sol"))
        );
    }

    #[test]
    fn dot_dot_climbs_one_dir() {
        assert_eq!(
            resolve_import(Path::new("/repo/contracts/A.sol"), "../shared/B.sol"),
            Some(PathBuf::from("/repo/shared/B.sol"))
        );
    }

    #[test]
    fn bare_and_absolute_return_none() {
        assert!(resolve_import(Path::new("/repo/A.sol"), "openzeppelin/ERC20.sol").is_none());
        assert!(resolve_import(Path::new("/repo/A.sol"), "/abs/B.sol").is_none());
        assert!(resolve_import(Path::new("/repo/A.sol"), "").is_none());
    }
}
