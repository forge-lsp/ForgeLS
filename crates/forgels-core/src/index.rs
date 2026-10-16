use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// One tracked file: ordered deduped deps as resolved paths.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileEntry {
    pub deps: Vec<PathBuf>,
}

/// Language agnostic dependency graph across open files.
///
/// `files` holds forward edges, `dependents` holds reverse edges so
/// change propagation finds affected files without scanning.
#[derive(Debug, Default, Clone)]
pub struct ProjectIndex {
    files: HashMap<PathBuf, FileEntry>,
    dependents: HashMap<PathBuf, Vec<PathBuf>>,
}

impl ProjectIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace the dep list for `path`, fixing reverse edges.
    pub fn rebuild_file(&mut self, path: PathBuf, deps: Vec<PathBuf>) {
        let mut seen = HashSet::new();
        let mut clean: Vec<PathBuf> = Vec::new();
        for dep in deps {
            if seen.insert(dep.clone()) {
                clean.push(dep);
            }
        }
        let old = self
            .files
            .get(&path)
            .map(|entry| entry.deps.clone())
            .unwrap_or_default();
        for dep in &old {
            if !clean.contains(dep) {
                if let Some(list) = self.dependents.get_mut(dep) {
                    list.retain(|item| item != &path);
                }
                if self.dependents.get(dep).is_some_and(|list| list.is_empty()) {
                    self.dependents.remove(dep);
                }
            }
        }
        for dep in &clean {
            if !old.contains(dep) {
                self.dependents
                    .entry(dep.clone())
                    .or_default()
                    .push(path.clone());
            }
        }
        self.files.insert(path, FileEntry { deps: clean });
    }

    /// Drop `path` from the graph. Dependents keep their dep entry so a
    /// removed file still reports who depended on it as missing.
    pub fn remove(&mut self, path: &Path) {
        let Some(entry) = self.files.remove(path) else {
            return;
        };
        for dep in &entry.deps {
            if let Some(list) = self.dependents.get_mut(dep) {
                list.retain(|item| item != path);
            }
            if self.dependents.get(dep).is_some_and(|list| list.is_empty()) {
                self.dependents.remove(dep);
            }
        }
    }

    /// Direct deps of `path`. Empty when unknown.
    pub fn deps_of(&self, path: &Path) -> &[PathBuf] {
        self.files
            .get(path)
            .map(|entry| entry.deps.as_slice())
            .unwrap_or(&[])
    }

    /// Direct dependents of `path`. Includes open files even when `path`
    /// itself is missing or removed.
    pub fn dependents_of(&self, path: &Path) -> &[PathBuf] {
        self.dependents.get(path).map(Vec::as_slice).unwrap_or(&[])
    }

    /// All files reachable from `path`, excluding `path` itself.
    /// Cycle safe via a visited set.
    pub fn transitive_closure(&self, path: &Path) -> Vec<PathBuf> {
        let mut visited: HashSet<PathBuf> = HashSet::new();
        visited.insert(path.to_path_buf());
        let mut out = Vec::new();
        let mut stack: Vec<PathBuf> = Vec::new();
        if let Some(entry) = self.files.get(path) {
            for dep in entry.deps.iter().rev() {
                stack.push(dep.clone());
            }
        }
        while let Some(current) = stack.pop() {
            if !visited.insert(current.clone()) {
                continue;
            }
            out.push(current.clone());
            if let Some(entry) = self.files.get(&current) {
                for dep in entry.deps.iter().rev() {
                    if !visited.contains(dep) {
                        stack.push(dep.clone());
                    }
                }
            }
        }
        out
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.files.contains_key(path)
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(name: &str) -> PathBuf {
        PathBuf::from(name)
    }

    #[test]
    fn linear_chain_reports_closure_in_order() {
        let mut index = ProjectIndex::new();
        index.rebuild_file(path("/a.sol"), vec![path("/b.sol")]);
        index.rebuild_file(path("/b.sol"), Vec::new());
        assert_eq!(index.deps_of(&path("/a.sol")), &[path("/b.sol")]);
        assert_eq!(index.dependents_of(&path("/b.sol")), &[path("/a.sol")]);
        assert_eq!(
            index.transitive_closure(&path("/a.sol")),
            vec![path("/b.sol")]
        );
    }

    #[test]
    fn diamond_merges_without_duplicates() {
        let mut index = ProjectIndex::new();
        index.rebuild_file(path("/a.sol"), vec![path("/b.sol"), path("/c.sol")]);
        index.rebuild_file(path("/b.sol"), vec![path("/d.sol")]);
        index.rebuild_file(path("/c.sol"), vec![path("/d.sol")]);
        index.rebuild_file(path("/d.sol"), Vec::new());
        let closure = index.transitive_closure(&path("/a.sol"));
        assert_eq!(closure.len(), 3);
        assert!(closure.contains(&path("/b.sol")));
        assert!(closure.contains(&path("/c.sol")));
        assert!(closure.contains(&path("/d.sol")));
        assert_eq!(index.dependents_of(&path("/d.sol")).len(), 2);
    }

    #[test]
    fn cycle_terminates_and_excludes_start() {
        let mut index = ProjectIndex::new();
        index.rebuild_file(path("/a.sol"), vec![path("/b.sol")]);
        index.rebuild_file(path("/b.sol"), vec![path("/a.sol")]);
        let closure = index.transitive_closure(&path("/a.sol"));
        assert_eq!(closure, vec![path("/b.sol")]);
        assert_eq!(index.deps_of(&path("/b.sol")), &[path("/a.sol")]);
    }

    #[test]
    fn missing_dep_stays_queryable() {
        let mut index = ProjectIndex::new();
        index.rebuild_file(path("/a.sol"), vec![path("/gone.sol")]);
        assert_eq!(index.deps_of(&path("/a.sol")), &[path("/gone.sol")]);
        assert_eq!(index.dependents_of(&path("/gone.sol")), &[path("/a.sol")]);
        assert_eq!(
            index.transitive_closure(&path("/a.sol")),
            vec![path("/gone.sol")]
        );
    }

    #[test]
    fn rebuild_on_change_updates_both_directions() {
        let mut index = ProjectIndex::new();
        index.rebuild_file(path("/a.sol"), vec![path("/b.sol")]);
        index.rebuild_file(path("/b.sol"), Vec::new());
        index.rebuild_file(path("/c.sol"), Vec::new());
        index.rebuild_file(path("/a.sol"), vec![path("/c.sol")]);
        assert_eq!(index.deps_of(&path("/a.sol")), &[path("/c.sol")]);
        assert!(index.dependents_of(&path("/b.sol")).is_empty());
        assert_eq!(index.dependents_of(&path("/c.sol")), &[path("/a.sol")]);
    }
}
