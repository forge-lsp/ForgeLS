use std::collections::HashSet;
use std::path::PathBuf;

use forgels_core::{CodeActionData, MAX_CODE_ACTIONS, ProjectContext, analyzer::TextEdit};
use forgels_lint::RuleId;
use forgels_syntax::{AstNode, Lexer, Span};

use super::collect_symbols;
use crate::{SolidityFile, SolidityLexer, SolidityParser};

const CAP: usize = MAX_CODE_ACTIONS;
const QUICKFIX: &str = "quickfix";

fn overlaps(left: Span, right: Span) -> bool {
    left.start <= right.end && left.end >= right.start
}

fn basename_of(path: &std::path::Path) -> Option<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
}

fn already_imported(file: &SolidityFile, basename: &str) -> bool {
    for directive in file.imports() {
        if let Some(raw) = directive.path()
            && (raw == basename
                || raw.ends_with(&format!("/{basename}"))
                || raw.ends_with(basename))
        {
            return true;
        }
    }
    false
}

fn parse_text(text: &str) -> SolidityFile {
    let tokens = SolidityLexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    file
}

fn dep_names(deps: &[(PathBuf, SolidityFile)]) -> HashSet<String> {
    let mut out = HashSet::new();
    for (_, dep_file) in deps {
        let scope = collect_symbols(dep_file);
        for symbol in scope.export_symbols() {
            out.insert(symbol.name.clone());
        }
    }
    out
}

fn single_tracked_provider(
    name: &str,
    ctx: &ProjectContext,
    current: &std::path::Path,
) -> Option<String> {
    let mut hits: Vec<PathBuf> = Vec::new();
    for (path, doc) in ctx.workspace.iter_files() {
        if doc.language != forgels_core::routing::Language::Solidity {
            continue;
        }
        if path.as_path() == current {
            continue;
        }
        let candidate = parse_text(&doc.text);
        let scope = collect_symbols(&candidate);
        if scope.export_symbols().iter().any(|item| item.name == name) {
            if hits.contains(path) {
                continue;
            }
            hits.push(path.clone());
            if hits.len() > 1 {
                return None;
            }
        }
    }
    if hits.len() != 1 {
        return None;
    }
    basename_of(&hits[0])
}

fn import_fixes(
    file: &SolidityFile,
    text: &str,
    range: Span,
    deps: &[(PathBuf, SolidityFile)],
    ctx: &ProjectContext,
    current: &std::path::Path,
    out: &mut Vec<CodeActionData>,
) {
    if text.is_empty() {
        return;
    }
    let scope = collect_symbols(file);
    let available = dep_names(deps);
    for diag in super::unknown::unknown_identifier_diagnostics(file, text, &scope, &[]) {
        if out.len() >= CAP {
            break;
        }
        if !overlaps(diag.span, range) {
            continue;
        }
        let Some(name) = text
            .get(diag.span.start as usize..diag.span.end as usize)
            .map(|slice| slice.to_string())
        else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        if available.contains(&name) {
            continue;
        }
        let Some(base) = single_tracked_provider(&name, ctx, current) else {
            continue;
        };
        if already_imported(file, &base) {
            continue;
        }
        let mut insert = String::from("import \"./");
        insert.push_str(&base);
        insert.push_str("\";\n");
        let mut title = String::from("Import '");
        title.push_str(&name);
        title.push_str("' from \"./");
        title.push_str(&base);
        title.push('"');
        out.push(CodeActionData::new(
            title,
            QUICKFIX.to_string(),
            TextEdit::new(Span::new(0, 0), insert),
        ));
    }
}

fn override_fixes(file: &SolidityFile, range: Span, out: &mut Vec<CodeActionData>) {
    for diag in super::override_diagnostics::override_diagnostics(file) {
        if out.len() >= CAP {
            break;
        }
        if !diag.message.contains("missing 'override'") {
            continue;
        }
        if diag.rule != RuleId::OVERRIDE {
            continue;
        }
        if !overlaps(diag.span, range) {
            continue;
        }
        for contract in file.contracts() {
            for function in contract.functions() {
                let Some(name_span) = function.name_span() else {
                    continue;
                };
                if name_span.start != diag.span.start || name_span.end != diag.span.end {
                    continue;
                }
                let Some(name) = function.name() else {
                    continue;
                };
                if name.is_empty() || function.is_override() {
                    continue;
                }
                let Some(body) = function.body() else {
                    continue;
                };
                let at = body.syntax().span.start;
                let mut title = String::from("Add 'override' to '");
                title.push_str(&name);
                title.push('\'');
                out.push(CodeActionData::new(
                    title,
                    QUICKFIX.to_string(),
                    TextEdit::new(Span::new(at, at), "override ".to_string()),
                ));
            }
        }
    }
}

/// Quickfixes inside `range`: missing import plus missing override.
///
/// Import fires only when exactly one tracked file exports the
/// unresolved name, so ambiguous names stay silent. Names already
/// available through deps stay silent, and already imported files stay
/// silent. Override fires only for O1 diagnostics and inserts
/// `override` before the function body. Both return `edit` only:
/// no `command`, no `resolve` in V1. Caps at `MAX_CODE_ACTIONS`.
pub fn code_actions_in(
    file: &SolidityFile,
    ctx: &ProjectContext,
    range: Span,
    deps: &[(PathBuf, SolidityFile)],
) -> Vec<CodeActionData> {
    let mut out = Vec::new();
    import_fixes(file, ctx.text, range, deps, ctx, ctx.path, &mut out);
    override_fixes(file, range, &mut out);
    out.sort_by(|left, right| {
        left.edit
            .span
            .start
            .cmp(&right.edit.span.start)
            .then(left.title.cmp(&right.title))
    });
    out.truncate(CAP);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgels_core::routing::Language;

    fn parse(text: &str) -> SolidityFile {
        parse_text(text)
    }

    fn workspace_with(files: &[(&str, &str)]) -> forgels_core::Workspace {
        let mut workspace = forgels_core::Workspace::new();
        for (name, text) in files {
            workspace
                .open(
                    PathBuf::from(name),
                    Language::Solidity,
                    (*text).to_string(),
                    1,
                )
                .expect("open works");
        }
        workspace
    }

    #[test]
    fn import_single_provider_proposes_insert() {
        let dep_text = "contract Vault {\nfunction withdraw(uint a) public {}\n}";
        let cur = "contract A {\nfunction f() public {\nwithdraw;\n}\n}";
        let workspace = workspace_with(&[("/B.sol", dep_text), ("/A.sol", cur)]);
        let buf = PathBuf::from("/A.sol");
        let at = cur.find("withdraw;").expect("use") as u32;
        let ctx = ProjectContext::new(buf.as_path(), cur, at, &workspace);
        let file = parse(cur);
        let found = code_actions_in(&file, &ctx, Span::new(at, at + 8), &[]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, QUICKFIX);
        assert!(found[0].title.contains("withdraw"));
        assert!(found[0].edit.new_text.contains("B.sol"));
    }

    #[test]
    fn import_ambiguous_or_missing_stays_silent() {
        let cur = "contract A {\nfunction f() public {\nmissing;\n}\n}";
        let workspace = workspace_with(&[("/A.sol", cur)]);
        let buf = PathBuf::from("/A.sol");
        let at = cur.find("missing").expect("use") as u32;
        let ctx = ProjectContext::new(buf.as_path(), cur, at, &workspace);
        let file = parse(cur);
        assert!(code_actions_in(&file, &ctx, Span::new(at, at + 7), &[]).is_empty());
        let a = "contract X {\nfunction dup() public {}\n}";
        let b = "contract Y {\nfunction dup() public {}\n}";
        let use_text = "contract C {\nfunction f() public {\ndup;\n}\n}";
        let workspace = workspace_with(&[("/A.sol", a), ("/B.sol", b), ("/C.sol", use_text)]);
        let buf = PathBuf::from("/C.sol");
        let at = use_text.find("dup;").expect("use") as u32;
        let ctx = ProjectContext::new(buf.as_path(), use_text, at, &workspace);
        let use_file = parse(use_text);
        assert!(code_actions_in(&use_file, &ctx, Span::new(at, at + 3), &[]).is_empty());
    }

    #[test]
    fn override_missing_proposes_edit() {
        let text = "contract B {\nfunction greet(uint a) public virtual { a; }\n}\ncontract D is B {\nfunction greet(uint a) public { a; }\n}";
        let workspace = workspace_with(&[("/A.sol", text)]);
        let buf = PathBuf::from("/A.sol");
        let at = text.rfind("greet").expect("use") as u32;
        let ctx = ProjectContext::new(buf.as_path(), text, at, &workspace);
        let file = parse(text);
        let found = code_actions_in(&file, &ctx, Span::new(at, at + 5), &[]);
        assert_eq!(found.len(), 1);
        assert!(found[0].title.contains("override"));
        assert_eq!(found[0].edit.new_text, "override ");
    }

    #[test]
    fn range_miss_stays_silent() {
        let text = "contract B {\nfunction greet(uint a) public virtual { a; }\n}\ncontract D is B {\nfunction greet(uint a) public { a; }\n}";
        let workspace = workspace_with(&[("/A.sol", text)]);
        let buf = PathBuf::from("/A.sol");
        let ctx = ProjectContext::new(buf.as_path(), text, 0, &workspace);
        let file = parse(text);
        assert!(code_actions_in(&file, &ctx, Span::new(0, 1), &[]).is_empty());
    }
}
