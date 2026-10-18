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
                kind: forgels_protocol::CompletionItemKind::FUNCTION,
                text_edit: None,
            },
            crate::analyzer::CompletionData {
                label: "stub_two".to_string(),
                detail: "second".to_string(),
                kind: forgels_protocol::CompletionItemKind::VARIABLE,
                text_edit: None,
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

    fn rename(
        &self,
        ctx: &crate::analyzer::ProjectContext,
        new_name: &str,
    ) -> Option<Vec<crate::analyzer::ReferenceData>> {
        if !self.found || new_name.is_empty() {
            return None;
        }
        Some(vec![
            crate::analyzer::ReferenceData::new(
                ctx.path.to_path_buf(),
                forgels_syntax::Span::new(9, 10),
                true,
            ),
            crate::analyzer::ReferenceData::new(
                ctx.path.to_path_buf(),
                forgels_syntax::Span::new(0, 1),
                false,
            ),
        ])
    }

    fn inlay_hints(
        &self,
        _ctx: &crate::analyzer::ProjectContext,
        _range: forgels_syntax::Span,
    ) -> Vec<crate::analyzer::InlayHintData> {
        if !self.found {
            return Vec::new();
        }
        vec![crate::analyzer::InlayHintData::new(
            0,
            "a:".to_string(),
            forgels_protocol::InlayHintKind::PARAMETER,
        )]
    }

    fn semantic_tokens(&self, text: &str) -> Vec<crate::analyzer::SemanticTokenData> {
        if !self.found || text.is_empty() {
            return Vec::new();
        }
        vec![crate::analyzer::SemanticTokenData::new(
            forgels_syntax::Span::new(0, 8),
            14,
            0,
        )]
    }

    fn prepare_call_hierarchy(
        &self,
        ctx: &crate::analyzer::ProjectContext,
    ) -> Option<crate::analyzer::CallHierarchyItemData> {
        if !self.found {
            return None;
        }
        Some(crate::analyzer::CallHierarchyItemData::new(
            "stub_fn".to_string(),
            forgels_protocol::SymbolKind::METHOD,
            "function stub_fn()".to_string(),
            ctx.path.to_path_buf(),
            forgels_syntax::Span::new(0, 12),
            forgels_syntax::Span::new(9, 10),
        ))
    }

    fn incoming_calls(
        &self,
        ctx: &crate::analyzer::ProjectContext,
    ) -> Option<Vec<crate::analyzer::CallHierarchyCallData>> {
        if !self.found {
            return None;
        }
        let item = crate::analyzer::CallHierarchyItemData::new(
            "caller".to_string(),
            forgels_protocol::SymbolKind::METHOD,
            "function caller()".to_string(),
            ctx.path.to_path_buf(),
            forgels_syntax::Span::new(0, 12),
            forgels_syntax::Span::new(9, 10),
        );
        Some(vec![crate::analyzer::CallHierarchyCallData::new(
            item,
            vec![forgels_syntax::Span::new(0, 1)],
        )])
    }

    fn outgoing_calls(
        &self,
        ctx: &crate::analyzer::ProjectContext,
    ) -> Option<Vec<crate::analyzer::CallHierarchyCallData>> {
        if !self.found {
            return None;
        }
        let item = crate::analyzer::CallHierarchyItemData::new(
            "callee".to_string(),
            forgels_protocol::SymbolKind::METHOD,
            "function callee()".to_string(),
            ctx.path.to_path_buf(),
            forgels_syntax::Span::new(0, 12),
            forgels_syntax::Span::new(9, 10),
        );
        Some(vec![crate::analyzer::CallHierarchyCallData::new(
            item,
            vec![forgels_syntax::Span::new(0, 1)],
        )])
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
