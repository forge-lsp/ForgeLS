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

/// Lexical path of `target` seen from `from_file`.
///
/// Always `./` or `../` prefixed, with `/` separators. Both sides must be
/// absolute or both relative, otherwise `None`. No disk access.
pub fn relative_path(from_file: &Path, target: &Path) -> Option<String> {
    let base = from_file.parent().unwrap_or(Path::new(""));
    let base_norm = normalize_lexically(base);
    let target_norm = normalize_lexically(target);
    if base_norm.has_root() != target_norm.has_root() {
        return None;
    }
    let base_drive = base_norm.components().find_map(|part| match part {
        Component::Prefix(prefix) => Some(prefix.as_os_str().to_owned()),
        _ => None,
    });
    let target_drive = target_norm.components().find_map(|part| match part {
        Component::Prefix(prefix) => Some(prefix.as_os_str().to_owned()),
        _ => None,
    });
    if base_drive != target_drive {
        return None;
    }
    let base_parts: Vec<Component> = base_norm.components().collect();
    let target_parts: Vec<Component> = target_norm.components().collect();
    let mut common = 0;
    while common < base_parts.len()
        && common < target_parts.len()
        && base_parts[common] == target_parts[common]
    {
        common += 1;
    }
    let mut rel: Vec<String> = Vec::new();
    for part in &base_parts[common..] {
        match part {
            Component::Normal(_) | Component::ParentDir => rel.push("..".to_string()),
            _ => {}
        }
    }
    for part in &target_parts[common..] {
        match part {
            Component::Normal(name) => rel.push(name.to_string_lossy().into_owned()),
            Component::ParentDir => rel.push("..".to_string()),
            _ => {}
        }
    }
    if rel.is_empty() {
        return Some("./".to_string());
    }
    let joined = rel.join("/");
    if joined.starts_with("..") {
        Some(joined)
    } else {
        Some(format!("./{joined}"))
    }
}

/// Max ancestor dirs to probe for `node_modules`. Bounds disk checks.
pub const MAX_NODE_MODULES_DEPTH: usize = 8;

/// Candidate `node_modules` paths for a bare specifier, nearest first.
///
/// Lexical only, no disk access. Relative, absolute, and empty
/// specifiers yield no candidates. Stops after `MAX_NODE_MODULES_DEPTH`
/// ancestors.
pub fn node_module_candidates(from_file: &Path, raw: &str) -> Vec<PathBuf> {
    if raw.is_empty() {
        return Vec::new();
    }
    if raw == "." || raw == ".." || raw.starts_with("./") || raw.starts_with("../") {
        return Vec::new();
    }
    if Path::new(raw).is_absolute() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut dir = from_file.parent().map(|parent| parent.to_path_buf());
    for _ in 0..MAX_NODE_MODULES_DEPTH {
        let Some(current) = dir else {
            break;
        };
        out.push(normalize_lexically(&current.join("node_modules").join(raw)));
        dir = current.parent().map(|parent| parent.to_path_buf());
    }
    out
}

/// Resolve with `node_modules` fallback: relative, then remappings,
/// then the first existing `node_modules` candidate.
///
/// Old callers keep `resolve_import` and `resolve_with_remappings`
/// unchanged. Only new resolution flows use this entry point.
pub fn resolve_import_full(
    from_file: &Path,
    raw: &str,
    root: &Path,
    remappings: &[Remapping],
    exists: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    if let Some(path) = resolve_with_remappings(from_file, raw, root, remappings) {
        return Some(path);
    }
    node_module_candidates(from_file, raw)
        .into_iter()
        .find(|candidate| exists(candidate))
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

    #[test]
    fn relative_same_dir_gets_dot_prefix() {
        assert_eq!(
            relative_path(
                Path::new("/repo/contracts/A.sol"),
                Path::new("/repo/contracts/B.sol")
            ),
            Some("./B.sol".to_string())
        );
    }

    #[test]
    fn relative_subdir_stays_dot_prefixed() {
        assert_eq!(
            relative_path(
                Path::new("/repo/contracts/A.sol"),
                Path::new("/repo/contracts/sub/C.sol")
            ),
            Some("./sub/C.sol".to_string())
        );
    }

    #[test]
    fn relative_parent_climbs_with_dots() {
        assert_eq!(
            relative_path(
                Path::new("/repo/contracts/A.sol"),
                Path::new("/repo/shared/B.sol")
            ),
            Some("../shared/B.sol".to_string())
        );
    }

    #[test]
    fn relative_mixed_roots_return_none() {
        assert!(relative_path(Path::new("/repo/A.sol"), Path::new("contracts/B.sol")).is_none());
        assert!(relative_path(Path::new("contracts/A.sol"), Path::new("/repo/B.sol")).is_none());
    }

    #[test]
    fn node_candidates_nearest_first() {
        let found = node_module_candidates(Path::new("/repo/contracts/A.sol"), "@oz/ERC20.sol");
        assert!(found.len() >= 2);
        assert_eq!(
            found[0],
            PathBuf::from("/repo/contracts/node_modules/@oz/ERC20.sol")
        );
        assert_eq!(found[1], PathBuf::from("/repo/node_modules/@oz/ERC20.sol"));
    }

    #[test]
    fn node_candidates_cap_at_depth() {
        let found = node_module_candidates(Path::new("/a/b/c/d/e/f/g/h/i/A.sol"), "pkg/M.sol");
        assert_eq!(found.len(), MAX_NODE_MODULES_DEPTH);
    }

    #[test]
    fn node_candidates_reject_non_bare() {
        assert!(node_module_candidates(Path::new("/repo/A.sol"), "").is_empty());
        assert!(node_module_candidates(Path::new("/repo/A.sol"), "./B.sol").is_empty());
        assert!(node_module_candidates(Path::new("/repo/A.sol"), "../B.sol").is_empty());
        assert!(node_module_candidates(Path::new("/repo/A.sol"), "/abs/B.sol").is_empty());
    }

    #[test]
    fn full_prefers_relative_then_remapping_then_node() {
        let root = Path::new("/repo");
        let maps = vec![parse_remapping("@oz/=lib/oz/").expect("valid")];
        let never = |_: &Path| false;
        assert_eq!(
            resolve_import_full(
                Path::new("/repo/contracts/A.sol"),
                "./B.sol",
                root,
                &maps,
                never
            ),
            Some(PathBuf::from("/repo/contracts/B.sol"))
        );
        assert_eq!(
            resolve_import_full(Path::new("/repo/A.sol"), "@oz/M.sol", root, &maps, never),
            Some(PathBuf::from("/repo/lib/oz/M.sol"))
        );
        let only_node = |path: &Path| path == Path::new("/repo/node_modules/pkg/M.sol");
        assert_eq!(
            resolve_import_full(
                Path::new("/repo/contracts/A.sol"),
                "pkg/M.sol",
                root,
                &[],
                only_node
            ),
            Some(PathBuf::from("/repo/node_modules/pkg/M.sol"))
        );
    }

    #[test]
    fn full_missing_returns_none() {
        let root = Path::new("/repo");
        let never = |_: &Path| false;
        assert!(
            resolve_import_full(Path::new("/repo/A.sol"), "pkg/M.sol", root, &[], never).is_none()
        );
        assert!(resolve_import_full(Path::new("/repo/A.sol"), "", root, &[], never).is_none());
    }
}
