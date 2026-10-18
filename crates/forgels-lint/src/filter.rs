use glob::Pattern;

// Decide whether a cwd-relative posix path is linted.
// Empty includes match all, excludes remove. Invalid patterns warn
// and are ignored so one typo never drops the whole run.
// `**/*.sol` matches root and nested files: `a.sol` plus `sub/b.sol`.
pub fn should_lint(rel: &str, includes: &[String], excludes: &[String]) -> (bool, Vec<String>) {
    let mut warnings = Vec::new();
    if !includes.is_empty() {
        let mut matched = false;
        for raw in includes {
            match Pattern::new(raw) {
                Ok(pattern) => {
                    if pattern_matches(&pattern, raw, rel) {
                        matched = true;
                    }
                }
                Err(err) => warnings.push(format!("fls: invalid include pattern \"{raw}\": {err}")),
            }
        }
        if !matched {
            return (false, warnings);
        }
    }
    for raw in excludes {
        match Pattern::new(raw) {
            Ok(pattern) => {
                if pattern_matches(&pattern, raw, rel) {
                    return (false, warnings);
                }
            }
            Err(err) => warnings.push(format!("fls: invalid exclude pattern \"{raw}\": {err}")),
        }
    }
    (true, warnings)
}

fn pattern_matches(pattern: &Pattern, raw: &str, rel: &str) -> bool {
    if pattern.matches(rel) {
        return true;
    }
    // `**/` means zero or more dirs, so `**/*.sol` also matches `a.sol`.
    if let Some(rest) = raw.strip_prefix("**/")
        && let Ok(fallback) = Pattern::new(rest)
    {
        return fallback.matches(rel);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_includes_match_all() {
        let (keep, warnings) = should_lint("a.sol", &[], &[]);
        assert!(keep);
        assert!(warnings.is_empty());
    }

    #[test]
    fn doublestar_matches_root_and_nested() {
        let includes = vec!["**/*.sol".to_string()];
        let (root, _) = should_lint("a.sol", &includes, &[]);
        let (nested, _) = should_lint("sub/b.sol", &includes, &[]);
        let (other, _) = should_lint("sub/b.txt", &includes, &[]);
        assert!(root, "root file matches **/*.sol");
        assert!(nested, "nested file matches **/*.sol");
        assert!(!other, "non-sol stays out");
    }

    #[test]
    fn excludes_remove_after_include() {
        let includes = vec!["**/*.sol".to_string()];
        let excludes = vec!["lib/**".to_string()];
        let (kept, _) = should_lint("src/a.sol", &includes, &excludes);
        let (dropped, _) = should_lint("lib/a.sol", &includes, &excludes);
        assert!(kept);
        assert!(!dropped);
    }

    #[test]
    fn invalid_pattern_warns_and_ignores() {
        let includes = vec!["[bad".to_string(), "**/*.sol".to_string()];
        let (keep, warnings) = should_lint("a.sol", &includes, &[]);
        assert!(keep);
        assert_eq!(warnings.len(), 1);
        let (dropped, warnings) = should_lint("a.sol", &["[bad".to_string()], &[]);
        assert!(!dropped);
        assert_eq!(warnings.len(), 1);
        let (kept, warnings) = should_lint("a.sol", &[], &["[bad".to_string()]);
        assert!(kept);
        assert_eq!(warnings.len(), 1);
    }
}
