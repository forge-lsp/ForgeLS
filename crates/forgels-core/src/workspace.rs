use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Instant;

use forgels_common::log::debug;

use crate::diagnostics::{DEFAULT_PUBLISH_DELAY, PublishGate};
use crate::routing::Language;
use forgels_protocol::Diagnostic;

/// One tracked document: routed language, latest full text, and the
/// editor version from the last open or change notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    pub language: Language,
    pub text: String,
    pub version: i32,
}

/// Sync failure for open, change, or close.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncError {
    AlreadyOpen(PathBuf),
    UnknownDocument(PathBuf),
}

impl fmt::Display for SyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyncError::AlreadyOpen(path) => write!(f, "document already open: {}", path.display()),
            SyncError::UnknownDocument(path) => {
                write!(f, "document not open: {}", path.display())
            }
        }
    }
}

impl std::error::Error for SyncError {}

/// In-memory document state. Full sync only: each change replaces the text.
#[derive(Debug)]
pub struct Workspace {
    files: HashMap<PathBuf, Document>,
    roots: Vec<PathBuf>,
    publish: PublishGate,
    diagnostics: HashMap<PathBuf, Vec<Diagnostic>>,
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

impl Workspace {
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
            roots: Vec::new(),
            publish: PublishGate::new(DEFAULT_PUBLISH_DELAY),
            diagnostics: HashMap::new(),
        }
    }

    /// Workspace with a custom publish debounce delay.
    pub fn with_delay(delay: std::time::Duration) -> Self {
        Self {
            files: HashMap::new(),
            roots: Vec::new(),
            publish: PublishGate::new(delay),
            diagnostics: HashMap::new(),
        }
    }

    /// Apply `didOpen`. Fails when the path is already tracked.
    pub fn open(
        &mut self,
        path: PathBuf,
        language: Language,
        text: String,
        version: i32,
    ) -> Result<(), SyncError> {
        if self.files.contains_key(&path) {
            return Err(SyncError::AlreadyOpen(path));
        }
        debug!("open {}", path.display());
        self.files.insert(
            path,
            Document {
                language,
                text,
                version,
            },
        );
        Ok(())
    }

    /// Apply `didChange` with full text and version. Fails when unknown.
    pub fn change(&mut self, path: &Path, text: String, version: i32) -> Result<(), SyncError> {
        let Some(doc) = self.files.get_mut(path) else {
            return Err(SyncError::UnknownDocument(path.to_path_buf()));
        };
        doc.text = text;
        doc.version = version;
        debug!("change {} to version {version}", path.display());
        Ok(())
    }

    /// Apply `didClose`. Drops the document and all its diagnostics.
    /// Fails when the path is unknown.
    pub fn close(&mut self, path: &Path) -> Result<(), SyncError> {
        if self.files.remove(path).is_none() {
            return Err(SyncError::UnknownDocument(path.to_path_buf()));
        }
        debug!("close {}", path.display());
        self.diagnostics.remove(path);
        Ok(())
    }

    /// Mark a URI dirty after a change. Latest version wins.
    pub fn mark_dirty(&mut self, uri: &str, version: u64, now: Instant) {
        self.publish.mark_dirty(uri, version, now);
    }

    /// Drain URIs quiet for at least the publish delay.
    pub fn take_due_publish(&mut self, now: Instant) -> Vec<(String, u64)> {
        self.publish.take_due(now)
    }

    /// Drain every pending URI, due or not.
    pub fn take_all_pending(&mut self) -> Vec<(String, u64)> {
        self.publish.take_all()
    }

    /// Replace the diagnostics for a document. An empty set removes the
    /// entry, which is how a fixed file clears its squiggles.
    pub fn set_diagnostics(&mut self, path: PathBuf, diagnostics: Vec<Diagnostic>) {
        if diagnostics.is_empty() {
            self.diagnostics.remove(&path);
        } else {
            self.diagnostics.insert(path, diagnostics);
        }
    }

    /// Current diagnostics for a document. Empty when clean or unknown.
    pub fn diagnostics_for(&self, path: &Path) -> &[Diagnostic] {
        self.diagnostics.get(path).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Latest tracked document, if any.
    pub fn get(&self, path: &Path) -> Option<&Document> {
        self.files.get(path)
    }

    /// Replace the root list, from initialize params.
    pub fn set_roots(&mut self, roots: Vec<PathBuf>) {
        self.roots = roots;
    }

    /// Add one root from a workspace folder change. Idempotent.
    pub fn add_root(&mut self, root: PathBuf) {
        if !self.roots.contains(&root) {
            self.roots.push(root);
        }
    }

    /// Remove one root. Returns false when it was not registered.
    pub fn remove_root(&mut self, root: &Path) -> bool {
        let before = self.roots.len();
        self.roots.retain(|known| known != root);
        self.roots.len() != before
    }

    /// Root owning `path`, if any.
    pub fn root_for(&self, path: &Path) -> Option<&PathBuf> {
        self.roots.iter().find(|root| path.starts_with(root))
    }

    /// True when `path` may be tracked. Empty roots allow everything,
    /// for editors that send no workspace folders.
    pub fn allows(&self, path: &Path) -> bool {
        self.roots.is_empty() || self.root_for(path).is_some()
    }

    /// Drop tracked files that no longer sit under a known root.
    /// Empty roots keep everything.
    pub fn evict_outside_roots(&mut self) {
        if self.roots.is_empty() {
            return;
        }
        let doomed: Vec<PathBuf> = self
            .files
            .keys()
            .filter(|path| self.root_for(path).is_none())
            .cloned()
            .collect();
        for path in doomed {
            self.files.remove(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sol_path() -> PathBuf {
        PathBuf::from("contracts/Token.sol")
    }

    #[test]
    fn open_tracks_text_and_language() {
        let mut workspace = Workspace::new();
        workspace
            .open(
                sol_path(),
                Language::Solidity,
                "contract C {}".to_string(),
                1,
            )
            .expect("open works");
        let doc = workspace.get(&sol_path()).expect("tracked");
        assert_eq!(doc.language, Language::Solidity);
        assert_eq!(doc.text, "contract C {}");
    }

    #[test]
    fn open_twice_keeps_first_text() {
        let mut workspace = Workspace::new();
        workspace
            .open(sol_path(), Language::Solidity, "first".to_string(), 1)
            .expect("open works");
        let err = workspace
            .open(sol_path(), Language::Solidity, "second".to_string(), 2)
            .expect_err("second open fails");
        assert_eq!(err, SyncError::AlreadyOpen(sol_path()));
        assert_eq!(workspace.get(&sol_path()).expect("tracked").text, "first");
    }

    #[test]
    fn change_replaces_full_text() {
        let mut workspace = Workspace::new();
        workspace
            .open(sol_path(), Language::Solidity, "v1".to_string(), 1)
            .expect("open works");
        workspace
            .change(&sol_path(), "v2".to_string(), 2)
            .expect("change works");
        assert_eq!(workspace.get(&sol_path()).expect("tracked").text, "v2");
    }

    #[test]
    fn change_unknown_fails() {
        let mut workspace = Workspace::new();
        let err = workspace
            .change(&sol_path(), "v1".to_string(), 1)
            .expect_err("change fails");
        assert_eq!(err, SyncError::UnknownDocument(sol_path()));
    }

    #[test]
    fn close_removes_and_second_close_fails() {
        let mut workspace = Workspace::new();
        workspace
            .open(sol_path(), Language::Solidity, "v1".to_string(), 1)
            .expect("open works");
        workspace.close(&sol_path()).expect("close works");
        assert!(workspace.get(&sol_path()).is_none());
        let err = workspace
            .close(&sol_path())
            .expect_err("second close fails");
        assert_eq!(err, SyncError::UnknownDocument(sol_path()));
    }

    #[test]
    fn full_session_mirrors_editor() {
        let mut workspace = Workspace::new();
        let edits = ["contract A {}", "contract A { uint x; }", "contract B {}"];
        workspace
            .open(sol_path(), Language::Solidity, edits[0].to_string(), 1)
            .expect("open works");
        for (index, edit) in edits[1..].iter().enumerate() {
            workspace
                .change(&sol_path(), (*edit).to_string(), index as i32 + 2)
                .expect("change works");
        }
        let tracked = workspace.get(&sol_path()).expect("tracked");
        assert_eq!(tracked.text, "contract B {}");
        assert_eq!(tracked.version, 3);
        workspace.close(&sol_path()).expect("close works");
        assert!(workspace.get(&sol_path()).is_none());
    }

    #[test]
    fn version_tracks_open_and_change() {
        let mut workspace = Workspace::new();
        workspace
            .open(sol_path(), Language::Solidity, "v1".to_string(), 4)
            .expect("open works");
        assert_eq!(workspace.get(&sol_path()).expect("tracked").version, 4);
        workspace
            .change(&sol_path(), "v2".to_string(), 9)
            .expect("change works");
        assert_eq!(workspace.get(&sol_path()).expect("tracked").version, 9);
    }

    #[test]
    fn roots_track_multiple_folders() {
        let mut workspace = Workspace::new();
        let backend = PathBuf::from("/repo/backend");
        let frontend = PathBuf::from("/repo/frontend");
        workspace.set_roots(vec![backend.clone(), frontend.clone()]);
        assert_eq!(
            workspace.root_for(&PathBuf::from("/repo/backend/Token.sol")),
            Some(&backend)
        );
        assert_eq!(
            workspace.root_for(&PathBuf::from("/repo/frontend/App.sol")),
            Some(&frontend)
        );
        assert!(workspace.root_for(&PathBuf::from("/other/X.sol")).is_none());
    }

    #[test]
    fn roots_change_dynamically() {
        let mut workspace = Workspace::new();
        let added = PathBuf::from("/repo/new");
        workspace.add_root(added.clone());
        workspace.add_root(added.clone());
        assert!(workspace.allows(&PathBuf::from("/repo/new/A.sol")));
        assert!(!workspace.allows(&PathBuf::from("/repo/other/A.sol")));
        assert!(workspace.remove_root(&added));
        assert!(!workspace.remove_root(&added));
        // Last root gone means empty roots, which allow everything.
        assert!(workspace.allows(&PathBuf::from("/repo/new/A.sol")));
    }

    #[test]
    fn empty_roots_allow_everything() {
        let workspace = Workspace::new();
        assert!(workspace.allows(&PathBuf::from("/anywhere/A.sol")));
    }

    fn sample_diagnostic(message: &str) -> Diagnostic {
        use forgels_syntax::Span;
        Diagnostic::error(
            crate::diagnostics::span_to_range("contract A {}", Span::new(0, 8)),
            message.to_string(),
        )
    }

    #[test]
    fn diagnostics_roundtrip_per_document() {
        let mut workspace = Workspace::new();
        workspace
            .open(
                sol_path(),
                Language::Solidity,
                "contract A {}".to_string(),
                1,
            )
            .expect("open works");
        assert!(workspace.diagnostics_for(&sol_path()).is_empty());
        workspace.set_diagnostics(
            sol_path(),
            vec![sample_diagnostic("first"), sample_diagnostic("second")],
        );
        let current = workspace.diagnostics_for(&sol_path());
        assert_eq!(current.len(), 2);
        assert_eq!(current[0].message, "first");
    }

    #[test]
    fn fixed_file_clears_its_diagnostics() {
        let mut workspace = Workspace::new();
        workspace
            .open(
                sol_path(),
                Language::Solidity,
                "contract A {}".to_string(),
                1,
            )
            .expect("open works");
        workspace.set_diagnostics(sol_path(), vec![sample_diagnostic("oops")]);
        workspace.set_diagnostics(sol_path(), Vec::new());
        assert!(workspace.diagnostics_for(&sol_path()).is_empty());
    }

    #[test]
    fn close_drops_all_document_diagnostics() {
        let mut workspace = Workspace::new();
        workspace
            .open(
                sol_path(),
                Language::Solidity,
                "contract A {}".to_string(),
                1,
            )
            .expect("open works");
        workspace.set_diagnostics(sol_path(), vec![sample_diagnostic("oops")]);
        workspace.close(&sol_path()).expect("close works");
        assert!(workspace.diagnostics_for(&sol_path()).is_empty());
    }

    #[test]
    fn unknown_document_has_no_diagnostics() {
        let workspace = Workspace::new();
        assert!(workspace.diagnostics_for(&sol_path()).is_empty());
    }
}
