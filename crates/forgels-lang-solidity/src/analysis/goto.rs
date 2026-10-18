use super::{collect_symbols, word_at};
use crate::{SolidityFile, SolidityLexer, SolidityParser};
use forgels_syntax::{Lexer, Span};

/// Jump target for the word at `offset`: the declaration span.
///
/// Member positions after a dot jump to the field or contract member.
/// Array members (`length`, `push`, `pop`), address members
/// (`balance`, `transfer`, and rest), bytes (`length`), function
/// (`selector`, `gas`, `value`), globals (`msg.*`, `block.*`, `tx.*`,
/// `abi.*`), user type `wrap` and `unwrap`, and `type()` metadata return
/// `None` by design: their spans are synthetic with no declaration in
/// the workspace. Super members jump to the base declaration, enum
/// members jump to the variant token. Emit name
/// slots jump to the event declaration only. Assembly bodies resolve
/// flat Yul decls then outer scope; Yul builtins return `None` by
/// design. Positions on whitespace,
/// operators, or undeclared words return `None`.
/// Forge builtins (`vm.*`, `console.*`, `Test` asserts) return `None` by
/// design: they have no declaration in the workspace. The caller guards
/// dep lookup the same way so builtins never jump to coincidental matches.
/// The caller attaches the document URI; the span alone locates the jump.
pub fn goto_definition(file: &SolidityFile, text: &str, offset: u32) -> Option<Span> {
    if let Some(found) = goto_yul_definition(text, offset) {
        return Some(found);
    }
    if let Some(type_ctx) = super::meta::type_context_at(text, offset)
        && (type_ctx.member_span.contains(offset) || type_ctx.member_span.end == offset)
    {
        return None;
    }
    if let Some(ctx) = super::members::member_context_at(text, offset)
        && (ctx.member_span.contains(offset) || ctx.member_span.end == offset)
    {
        let scope = collect_symbols(file);
        let chained = super::members::chain_before(text, ctx.dot_pos).len() >= 2;
        let kind = super::members::parent_kind_for_member(file, &scope, text, offset, &ctx, &[]);
        if matches!(
            kind,
            super::members::TypeKind::Array { .. }
                | super::members::TypeKind::Address { .. }
                | super::members::TypeKind::Bytes
                | super::members::TypeKind::String
                | super::members::TypeKind::Function
                | super::members::TypeKind::Meta(_)
                | super::members::TypeKind::Global(_)
                | super::members::TypeKind::UserType(_)
        ) {
            return None;
        }
        let found = super::members::members_of(file, &kind)
            .into_iter()
            .find(|member| member.name == ctx.member);
        if let Some(found) = found {
            return Some(found.span);
        }
        if chained {
            return None;
        }
        let scope = collect_symbols(file);
        let (found, _) = super::using::resolve_using_member(
            file,
            &scope,
            offset,
            &ctx.receiver,
            &ctx.member,
            &[],
        )?;
        return Some(found.span);
    }
    if let Some((name, _)) = super::emits::emit_name_at(text, offset) {
        let found = super::emits::resolve_local_emit(file, offset, &name)?;
        return Some(found.span);
    }
    if let Some((name, _)) = super::errors::revert_name_at(text, offset) {
        let found = super::errors::resolve_local_error(file, offset, &name)?;
        return Some(found.span);
    }
    let (word, _) = word_at(text, offset)?;
    let scope = collect_symbols(file);
    scope.resolve(&word, offset).map(|symbol| symbol.span)
}

/// Yul jump target inside assembly, or `None` outside.
///
/// Flat decls resolve to their declaration span. Builtins return `None`
/// by design and the caller must guard dep lookup the same way. Outer
/// scope falls through to the normal path via `None` here: the caller
/// below resolves it. Returns `Some` only for Yul decl hits.
fn goto_yul_definition(text: &str, offset: u32) -> Option<Span> {
    let tokens = SolidityLexer.tokenize(text);
    let assemblies = super::unknown::assembly_ranges(&tokens);
    if !super::yul::is_in_assembly(&assemblies, offset) {
        return None;
    }
    let (word, _) = word_at(text, offset)?;
    if word.is_empty() {
        return None;
    }
    if super::yul::is_yul_builtin(&word) {
        return None;
    }
    let decls = super::yul::collect_yul_decls(&tokens, &assemblies);
    super::yul::find_yul_decl(&decls, &word, offset).map(|decl| decl.span)
}

/// True when the cursor sits on a Yul builtin.
///
/// Goto guard uses this to return `None` before dep lookup, so Yul
/// opcodes never jump to coincidental same-name declarations.
pub(crate) fn is_yul_builtin_at(text: &str, offset: u32) -> bool {
    let tokens = SolidityLexer.tokenize(text);
    let assemblies = super::unknown::assembly_ranges(&tokens);
    if !super::yul::is_in_assembly(&assemblies, offset) {
        return false;
    }
    let Some((word, _)) = word_at(text, offset) else {
        return false;
    };
    super::yul::is_yul_builtin(&word)
}

/// Exported declaration span for `word` inside a dependency text.
///
/// Parses with recovery so broken deps still resolve. Only exported
/// symbols cross files: contracts, functions, structs, enums, errors,
/// events, modifiers, and user types. Locals, params, and state vars
/// stay local.
pub fn resolve_in_dep(dep_text: &str, word: &str) -> Option<Span> {
    let tokens = SolidityLexer.tokenize(dep_text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    let scope = collect_symbols(&file);
    scope
        .export_symbols()
        .iter()
        .find(|symbol| symbol.name == word)
        .map(|symbol| symbol.span)
}
