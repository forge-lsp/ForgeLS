use std::path::{Path, PathBuf};

use forgels_core::{Analyzer, ProjectContext, Workspace, routing::Language};
use forgels_lang_solidity::{SolidityAnalyzer, SolidityLexer, SolidityParser};
use forgels_lint::{LintDiagnostic, LintFix, RuleId, apply};
use forgels_syntax::{AstNode, Lexer, Span};

/// Build an ephemeral workspace from scanned files.
///
/// Lets `code_actions` reuse the same single-provider lookup as the
/// server without any persistent state.
pub fn ephemeral_workspace(files: &[(PathBuf, String)]) -> Workspace {
    let mut workspace = Workspace::new();
    for (path, text) in files {
        let _ = workspace.open(path.clone(), Language::Solidity, text.clone(), 1);
    }
    workspace
}

fn first_action_fix(
    analyzer: &SolidityAnalyzer,
    path: &Path,
    text: &str,
    span: Span,
    workspace: &Workspace,
) -> Option<LintFix> {
    let ctx = ProjectContext::new(path, text, span.start, workspace);
    let mut actions = analyzer.code_actions(&ctx, span);
    if actions.is_empty() {
        return None;
    }
    actions.sort_by(|left, right| {
        left.edit
            .span
            .start
            .cmp(&right.edit.span.start)
            .then(left.title.cmp(&right.title))
    });
    let action = actions.into_iter().next()?;
    Some(LintFix::safe(action.edit.span, action.edit.new_text))
}

// Minimal O1 replicate: insert `override` before the function body.
// Mirrors analysis/code_action.rs override_fixes O1 path. Fallback
// only, used when the workspace reuse above yields nothing.
fn override_fallback_fix(text: &str, span: Span) -> Option<LintFix> {
    let tokens = SolidityLexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    for contract in file.contracts() {
        for function in contract.functions() {
            let Some(name_span) = function.name_span() else {
                continue;
            };
            if name_span != span {
                continue;
            }
            if function.is_override() {
                continue;
            }
            let Some(body) = function.body() else {
                continue;
            };
            let at = body.syntax().span.start;
            return Some(LintFix::safe(Span::new(at, at), "override ".to_string()));
        }
    }
    None
}

/// Attach safe fixes to diagnostics in place.
///
/// Reuses `Analyzer::code_actions` (import single-provider plus
/// override O1, both `quickfix` with `edit` only). Falls back to the
/// minimal O1 replicate above. No other rules gain fixes.
pub fn attach_fixes(
    path: &Path,
    text: &str,
    diagnostics: &mut [LintDiagnostic],
    workspace: &Workspace,
) {
    if text.is_empty() || diagnostics.is_empty() {
        return;
    }
    let analyzer = SolidityAnalyzer::new();
    for diag in diagnostics.iter_mut() {
        if diag.fix.is_some() {
            continue;
        }
        // Only import and override diagnostics gain fixes. The action
        // list itself filters to those two, so other rules stay fixless.
        let fixable = diag.rule == RuleId::OVERRIDE
            || diag.rule == RuleId::UNRESOLVED_IDENTIFIER
            || diag.rule == RuleId::UNRESOLVED_IMPORT;
        if !fixable {
            continue;
        }
        if let Some(fix) = first_action_fix(&analyzer, path, text, diag.span, workspace) {
            diag.fix = Some(fix);
            continue;
        }
        if diag.rule == RuleId::OVERRIDE && diag.message.contains("missing 'override'") {
            diag.fix = override_fallback_fix(text, diag.span);
        }
    }
}

fn overlaps(left: Span, right: Span) -> bool {
    left.start <= right.end && left.end >= right.start
}

/// Apply attached fixes and write the file in place.
///
/// Uses the shared engine applier (safe only, first of overlapping
/// wins). Returns per-diagnostic fixed flags parallel to input.
/// Write failure is an operational error string for exit 2.
pub fn apply_and_write(
    path: &Path,
    text: &str,
    diagnostics: &[LintDiagnostic],
) -> Result<(String, Vec<bool>), String> {
    let mut indexed: Vec<(usize, LintFix)> = Vec::new();
    for (index, diag) in diagnostics.iter().enumerate() {
        if let Some(fix) = &diag.fix {
            indexed.push((index, fix.clone()));
        }
    }
    let mut fixed = vec![false; diagnostics.len()];
    if indexed.is_empty() {
        return Ok((text.to_string(), fixed));
    }
    indexed.sort_by(|left, right| {
        left.1
            .span
            .start
            .cmp(&right.1.span.start)
            .then(left.1.span.end.cmp(&right.1.span.end))
    });
    let fixes: Vec<LintFix> = indexed.iter().map(|(_, fix)| fix.clone()).collect();
    let outcome = apply(text, &fixes);
    let mut kept: Option<Span> = None;
    let mut cursor = 0usize;
    let mut applied_spans: Vec<Span> = Vec::new();
    for (_, fix) in &indexed {
        if fix.applicability != forgels_lint::Applicability::Safe {
            continue;
        }
        if let Some(prev) = kept
            && overlaps(prev, fix.span)
        {
            continue;
        }
        let start = fix.span.start as usize;
        let end = fix.span.end as usize;
        let (Some(_), Some(_)) = (text.get(cursor..start), text.get(start..end)) else {
            continue;
        };
        cursor = end;
        kept = Some(fix.span);
        applied_spans.push(fix.span);
    }
    for (index, fix) in &indexed {
        if applied_spans.contains(&fix.span)
            && let Some(flag) = fixed.get_mut(*index)
        {
            *flag = true;
        }
    }
    if outcome.text != text
        && let Err(err) = forgels_common::fs::write_string(path, &outcome.text)
    {
        return Err(format!("fls: cannot write {}: {err}", path.display()));
    }
    Ok((outcome.text, fixed))
}
