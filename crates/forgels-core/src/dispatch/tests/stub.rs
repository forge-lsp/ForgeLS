use super::super::{Session, handle_message};
use crate::workspace::Workspace;

#[derive(Debug)]
struct StubAnalyzer {
    found: bool,
    cross: Option<(std::path::PathBuf, forgels_syntax::Span)>,
}

impl crate::analyzer::Analyzer for StubAnalyzer {
    fn hover(&self, _ctx: &crate::analyzer::ProjectContext) -> Option<crate::analyzer::HoverData> {
        if self.found {
            Some(crate::analyzer::HoverData {
                contents: "stub detail".to_string(),
                span: forgels_syntax::Span::new(13, 18),
            })
        } else {
            None
        }
    }

    fn complete(
        &self,
        _ctx: &crate::analyzer::ProjectContext,
    ) -> Vec<crate::analyzer::CompletionData> {
        vec![
            crate::analyzer::CompletionData {
                label: "stub_one".to_string(),
                detail: "first".to_string(),
            },
            crate::analyzer::CompletionData {
                label: "stub_two".to_string(),
                detail: "second".to_string(),
            },
        ]
    }

    fn goto(&self, _ctx: &crate::analyzer::ProjectContext) -> Option<forgels_syntax::Span> {
        Some(forgels_syntax::Span::new(9, 10))
    }

    fn goto_location(
        &self,
        ctx: &crate::analyzer::ProjectContext,
    ) -> Option<crate::analyzer::GotoTarget> {
        if let Some((path, span)) = &self.cross {
            return Some(crate::analyzer::GotoTarget::new(path.clone(), *span));
        }
        self.goto(ctx)
            .map(|span| crate::analyzer::GotoTarget::new(ctx.path.to_path_buf(), span))
    }

    fn diagnostics(&self, text: &str) -> Vec<crate::analyzer::DiagnosticData> {
        if text.contains("broken") {
            vec![crate::analyzer::DiagnosticData {
                span: forgels_syntax::Span::new(0, 6),
                message: "stub broken".to_string(),
            }]
        } else {
            Vec::new()
        }
    }
}

pub(crate) fn session_with_stub(found: bool) -> Session {
    let mut session = Session::new();
    session.analyzers.insert(
        crate::routing::Language::Solidity,
        Box::new(StubAnalyzer { found, cross: None }),
    );
    session
}

pub(crate) fn session_with_cross(path: std::path::PathBuf, span: forgels_syntax::Span) -> Session {
    let mut session = Session::new();
    session.analyzers.insert(
        crate::routing::Language::Solidity,
        Box::new(StubAnalyzer {
            found: true,
            cross: Some((path, span)),
        }),
    );
    session
}

/// Feed a bare initialize so later requests pass the lifecycle gate.
pub(crate) fn init(session: &mut Session, workspace: &mut Workspace) {
    let body = r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"capabilities":{}}}"#;
    assert!(handle_message(session, workspace, body).is_some());
}

pub(crate) fn open_sol(session: &mut Session, workspace: &mut Workspace) {
    init(session, workspace);
    let open = r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///repo/A.sol","languageId":"solidity","version":1,"text":"contract A {}"}}}"#;
    assert!(handle_message(session, workspace, open).is_none());
}
