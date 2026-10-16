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

/// Prefix remapping in Foundry style, for example `@oz/=lib/oz/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remapping {
    pub prefix: String,
    pub target: PathBuf,
}

impl Remapping {
    pub fn new(prefix: String, target: PathBuf) -> Self {
        Self { prefix, target }
    }
}

/// Parse one `prefix=target` entry. Empty sides mean `None`.
pub fn parse_remapping(raw: &str) -> Option<Remapping> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (prefix, target) = trimmed.split_once('=')?;
    let prefix = prefix.trim();
    let target = target.trim();
    if prefix.is_empty() || target.is_empty() {
        return None;
    }
    Some(Remapping::new(prefix.to_string(), PathBuf::from(target)))
}

/// Resolve an import with remappings on top of relative resolution.
///
/// Relative imports win first. Otherwise the longest matching prefix
/// replaces its head with the mapped target joined to `root`.
/// Join is lexical only, no disk access.
pub fn resolve_with_remappings(
    from_file: &Path,
    raw: &str,
    root: &Path,
    remappings: &[Remapping],
) -> Option<PathBuf> {
    if let Some(path) = resolve_import(from_file, raw) {
        return Some(path);
    }
    if raw.is_empty() {
        return None;
    }
    let mut best: Option<&Remapping> = None;
    for item in remappings {
        if raw.starts_with(item.prefix.as_str()) {
            let longer = best.is_some_and(|prev| prev.prefix.len() >= item.prefix.len());
            if !longer {
                best = Some(item);
            }
        }
    }
    let item = best?;
    let rest = raw[item.prefix.len()..].trim_start_matches('/');
    let base = if item.target.is_absolute() {
        item.target.clone()
    } else {
        root.join(&item.target)
    };
    let joined = if rest.is_empty() {
        base
    } else {
        base.join(rest)
    };
    Some(normalize_lexically(&joined))
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

    #[test]
    fn remapping_joins_target_to_root() {
        let maps = vec![parse_remapping("@oz/=lib/oz/").expect("valid")];
        assert_eq!(
            resolve_with_remappings(
                Path::new("/repo/contracts/A.sol"),
                "@oz/ERC20.sol",
                Path::new("/repo"),
                &maps
            ),
            Some(PathBuf::from("/repo/lib/oz/ERC20.sol"))
        );
    }

    #[test]
    fn relative_wins_over_remapping() {
        let maps = vec![parse_remapping("./=lib/").expect("valid")];
        assert_eq!(
            resolve_with_remappings(
                Path::new("/repo/contracts/A.sol"),
                "./B.sol",
                Path::new("/repo"),
                &maps
            ),
            Some(PathBuf::from("/repo/contracts/B.sol"))
        );
    }

    #[test]
    fn longest_prefix_wins() {
        let maps = vec![
            parse_remapping("@oz/=lib/oz/").expect("valid"),
            parse_remapping("@oz/extra/=lib/extra/").expect("valid"),
        ];
        assert_eq!(
            resolve_with_remappings(
                Path::new("/repo/A.sol"),
                "@oz/extra/M.sol",
                Path::new("/repo"),
                &maps
            ),
            Some(PathBuf::from("/repo/lib/extra/M.sol"))
        );
    }

    #[test]
    fn bad_remapping_lines_return_none() {
        assert!(parse_remapping("").is_none());
        assert!(parse_remapping("no-equals").is_none());
        assert!(parse_remapping("=lib/").is_none());
        assert!(parse_remapping("@oz/=").is_none());
    }
}
