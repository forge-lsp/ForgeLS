use std::collections::HashSet;
use std::path::{Path, PathBuf};

use forgels_core::{ProjectContext, ReferenceData};
use forgels_syntax::{Lexer, Span};

use super::{SymbolKind, collect_symbols, word_at};
use crate::{SolidityFile, SolidityLexer, SolidityParser};

const CAP: usize = forgels_core::MAX_REFERENCES;

/// Target declaration for reference search.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Target {
    name: String,
    path: PathBuf,
    span: Span,
    kind: SymbolKind,
    is_member: bool,
    is_emit: bool,
}

fn parse_text(text: &str) -> SolidityFile {
    let tokens = SolidityLexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    file
}

fn dep_files(ctx: &ProjectContext) -> Vec<(PathBuf, SolidityFile)> {
    let mut out = Vec::new();
    for (path, text) in forgels_core::dep_texts(ctx, 16) {
        out.push((path.clone(), parse_text(&text)));
        if out.len() >= 16 {
            break;
        }
    }
    out
}

fn deps_for(ctx: &ProjectContext, path: &Path, text: &str) -> Vec<(PathBuf, SolidityFile)> {
    let tmp = ProjectContext::new(path, text, 0, ctx.workspace);
    dep_files(&tmp)
}

fn is_export_kind(kind: SymbolKind) -> bool {
    matches!(
        kind,
        SymbolKind::Contract
            | SymbolKind::Function
            | SymbolKind::Struct
            | SymbolKind::Enum
            | SymbolKind::Error
            | SymbolKind::Event
            | SymbolKind::Modifier
            | SymbolKind::UserType
    )
}

fn is_local_only(kind: SymbolKind) -> bool {
    matches!(
        kind,
        SymbolKind::StateVar | SymbolKind::LocalVar | SymbolKind::Param
    )
}

/// Resolve the symbol under the cursor to its declaration.
fn resolve_target(
    file: &SolidityFile,
    text: &str,
    offset: u32,
    current: &Path,
    deps: &[(PathBuf, SolidityFile)],
) -> Option<Target> {
    if let Some(member) = super::members::member_context_at(text, offset)
        && (member.member_span.contains(offset) || member.member_span.end == offset)
        && !member.member.is_empty()
    {
        return resolve_member_target(file, text, offset, current, &member, deps);
    }
    if let Some((name, _)) = super::emits::emit_name_at(text, offset) {
        if name.is_empty() {
            return None;
        }
        return resolve_emit_target(file, &name, offset, current, deps);
    }
    let (word, _) = word_at(text, offset)?;
    if word.is_empty() {
        return None;
    }
    resolve_plain_target(file, text, offset, &word, current, deps)
}

fn resolve_member_target(
    file: &SolidityFile,
    text: &str,
    offset: u32,
    current: &Path,
    member: &super::members::MemberContext,
    deps: &[(PathBuf, SolidityFile)],
) -> Option<Target> {
    let scope = collect_symbols(file);
    if super::forge::is_builtin_at(file, text, offset, &scope) {
        return None;
    }
    if super::array::is_array_at(file, text, offset, &scope) {
        return None;
    }
    if super::address::is_address_at(file, text, offset, &scope) {
        return None;
    }
    if super::bytes::is_bytes_at(file, text, offset, &scope) {
        return None;
    }
    if super::bytes::is_string_at(file, text, offset, &scope) {
        return None;
    }
    if super::function::is_function_at(file, text, offset, &scope) {
        return None;
    }
    if super::globals::is_global_at(file, text, offset, &scope) {
        return None;
    }
    if super::meta::is_meta_at(text, offset) {
        return None;
    }
    let kind = super::members::parent_kind_for_member(file, &scope, text, offset, member, deps);
    if !matches!(
        kind,
        super::members::TypeKind::Contract(_) | super::members::TypeKind::Struct(_)
    ) {
        return None;
    }
    if let Some(found) = super::members::members_of(file, &kind)
        .into_iter()
        .find(|item| item.name == member.member)
    {
        return Some(Target {
            name: found.name,
            path: current.to_path_buf(),
            span: found.span,
            kind: found.kind,
            is_member: true,
            is_emit: false,
        });
    }
    let (info, path) =
        super::members::resolve_member_cross_file(file, &kind, &member.member, deps)?;
    Some(Target {
        name: info.name,
        path,
        span: info.span,
        kind: info.kind,
        is_member: true,
        is_emit: false,
    })
}

fn resolve_emit_target(
    file: &SolidityFile,
    name: &str,
    offset: u32,
    current: &Path,
    deps: &[(PathBuf, SolidityFile)],
) -> Option<Target> {
    if let Some(found) = super::emits::resolve_local_emit(file, offset, name) {
        return Some(Target {
            name: name.to_string(),
            path: current.to_path_buf(),
            span: found.span,
            kind: SymbolKind::Event,
            is_member: false,
            is_emit: true,
        });
    }
    for (path, dep_file) in deps {
        if let Some(found) = super::emits::find_emit_in_dep(dep_file, name) {
            return Some(Target {
                name: name.to_string(),
                path: path.clone(),
                span: found.span,
                kind: SymbolKind::Event,
                is_member: false,
                is_emit: true,
            });
        }
    }
    None
}

fn resolve_plain_target(
    file: &SolidityFile,
    text: &str,
    offset: u32,
    word: &str,
    current: &Path,
    deps: &[(PathBuf, SolidityFile)],
) -> Option<Target> {
    if super::yul::is_yul_builtin(word) {
        let tokens = SolidityLexer.tokenize(text);
        let assemblies = super::unknown::assembly_ranges(&tokens);
        if super::yul::is_in_assembly(&assemblies, offset) {
            return None;
        }
    }
    {
        let tokens = SolidityLexer.tokenize(text);
        let assemblies = super::unknown::assembly_ranges(&tokens);
        if super::yul::is_in_assembly(&assemblies, offset) {
            let decls = super::yul::collect_yul_decls(&tokens, &assemblies);
            if super::yul::find_yul_decl(&decls, word, offset).is_some() {
                return None;
            }
        }
    }
    let scope = collect_symbols(file);
    if let Some(symbol) = scope.resolve(word, offset) {
        return Some(Target {
            name: word.to_string(),
            path: current.to_path_buf(),
            span: symbol.span,
            kind: symbol.kind,
            is_member: false,
            is_emit: false,
        });
    }
    if super::forge::is_builtin_at(file, text, offset, &scope) {
        return None;
    }
    if super::array::is_array_at(file, text, offset, &scope) {
        return None;
    }
    if super::address::is_address_at(file, text, offset, &scope) {
        return None;
    }
    if super::bytes::is_bytes_at(file, text, offset, &scope) {
        return None;
    }
    if super::bytes::is_string_at(file, text, offset, &scope) {
        return None;
    }
    if super::function::is_function_at(file, text, offset, &scope) {
        return None;
    }
    if super::globals::is_global_at(file, text, offset, &scope) {
        return None;
    }
    if super::meta::is_meta_at(text, offset) {
        return None;
    }
    for (path, dep_file) in deps {
        let dep_scope = collect_symbols(dep_file);
        if let Some(symbol) = dep_scope
            .export_symbols()
            .into_iter()
            .find(|item| item.name == word)
        {
            return Some(Target {
                name: word.to_string(),
                path: path.clone(),
                span: symbol.span,
                kind: symbol.kind,
                is_member: false,
                is_emit: false,
            });
        }
    }
    None
}

/// Resolve one member usage token to its declaration.
fn resolve_member_usage(
    file: &SolidityFile,
    text: &str,
    at: u32,
    deps: &[(PathBuf, SolidityFile)],
) -> Option<(PathBuf, Span)> {
    let member = super::members::member_context_at(text, at)?;
    if member.member.is_empty() {
        return None;
    }
    let scope = collect_symbols(file);
    let kind = super::members::parent_kind_for_member(file, &scope, text, at, &member, deps);
    if !matches!(
        kind,
        super::members::TypeKind::Contract(_) | super::members::TypeKind::Struct(_)
    ) {
        return None;
    }
    if let Some(found) = super::members::members_of(file, &kind)
        .into_iter()
        .find(|item| item.name == member.member)
    {
        return Some((PathBuf::new(), found.span));
    }
    let (info, path) =
        super::members::resolve_member_cross_file(file, &kind, &member.member, deps)?;
    Some((path, info.span))
}

fn same_span(left: Span, right: Span) -> bool {
    left.start == right.start && left.end == right.end
}

/// Scan one file for references to `target`.
fn scan_plain(
    candidate_path: &Path,
    candidate_text: &str,
    candidate_file: &SolidityFile,
    target: &Target,
    ctx: &ProjectContext,
) -> Vec<ReferenceData> {
    let tokens = SolidityLexer.tokenize(candidate_text);
    let scope = collect_symbols(candidate_file);
    let decls = super::unknown::decl_spans(candidate_file);
    let decl_set: HashSet<(u32, u32)> = decls.iter().map(|span| (span.start, span.end)).collect();
    let imports = super::unknown::import_spans(candidate_file);
    let assemblies = super::unknown::assembly_ranges(&tokens);
    let yul_decls = super::yul::collect_yul_decls(&tokens, &assemblies);
    let members = super::unknown::member_spans(candidate_file);
    let member_set: HashSet<(u32, u32)> =
        members.iter().map(|span| (span.start, span.end)).collect();
    let emits = super::unknown::emit_spans(candidate_file);
    let emit_set: HashSet<(u32, u32)> = emits.iter().map(|span| (span.start, span.end)).collect();
    let func_types = super::unknown::function_type_spans(candidate_file);
    let mut out = Vec::new();
    for token in &tokens {
        if token.kind != crate::SolidityKind::Ident.as_u16() {
            continue;
        }
        if token.text != target.name {
            continue;
        }
        let span = token.span;
        if super::unknown::inside_any(&imports, span.start) {
            continue;
        }
        if super::unknown::inside_any(&assemblies, span.start) {
            if super::yul::is_yul_builtin(&token.text) {
                continue;
            }
            if super::yul::is_yul_decl_token(&yul_decls, span) {
                continue;
            }
            if super::yul::find_yul_decl(&yul_decls, &token.text, span.start).is_some() {
                continue;
            }
        }
        if member_set.contains(&(span.start, span.end)) {
            continue;
        }
        if emit_set.contains(&(span.start, span.end)) {
            continue;
        }
        if super::unknown::inside_any(&func_types, span.start) {
            continue;
        }
        if super::unknown::preceded_by_dot(candidate_text, span.start) {
            continue;
        }
        if candidate_text
            .get(span.start as usize..span.end as usize)
            .is_none()
        {
            continue;
        }
        let is_decl_token = decl_set.contains(&(span.start, span.end));
        if is_decl_token {
            if candidate_path != target.path.as_path() {
                continue;
            }
            let Some(resolved) = scope.resolve(&target.name, span.start) else {
                continue;
            };
            if !same_span(resolved.span, target.span) {
                continue;
            }
            out.push(ReferenceData::new(candidate_path.to_path_buf(), span, true));
            continue;
        }
        if candidate_path == target.path.as_path() {
            let Some(resolved) = scope.resolve(&target.name, span.start) else {
                continue;
            };
            if !same_span(resolved.span, target.span) {
                continue;
            }
            out.push(ReferenceData::new(
                candidate_path.to_path_buf(),
                span,
                false,
            ));
            continue;
        }
        if !is_export_kind(target.kind) {
            continue;
        }
        let Some(resolved) = scope.resolve(&target.name, span.start) else {
            if candidate_closure_contains(ctx, candidate_path, &target.path) {
                out.push(ReferenceData::new(
                    candidate_path.to_path_buf(),
                    span,
                    false,
                ));
            }
            continue;
        };
        let _ = resolved;
    }
    out
}

fn candidate_closure_contains(ctx: &ProjectContext, candidate: &Path, decl: &Path) -> bool {
    ctx.index()
        .transitive_closure(candidate)
        .iter()
        .any(|dep| dep.as_path() == decl)
}

fn scan_member(
    candidate_path: &Path,
    candidate_text: &str,
    candidate_file: &SolidityFile,
    target: &Target,
    ctx: &ProjectContext,
) -> Vec<ReferenceData> {
    let tokens = SolidityLexer.tokenize(candidate_text);
    let decls = super::unknown::decl_spans(candidate_file);
    let decl_set: HashSet<(u32, u32)> = decls.iter().map(|span| (span.start, span.end)).collect();
    let imports = super::unknown::import_spans(candidate_file);
    let assemblies = super::unknown::assembly_ranges(&tokens);
    let func_types = super::unknown::function_type_spans(candidate_file);
    let deps = deps_for(ctx, candidate_path, candidate_text);
    let mut out = Vec::new();
    for token in &tokens {
        if token.kind != crate::SolidityKind::Ident.as_u16() {
            continue;
        }
        if token.text != target.name {
            continue;
        }
        let span = token.span;
        if super::unknown::inside_any(&imports, span.start) {
            continue;
        }
        if super::unknown::inside_any(&assemblies, span.start) {
            continue;
        }
        if super::unknown::inside_any(&func_types, span.start) {
            continue;
        }
        if candidate_text
            .get(span.start as usize..span.end as usize)
            .is_none()
        {
            continue;
        }
        let is_decl_token = decl_set.contains(&(span.start, span.end));
        if is_decl_token {
            if candidate_path != target.path.as_path() {
                continue;
            }
            if !target.span.contains(span.start) {
                continue;
            }
            out.push(ReferenceData::new(candidate_path.to_path_buf(), span, true));
            continue;
        }
        if !super::unknown::preceded_by_dot(candidate_text, span.start) {
            continue;
        }
        let Some((decl_path, decl_span)) =
            resolve_member_usage(candidate_file, candidate_text, span.start, &deps)
        else {
            continue;
        };
        let resolved_path = if decl_path.as_os_str().is_empty() {
            candidate_path.to_path_buf()
        } else {
            decl_path
        };
        if resolved_path != target.path || !same_span(decl_span, target.span) {
            continue;
        }
        out.push(ReferenceData::new(
            candidate_path.to_path_buf(),
            span,
            false,
        ));
    }
    out
}

fn scan_emit(
    candidate_path: &Path,
    candidate_text: &str,
    candidate_file: &SolidityFile,
    target: &Target,
    ctx: &ProjectContext,
) -> Vec<ReferenceData> {
    let tokens = SolidityLexer.tokenize(candidate_text);
    let decls = super::unknown::decl_spans(candidate_file);
    let decl_set: HashSet<(u32, u32)> = decls.iter().map(|span| (span.start, span.end)).collect();
    let imports = super::unknown::import_spans(candidate_file);
    let assemblies = super::unknown::assembly_ranges(&tokens);
    let func_types = super::unknown::function_type_spans(candidate_file);
    let deps = deps_for(ctx, candidate_path, candidate_text);
    let mut out = Vec::new();
    for token in &tokens {
        if token.kind != crate::SolidityKind::Ident.as_u16() {
            continue;
        }
        if token.text != target.name {
            continue;
        }
        let span = token.span;
        if super::unknown::inside_any(&imports, span.start) {
            continue;
        }
        if super::unknown::inside_any(&assemblies, span.start) {
            continue;
        }
        if super::unknown::inside_any(&func_types, span.start) {
            continue;
        }
        if candidate_text
            .get(span.start as usize..span.end as usize)
            .is_none()
        {
            continue;
        }
        let is_decl_token = decl_set.contains(&(span.start, span.end));
        if is_decl_token {
            if candidate_path != target.path.as_path() {
                continue;
            }
            if !target.span.contains(span.start) {
                continue;
            }
            out.push(ReferenceData::new(candidate_path.to_path_buf(), span, true));
            continue;
        }
        if super::unknown::preceded_by_dot(candidate_text, span.start) {
            continue;
        }
        let Some((word, _)) = word_at(candidate_text, span.start) else {
            continue;
        };
        if word != target.name {
            continue;
        }
        if super::emits::emit_name_at(candidate_text, span.start).is_none() {
            continue;
        }
        if candidate_path == target.path.as_path() {
            if super::emits::resolve_local_emit(candidate_file, span.start, &target.name)
                .is_some_and(|found| same_span(found.span, target.span))
            {
                out.push(ReferenceData::new(
                    candidate_path.to_path_buf(),
                    span,
                    false,
                ));
            }
            continue;
        }
        if super::emits::resolve_local_emit(candidate_file, span.start, &target.name).is_some() {
            continue;
        }
        let mut found_target = false;
        for (dep_path, dep_file) in &deps {
            if let Some(found) = super::emits::find_emit_in_dep(dep_file, &target.name)
                && dep_path == &target.path
                && same_span(found.span, target.span)
            {
                found_target = true;
                break;
            }
        }
        if found_target && candidate_closure_contains(ctx, candidate_path, &target.path) {
            out.push(ReferenceData::new(
                candidate_path.to_path_buf(),
                span,
                false,
            ));
        }
    }
    out
}

/// Find all references to the symbol at `offset`.
///
/// Resolves member, emit, then plain targets. Plain locals use a fast
/// path that scans the current file only. Exported names, members, and
/// events scan every tracked Solidity file. Identity is span based:
/// each usage must resolve to the same `(path, span)` as the target,
/// so shadowed names never mix. Results sort by path then offset and
/// cap at `MAX_REFERENCES`.
pub fn find_references(
    file: &SolidityFile,
    text: &str,
    offset: u32,
    current: &Path,
    deps: &[(PathBuf, SolidityFile)],
    ctx: &ProjectContext,
) -> Option<Vec<ReferenceData>> {
    let target = resolve_target(file, text, offset, current, deps)?;
    let mut files: Vec<(PathBuf, String)> = Vec::new();
    if !target.is_member && !target.is_emit && is_local_only(target.kind) {
        files.push((current.to_path_buf(), text.to_string()));
    } else {
        let mut seen: HashSet<PathBuf> = HashSet::new();
        let mut tracked: Vec<(PathBuf, String)> = Vec::new();
        for (path, doc) in ctx.workspace.iter_files() {
            if doc.language != forgels_core::routing::Language::Solidity {
                continue;
            }
            if !seen.insert(path.clone()) {
                continue;
            }
            if path.as_path() == current {
                tracked.push((path.clone(), text.to_string()));
            } else {
                tracked.push((path.clone(), doc.text.clone()));
            }
        }
        if !seen.contains(current) {
            files.push((current.to_path_buf(), text.to_string()));
        }
        tracked.sort_by(|left, right| left.0.cmp(&right.0));
        files.extend(tracked);
    }
    let mut out: Vec<ReferenceData> = Vec::new();
    for (path, file_text) in &files {
        if out.len() >= CAP {
            break;
        }
        let candidate_file = parse_text(file_text);
        let mut found = if target.is_member {
            scan_member(path, file_text, &candidate_file, &target, ctx)
        } else if target.is_emit {
            scan_emit(path, file_text, &candidate_file, &target, ctx)
        } else {
            scan_plain(path, file_text, &candidate_file, &target, ctx)
        };
        out.append(&mut found);
        if out.len() >= CAP {
            break;
        }
    }
    out.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.span.start.cmp(&right.span.start))
            .then(left.span.end.cmp(&right.span.end))
    });
    out.truncate(CAP);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgels_core::Analyzer;

    fn workspace_with(files: &[(&str, &str)]) -> forgels_core::Workspace {
        let mut workspace = forgels_core::Workspace::new();
        for (name, text) in files {
            workspace
                .open(
                    PathBuf::from(name),
                    forgels_core::routing::Language::Solidity,
                    (*text).to_string(),
                    1,
                )
                .expect("open works");
        }
        workspace
    }

    fn refs_in(
        workspace: &forgels_core::Workspace,
        path: &str,
        text: &str,
        needle: &str,
    ) -> Option<Vec<ReferenceData>> {
        let buf = PathBuf::from(path);
        let at = text.rfind(needle).expect("needle") as u32;
        let ctx = ProjectContext::new(buf.as_path(), text, at, workspace);
        let file = parse_text(text);
        let deps = dep_files(&ctx);
        find_references(&file, text, at, buf.as_path(), &deps, &ctx)
    }

    fn refs_at(
        workspace: &forgels_core::Workspace,
        path: &str,
        text: &str,
        at: u32,
    ) -> Option<Vec<ReferenceData>> {
        let buf = PathBuf::from(path);
        let ctx = ProjectContext::new(buf.as_path(), text, at, workspace);
        let file = parse_text(text);
        let deps = dep_files(&ctx);
        find_references(&file, text, at, buf.as_path(), &deps, &ctx)
    }

    #[test]
    fn local_var_lists_decl_and_uses() {
        let text = "contract C {\nfunction f() public {\nuint x = 1;\nx;\nx;\n}\n}";
        let workspace = workspace_with(&[("/A.sol", text)]);
        let found = refs_in(&workspace, "/A.sol", text, "x;\nx;").expect("refs");
        assert_eq!(found.len(), 3);
        assert_eq!(found.iter().filter(|item| item.is_declaration).count(), 1);
    }

    #[test]
    fn shadowed_local_keeps_identities_apart() {
        let text = "contract C {\nuint x;\nfunction f() public {\nuint x = 1;\nx;\n}\n}";
        let workspace = workspace_with(&[("/A.sol", text)]);
        let outer_at = text.find("uint x;").expect("outer") as u32;
        let outer = refs_at(&workspace, "/A.sol", text, outer_at + 5).expect("outer refs");
        assert_eq!(outer.len(), 1);
        assert!(outer[0].is_declaration);
        let inner_use = text.rfind("\nx;\n").expect("use") as u32 + 1;
        let inner = refs_at(&workspace, "/A.sol", text, inner_use).expect("inner refs");
        assert_eq!(inner.len(), 2);
        assert_eq!(inner.iter().filter(|item| item.is_declaration).count(), 1);
    }

    #[test]
    fn cross_file_function_spans_two_files() {
        let b = "contract Vault {\nfunction withdraw(uint a) public {}\n}";
        let a =
            "import \"./B.sol\";\ncontract A {\nfunction f() public {\nwithdraw;\nwithdraw;\n}\n}";
        let mut workspace = workspace_with(&[("/A.sol", a), ("/B.sol", b)]);
        workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
        let found = refs_in(&workspace, "/A.sol", a, "withdraw;").expect("refs");
        assert!(found.len() >= 3, "found: {found:?}");
        assert!(
            found
                .iter()
                .any(|item| item.path.as_path() == Path::new("/B.sol"))
        );
        assert!(
            found
                .iter()
                .any(|item| item.path.as_path() == Path::new("/A.sol"))
        );
        assert_eq!(found.iter().filter(|item| item.is_declaration).count(), 1);
        let decl = found.iter().find(|item| item.is_declaration).expect("decl");
        assert_eq!(decl.path.as_path(), Path::new("/B.sol"));
    }

    #[test]
    fn member_field_lists_decl_and_dot_uses() {
        let text = "struct Person {\nuint age;\n}\ncontract C {\nfunction f() public {\nPerson p;\np.age;\np.age;\n}\n}";
        let workspace = workspace_with(&[("/A.sol", text)]);
        let found = refs_in(&workspace, "/A.sol", text, "p.age;").expect("refs");
        assert_eq!(found.len(), 3, "found: {found:?}");
        assert_eq!(found.iter().filter(|item| item.is_declaration).count(), 1);
    }

    #[test]
    fn emit_event_lists_decl_and_emit_uses() {
        let text = "contract C {\nevent Transfer(address a);\nfunction f() public {\nemit Transfer(a);\nemit Transfer(a);\n}\n}";
        let workspace = workspace_with(&[("/A.sol", text)]);
        let found = refs_in(&workspace, "/A.sol", text, "Transfer(a);").expect("refs");
        assert_eq!(found.len(), 3, "found: {found:?}");
        assert_eq!(found.iter().filter(|item| item.is_declaration).count(), 1);
    }

    #[test]
    fn builtin_member_returns_none() {
        let text = "contract C {\nfunction f() public {\naddress a;\na.balance;\n}\n}";
        let workspace = workspace_with(&[("/A.sol", text)]);
        let at = text.rfind("balance").expect("builtin") as u32;
        assert!(refs_at(&workspace, "/A.sol", text, at).is_none());
    }

    #[test]
    fn unknown_word_returns_none() {
        let text = "contract C {\nfunction f() public {\nmissing;\n}\n}";
        let workspace = workspace_with(&[("/A.sol", text)]);
        let at = text.rfind("missing").expect("word") as u32;
        assert!(refs_at(&workspace, "/A.sol", text, at).is_none());
    }

    #[test]
    fn many_uses_cap_at_max() {
        let mut text = String::from("contract C {\nfunction f() public {\nuint x = 1;\n");
        for _ in 0..150 {
            text.push_str("x;\n");
        }
        text.push_str("}\n}");
        let workspace = workspace_with(&[("/A.sol", text.as_str())]);
        let found = refs_in(&workspace, "/A.sol", &text, "x;").expect("refs");
        assert_eq!(found.len(), forgels_core::MAX_REFERENCES);
    }

    #[test]
    fn analyzer_returns_sorted_and_capped() {
        let b = "contract Vault {\nfunction withdraw(uint a) public {}\n}";
        let a = "import \"./B.sol\";\ncontract A {\nfunction f() public {\nwithdraw;\n}\n}";
        let mut workspace = workspace_with(&[("/A.sol", a), ("/B.sol", b)]);
        workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
        let buf = PathBuf::from("/A.sol");
        let at = a.rfind("withdraw").expect("use") as u32;
        let ctx = ProjectContext::new(buf.as_path(), a, at, &workspace);
        let found = crate::analysis::SolidityAnalyzer
            .references(&ctx)
            .expect("refs");
        assert!(found.len() >= 2);
        let mut sorted = found.clone();
        sorted.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then(left.span.start.cmp(&right.span.start))
        });
        assert_eq!(found, sorted);
        assert!(found.len() <= forgels_core::MAX_REFERENCES);
    }

    #[test]
    fn function_type_inner_name_ignored() {
        let text =
            "contract C {\nfunction (uint cb) external cb;\nfunction f() public {\ncb;\n}\n}";
        let workspace = workspace_with(&[("/A.sol", text)]);
        let found = refs_in(&workspace, "/A.sol", text, "cb;\n}\n}").expect("refs");
        assert_eq!(found.len(), 2);
        assert_eq!(found.iter().filter(|item| item.is_declaration).count(), 1);
    }

    #[test]
    fn outer_var_lists_assembly_use() {
        let text =
            "contract C {\nuint x = 1;\nfunction f() public {\nx;\nassembly {\nlet y := x\n}\n}\n}";
        let workspace = workspace_with(&[("/A.sol", text)]);
        let base = text.rfind("let y := x").expect("assembly use");
        let at = (base + "let y := ".len()) as u32;
        let found = refs_at(&workspace, "/A.sol", text, at).expect("refs");
        assert_eq!(found.len(), 3, "found: {found:?}");
        assert_eq!(found.iter().filter(|item| item.is_declaration).count(), 1);
    }

    #[test]
    fn yul_builtin_use_returns_none() {
        let text = "contract C {\nfunction f() public {\nassembly {\nlet y := add(1, 2)\n}\n}\n}";
        let workspace = workspace_with(&[("/A.sol", text)]);
        let at = text.rfind("add").expect("builtin") as u32;
        assert!(refs_at(&workspace, "/A.sol", text, at).is_none());
    }
}
