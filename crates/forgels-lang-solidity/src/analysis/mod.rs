use forgels_core::{
    Analyzer, CompletionData, DiagnosticData, GotoTarget, HoverData, ProjectContext,
    analyzer::{ImportRef, TextEdit},
};
use forgels_syntax::{AstNode, Lexer, Span};

use crate::{
    ContractDef, EnumDef, FunctionDef, SolidityFile, SolidityLexer, SolidityParser, StructDef,
};

pub(crate) mod address;
pub(crate) mod array;
pub(crate) mod bytes;
mod calls;
mod completion;
mod emits;
pub(crate) mod forge;
pub(crate) mod function;
pub(crate) mod globals;
mod goto;
mod hover;
mod imports;
pub(crate) mod inheritance;
pub(crate) mod members;
pub(crate) mod meta;
mod references;
mod rename;
mod signature;
mod symbols;
mod unknown;

#[cfg(test)]
mod tests;

pub use completion::{
    CompletionItem, CompletionKind, complete_at, complete_bare_imports, complete_import_paths,
    completion_item_kind, exports_of, import_inner_span, import_path_at,
};
pub use goto::{goto_definition, resolve_in_dep};
pub use hover::{Hover, hover_at};
pub use imports::imports_in;
pub use references::find_references;
pub use rename::{find_rename, is_valid_rename};
pub use signature::signature_at;
pub use symbols::document_symbols_in;

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

/// Word under the cursor: longest ident-char run containing `offset`.
pub(crate) fn word_at(text: &str, offset: u32) -> Option<(String, Span)> {
    let bytes = text.as_bytes();
    let index = offset as usize;
    if index >= bytes.len() || !is_word_byte(bytes[index]) {
        return None;
    }
    let mut start = index;
    while start > 0 && is_word_byte(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = index;
    while end < bytes.len() && is_word_byte(bytes[end]) {
        end += 1;
    }
    text.get(start..end)
        .map(|word| (word.to_string(), Span::new(start as u32, end as u32)))
}

/// Declaration kind for hover, completion, and goto-definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Contract,
    Function,
    Struct,
    Enum,
    Error,
    Event,
    Modifier,
    UserType,
    StateVar,
    LocalVar,
    Param,
    Field,
}

/// One declaration: name, kind, node span, and hover-ready detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub span: Span,
    pub detail: String,
}

impl Symbol {
    fn new(name: String, kind: SymbolKind, span: Span, detail: String) -> Self {
        Self {
            name,
            kind,
            span,
            detail,
        }
    }
}

/// Nested scopes: file holds contracts, contracts hold members,
/// functions hold params and locals.
#[derive(Debug)]
pub struct Scope {
    span: Span,
    symbols: Vec<Symbol>,
    children: Vec<Scope>,
}

impl Scope {
    fn new(span: Span) -> Self {
        Self {
            span,
            symbols: Vec::new(),
            children: Vec::new(),
        }
    }

    /// Resolve `name` at `offset`. The deepest scope wins, so locals
    /// shadow params and params shadow state variables. Cursor at the
    /// scope end edge counts as inside, same as `collect_visible`:
    /// typing happens at offsets no token covers yet.
    pub fn resolve(&self, name: &str, offset: u32) -> Option<&Symbol> {
        for child in &self.children {
            if (child.span.contains(offset) || child.span.end == offset)
                && let Some(found) = child.resolve(name, offset)
            {
                return Some(found);
            }
        }
        self.symbols.iter().find(|symbol| symbol.name == name)
    }

    /// All symbols visible at `offset`, innermost first.
    pub fn visible_at(&self, offset: u32) -> Vec<&Symbol> {
        let mut seen: Vec<&Symbol> = Vec::new();
        self.collect_visible(offset, &mut seen);
        seen
    }

    fn collect_visible<'a>(&'a self, offset: u32, out: &mut Vec<&'a Symbol>) {
        out.extend(self.symbols.iter());
        for child in &self.children {
            // Cursor at the scope end edge counts as inside: typing
            // happens at offsets no token covers yet.
            if child.span.contains(offset) || child.span.end == offset {
                child.collect_visible(offset, out);
            }
        }
    }

    /// Contract level symbols: contracts, functions, structs, enums,
    /// errors, events, modifiers, and user types.
    ///
    /// Used for cross file completion. State vars, params, and locals
    /// stay local and never cross files.
    pub fn export_symbols(&self) -> Vec<&Symbol> {
        let mut out: Vec<&Symbol> = Vec::new();
        for symbol in &self.symbols {
            if matches!(
                symbol.kind,
                SymbolKind::Contract
                    | SymbolKind::Function
                    | SymbolKind::Struct
                    | SymbolKind::Enum
                    | SymbolKind::Error
                    | SymbolKind::Event
                    | SymbolKind::Modifier
                    | SymbolKind::UserType
            ) {
                out.push(symbol);
            }
        }
        for child in &self.children {
            for symbol in &child.symbols {
                if matches!(
                    symbol.kind,
                    SymbolKind::Contract
                        | SymbolKind::Function
                        | SymbolKind::Struct
                        | SymbolKind::Enum
                        | SymbolKind::Error
                        | SymbolKind::Event
                        | SymbolKind::Modifier
                        | SymbolKind::UserType
                ) {
                    out.push(symbol);
                }
            }
        }
        out
    }
}

/// Collect every declaration in the file into nested scopes.
///
/// Uses recovery parse output as is: partial nodes may miss names,
/// and nameless contracts are skipped so they never surface.
pub fn collect_symbols(file: &SolidityFile) -> Scope {
    let mut root = Scope::new(file.syntax().span);
    for item in file.structs() {
        if item.name().is_none_or(|name| name.is_empty()) {
            continue;
        }
        root.symbols.push(struct_symbol(&item));
    }
    for item in file.enums() {
        if item.name().is_none_or(|name| name.is_empty()) {
            continue;
        }
        root.symbols.push(enum_symbol(&item));
    }
    for item in file.errors() {
        if item.name().is_none_or(|name| name.is_empty()) {
            continue;
        }
        root.symbols.push(error_symbol(&item));
    }
    for item in file.user_types() {
        if item.name().is_none_or(|name| name.is_empty()) {
            continue;
        }
        root.symbols.push(user_type_symbol(&item));
    }
    for item in file.events() {
        if item.name().is_none_or(|name| name.is_empty()) {
            continue;
        }
        root.symbols.push(event_symbol(&item));
    }
    for contract in file.contracts() {
        if contract.name().is_none_or(|name| name.is_empty()) {
            continue;
        }
        root.symbols.push(contract_symbol(&contract));
        root.children.push(contract_scope(file, &contract));
    }
    root
}

fn contract_symbol(contract: &ContractDef) -> Symbol {
    let name = contract.name().unwrap_or_default();
    let mut detail = String::from(contract.kind_label());
    detail.push(' ');
    detail.push_str(&name);
    if !contract.heritage().is_empty() {
        detail.push_str(" is ");
        detail.push_str(&contract.heritage().join(", "));
    }
    Symbol::new(name, SymbolKind::Contract, contract.syntax().span, detail)
}

fn struct_symbol(item: &StructDef) -> Symbol {
    let name = item.name().unwrap_or_default();
    let mut detail = String::from("struct ");
    detail.push_str(&name);
    Symbol::new(name, SymbolKind::Struct, item.syntax().span, detail)
}

fn enum_symbol(item: &EnumDef) -> Symbol {
    let name = item.name().unwrap_or_default();
    let mut detail = String::from("enum ");
    detail.push_str(&name);
    Symbol::new(name, SymbolKind::Enum, item.syntax().span, detail)
}

fn error_symbol(item: &crate::ErrorDef) -> Symbol {
    let name = item.name().unwrap_or_default();
    let mut detail = String::from("error ");
    detail.push_str(&name);
    Symbol::new(name, SymbolKind::Error, item.syntax().span, detail)
}

fn user_type_symbol(item: &crate::UserTypeDef) -> Symbol {
    let name = item.name().unwrap_or_default();
    let mut detail = String::from("type ");
    detail.push_str(&name);
    if let Some(underlying) = item.underlying() {
        detail.push_str(" is ");
        detail.push_str(&underlying.text());
    }
    Symbol::new(name, SymbolKind::UserType, item.syntax().span, detail)
}

fn event_symbol(item: &crate::EventDef) -> Symbol {
    let name = item.name().unwrap_or_default();
    let mut detail = String::from("event ");
    detail.push_str(&name);
    Symbol::new(name, SymbolKind::Event, item.syntax().span, detail)
}

fn modifier_symbol(item: &crate::ModifierDef) -> Symbol {
    let name = item.name().unwrap_or_default();
    let mut detail = String::from("modifier ");
    detail.push_str(&name);
    Symbol::new(name, SymbolKind::Modifier, item.syntax().span, detail)
}

fn contract_scope(file: &SolidityFile, contract: &ContractDef) -> Scope {
    let mut scope = Scope::new(contract.syntax().span);
    for item in contract.structs() {
        if let Some(name) = item.name() {
            if name.is_empty() {
                continue;
            }
            scope.symbols.push(struct_symbol(&item));
        }
    }
    for item in contract.enums() {
        if let Some(name) = item.name() {
            if name.is_empty() {
                continue;
            }
            scope.symbols.push(enum_symbol(&item));
        }
    }
    for item in contract.errors() {
        if let Some(name) = item.name() {
            if name.is_empty() {
                continue;
            }
            scope.symbols.push(error_symbol(&item));
        }
    }
    for item in contract.user_types() {
        if let Some(name) = item.name() {
            if name.is_empty() {
                continue;
            }
            scope.symbols.push(user_type_symbol(&item));
        }
    }
    for item in contract.events() {
        if let Some(name) = item.name() {
            if name.is_empty() {
                continue;
            }
            scope.symbols.push(event_symbol(&item));
        }
    }
    for item in contract.modifiers() {
        if let Some(name) = item.name() {
            if name.is_empty() {
                continue;
            }
            scope.symbols.push(modifier_symbol(&item));
        }
    }
    for var in contract.state_vars() {
        if let Some(name) = var.name() {
            let mut detail = var.type_node().map(|node| node.text()).unwrap_or_default();
            detail.push(' ');
            detail.push_str(&name);
            scope.symbols.push(Symbol::new(
                name,
                SymbolKind::StateVar,
                var.syntax().span,
                detail,
            ));
        }
    }
    for function in contract.functions() {
        if function.name().is_none_or(|name| name.is_empty()) {
            continue;
        }
        scope.symbols.push(function_symbol(&function));
        scope.children.push(function_scope(&function));
    }
    for inherited in inheritance::inherited_symbols(file, contract) {
        if scope.symbols.iter().any(|own| own.name == inherited.name) {
            continue;
        }
        scope.symbols.push(inherited);
    }
    scope
}

fn function_symbol(function: &FunctionDef) -> Symbol {
    let name = function.name().unwrap_or_default();
    let params: Vec<String> = function
        .params()
        .iter()
        .map(|param| {
            let mut text = param
                .type_node()
                .map(|node| node.text())
                .unwrap_or_default();
            if let Some(param_name) = param.name() {
                text.push(' ');
                text.push_str(&param_name);
            }
            text
        })
        .collect();
    let mut detail = format!("function {name}({})", params.join(", "));
    let returns: Vec<String> = function
        .return_types()
        .iter()
        .map(|node| node.text())
        .collect();
    if !returns.is_empty() {
        detail.push_str(" returns (");
        detail.push_str(&returns.join(", "));
        detail.push(')');
    }
    Symbol::new(name, SymbolKind::Function, function.syntax().span, detail)
}

fn function_scope(function: &FunctionDef) -> Scope {
    let mut scope = Scope::new(function.syntax().span);
    for param in function.params() {
        if let Some(name) = param.name() {
            let mut detail = param
                .type_node()
                .map(|node| node.text())
                .unwrap_or_default();
            detail.push(' ');
            detail.push_str(&name);
            scope.symbols.push(Symbol::new(
                name,
                SymbolKind::Param,
                param.syntax().span,
                detail,
            ));
        }
    }
    if let Some(body) = function.body() {
        for local in body.locals() {
            if let Some(name) = local.name() {
                let mut detail = local
                    .type_node()
                    .map(|node| node.text())
                    .unwrap_or_default();
                detail.push(' ');
                detail.push_str(&name);
                scope.symbols.push(Symbol::new(
                    name,
                    SymbolKind::LocalVar,
                    local.syntax().span,
                    detail,
                ));
            }
        }
    }
    scope
}

/// Solidity analyzer for the core registry.
///
/// Parses on each call: simple and correct for beta, no incremental cache.
#[derive(Debug, Default)]
pub struct SolidityAnalyzer;

impl SolidityAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Parse with recovery and keep the file even when errors exist.
    ///
    /// Completion, hover, and goto run while the user types, so errors
    /// are the norm. Diagnostics still report them separately.
    fn parse_file(text: &str) -> SolidityFile {
        let tokens = SolidityLexer.tokenize(text);
        let (file, _) = SolidityParser.parse_with_errors(&tokens);
        file
    }

    fn dep_files(ctx: &ProjectContext) -> Vec<(std::path::PathBuf, SolidityFile)> {
        let mut out = Vec::new();
        for (path, text) in forgels_core::dep_texts(ctx, 16) {
            out.push((path.clone(), Self::parse_file(&text)));
            if out.len() >= 16 {
                break;
            }
        }
        out
    }

    fn member_completion(
        file: &SolidityFile,
        ctx_text: &str,
        offset: u32,
        member: &members::MemberContext,
        deps: &[(std::path::PathBuf, SolidityFile)],
    ) -> Vec<CompletionData> {
        let scope = collect_symbols(file);
        let kind = members::receiver_type_with_deps(file, &scope, offset, &member.receiver, deps);
        let mut seen: Vec<String> = Vec::new();
        let mut out: Vec<CompletionData> = Vec::new();
        for builtin in forge::member_builtin_list(&member.receiver, &member.member, &scope, offset)
        {
            if seen.contains(&builtin.name.to_string()) {
                continue;
            }
            seen.push(builtin.name.to_string());
            out.push(CompletionData {
                label: builtin.name.to_string(),
                detail: forge::builtin_detail(builtin),
                kind: completion::completion_item_kind(completion::CompletionKind::Symbol(
                    SymbolKind::Function,
                )),
                text_edit: None,
            });
        }
        for item in members::members_of(file, &kind) {
            if !item.name.starts_with(member.member.as_str()) {
                continue;
            }
            if seen.contains(&item.name) {
                continue;
            }
            seen.push(item.name.clone());
            out.push(CompletionData {
                label: item.name.clone(),
                detail: item.detail.clone(),
                kind: completion::completion_item_kind(completion::CompletionKind::Symbol(
                    item.kind,
                )),
                text_edit: None,
            });
        }
        for (info, _) in members::collect_cross_file_members(file, &kind, deps) {
            if !info.name.starts_with(member.member.as_str()) {
                continue;
            }
            if seen.contains(&info.name) {
                continue;
            }
            seen.push(info.name.clone());
            out.push(CompletionData {
                label: info.name.clone(),
                detail: info.detail.clone(),
                kind: completion::completion_item_kind(completion::CompletionKind::Symbol(
                    info.kind,
                )),
                text_edit: None,
            });
        }
        out.sort_by(|left, right| left.label.cmp(&right.label));
        let _ = ctx_text;
        out
    }

    fn emit_completion(file: &SolidityFile, ctx: &ProjectContext) -> Vec<CompletionData> {
        let prefix = completion::prefix_at(ctx.text, ctx.offset);
        let local = emits::emit_candidates(file, ctx.offset);
        let mut seen: Vec<String> = Vec::new();
        let mut out: Vec<CompletionData> = Vec::new();
        for symbol in &local {
            if !symbol.name.starts_with(prefix.as_str()) {
                continue;
            }
            if seen.contains(&symbol.name) {
                continue;
            }
            seen.push(symbol.name.clone());
            out.push(CompletionData {
                label: symbol.name.clone(),
                detail: symbol.detail.clone(),
                kind: completion::completion_item_kind(completion::CompletionKind::Symbol(
                    symbol.kind,
                )),
                text_edit: None,
            });
        }
        let deps = Self::dep_files(ctx);
        for (symbol, _) in emits::dep_emit_candidates(&local, &deps) {
            if !symbol.name.starts_with(prefix.as_str()) {
                continue;
            }
            if seen.contains(&symbol.name) {
                continue;
            }
            seen.push(symbol.name.clone());
            out.push(CompletionData {
                label: symbol.name.clone(),
                detail: symbol.detail.clone(),
                kind: completion::completion_item_kind(completion::CompletionKind::Symbol(
                    symbol.kind,
                )),
                text_edit: None,
            });
        }
        out.sort_by(|left, right| left.label.cmp(&right.label));
        out
    }

    fn diagnostics_for(
        file: &SolidityFile,
        text: &str,
        errors: &[forgels_syntax::ParseError],
        scope: &Scope,
        deps: &[(std::path::PathBuf, SolidityFile)],
    ) -> Vec<DiagnosticData> {
        let mut out: Vec<DiagnosticData> = errors
            .iter()
            .map(|error| DiagnosticData {
                span: error.span,
                message: error.message.clone(),
            })
            .collect();
        out.extend(calls::arity_diagnostics(file, scope));
        out.extend(calls::member_arity_diagnostics(file, scope, deps));
        out.extend(forge::assert_arity_diagnostics(file, scope));
        out.extend(emits::emit_arity_diagnostics(file));
        out.extend(emits::unknown_emit_diagnostics(file, deps));
        out.extend(unknown::unknown_identifier_diagnostics(
            file, text, scope, deps,
        ));
        out.sort_by(|left, right| {
            left.span
                .start
                .cmp(&right.span.start)
                .then(left.span.end.cmp(&right.span.end))
                .then(left.message.cmp(&right.message))
        });
        out
    }
}

impl Analyzer for SolidityAnalyzer {
    fn hover(&self, ctx: &ProjectContext) -> Option<HoverData> {
        let file = Self::parse_file(ctx.text);
        if let Some(found) = hover::hover_at(&file, ctx.text, ctx.offset) {
            return Some(HoverData {
                contents: found.contents,
                span: found.span,
            });
        }
        if let Some((name, span)) = emits::emit_name_at(ctx.text, ctx.offset) {
            let deps = Self::dep_files(ctx);
            for (path, dep_file) in &deps {
                let Some(found) = emits::find_emit_in_dep(dep_file, &name) else {
                    continue;
                };
                let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                    continue;
                };
                return Some(HoverData {
                    contents: format!("{} - from {}", found.detail, file_name),
                    span,
                });
            }
            return None;
        }
        if let Some(member) = members::member_context_at(ctx.text, ctx.offset)
            && (member.member_span.contains(ctx.offset) || member.member_span.end == ctx.offset)
        {
            let deps = Self::dep_files(ctx);
            let scope = collect_symbols(&file);
            let kind = members::receiver_type_with_deps(
                &file,
                &scope,
                ctx.offset,
                &member.receiver,
                &deps,
            );
            if let Some((info, _)) =
                members::resolve_member_cross_file(&file, &kind, &member.member, &deps)
            {
                return Some(HoverData {
                    contents: info.detail,
                    span: member.member_span,
                });
            }
        }
        let (word, span) = word_at(ctx.text, ctx.offset)?;
        for (dep_path, dep_text) in forgels_core::dep_texts(ctx, 16) {
            let Some(file_name) = dep_path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            for item in completion::exports_of(&dep_text) {
                if item.label == word {
                    return Some(HoverData {
                        contents: format!("{} - from {}", item.detail, file_name),
                        span,
                    });
                }
            }
        }
        None
    }

    fn complete(&self, ctx: &ProjectContext) -> Vec<CompletionData> {
        let tokens = SolidityLexer.tokenize(ctx.text);
        let (recovered, _) = SolidityParser.parse_with_errors(&tokens);
        if let Some(outer) = completion::import_path_at(&recovered, ctx.text, ctx.offset) {
            let prefix = completion::path_prefix_at(ctx.text, ctx.offset);
            let mut items =
                completion::complete_import_paths(ctx.path, ctx.workspace.roots(), &prefix);
            if !prefix.starts_with('.') {
                for bare in
                    completion::complete_bare_imports(ctx.path, ctx.workspace.roots(), &prefix)
                {
                    if items.iter().any(|known| known.label == bare.label) {
                        continue;
                    }
                    items.push(bare);
                }
                items.sort_by(|left, right| left.label.cmp(&right.label));
            }
            let inner = completion::import_inner_span(ctx.text, outer);
            return items
                .into_iter()
                .map(|item| {
                    let edit = inner.map(|span| TextEdit::new(span, item.label.clone()));
                    CompletionData {
                        label: item.label.clone(),
                        detail: item.detail.clone(),
                        kind: completion::completion_item_kind(item.kind),
                        text_edit: edit,
                    }
                })
                .collect();
        }
        let file = Self::parse_file(ctx.text);
        if let Some(member) = members::member_context_at(ctx.text, ctx.offset) {
            let deps = Self::dep_files(ctx);
            return Self::member_completion(&file, ctx.text, ctx.offset, &member, &deps);
        }
        if let Some(type_ctx) = meta::type_context_at(ctx.text, ctx.offset) {
            let kind = members::TypeKind::Meta(type_ctx.inner.clone());
            let mut out: Vec<CompletionData> = members::members_of(&file, &kind)
                .into_iter()
                .filter(|item| item.name.starts_with(type_ctx.member.as_str()))
                .map(|item| CompletionData {
                    label: item.name.clone(),
                    detail: item.detail.clone(),
                    kind: completion::completion_item_kind(completion::CompletionKind::Symbol(
                        item.kind,
                    )),
                    text_edit: None,
                })
                .collect();
            out.sort_by(|left, right| left.label.cmp(&right.label));
            return out;
        }
        if emits::is_emit_name_context(ctx.text, ctx.offset) {
            return Self::emit_completion(&file, ctx);
        }
        let prefix = completion::prefix_at(ctx.text, ctx.offset);
        let mut seen: Vec<String> = Vec::new();
        let mut out: Vec<CompletionData> = Vec::new();
        for item in completion::complete_at(&file, ctx.text, ctx.offset) {
            if seen.contains(&item.label) {
                continue;
            }
            seen.push(item.label.clone());
            out.push(CompletionData {
                label: item.label,
                detail: item.detail,
                kind: completion::completion_item_kind(item.kind),
                text_edit: None,
            });
        }
        for (dep_path, dep_text) in forgels_core::dep_texts(ctx, 16) {
            let Some(file_name) = dep_path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let origin = file_name.to_string();
            for item in completion::exports_of(&dep_text) {
                if !item.label.starts_with(prefix.as_str()) {
                    continue;
                }
                if seen.contains(&item.label) {
                    continue;
                }
                seen.push(item.label.clone());
                out.push(CompletionData {
                    label: item.label.clone(),
                    detail: format!("{} - from {}", item.detail, origin),
                    kind: completion::completion_item_kind(item.kind),
                    text_edit: None,
                });
            }
        }
        out
    }

    fn goto(&self, ctx: &ProjectContext) -> Option<Span> {
        // Single file only; cross file lookup uses goto_location.
        let file = Self::parse_file(ctx.text);
        goto::goto_definition(&file, ctx.text, ctx.offset)
    }

    fn goto_location(&self, ctx: &ProjectContext) -> Option<GotoTarget> {
        let file = Self::parse_file(ctx.text);
        if let Some(span) = goto::goto_definition(&file, ctx.text, ctx.offset) {
            return Some(GotoTarget::new(ctx.path.to_path_buf(), span));
        }
        // Builtins have no target by design. Skip dep lookup so builtin
        // names never jump to coincidental same-name declarations.
        // Array, address, bytes, string, function, global, and type()
        // members use the same guard: synthetic spans never leak.
        {
            let scope = collect_symbols(&file);
            if forge::is_builtin_at(&file, ctx.text, ctx.offset, &scope) {
                return None;
            }
            if array::is_array_at(&file, ctx.text, ctx.offset, &scope) {
                return None;
            }
            if address::is_address_at(&file, ctx.text, ctx.offset, &scope) {
                return None;
            }
            if bytes::is_bytes_at(&file, ctx.text, ctx.offset, &scope) {
                return None;
            }
            if bytes::is_string_at(&file, ctx.text, ctx.offset, &scope) {
                return None;
            }
            if function::is_function_at(&file, ctx.text, ctx.offset, &scope) {
                return None;
            }
            if globals::is_global_at(&file, ctx.text, ctx.offset, &scope) {
                return None;
            }
            if meta::is_meta_at(ctx.text, ctx.offset) {
                return None;
            }
        }
        if let Some((name, _)) = emits::emit_name_at(ctx.text, ctx.offset) {
            let deps = Self::dep_files(ctx);
            for (path, dep_file) in &deps {
                if let Some(found) = emits::find_emit_in_dep(dep_file, &name) {
                    return Some(GotoTarget::new(path.clone(), found.span));
                }
            }
            return None;
        }
        if let Some(member) = members::member_context_at(ctx.text, ctx.offset)
            && (member.member_span.contains(ctx.offset) || member.member_span.end == ctx.offset)
        {
            let deps = Self::dep_files(ctx);
            let scope = collect_symbols(&file);
            let kind = members::receiver_type_with_deps(
                &file,
                &scope,
                ctx.offset,
                &member.receiver,
                &deps,
            );
            if let Some((info, path)) =
                members::resolve_member_cross_file(&file, &kind, &member.member, &deps)
            {
                return Some(GotoTarget::new(path, info.span));
            }
        }
        let (word, _) = word_at(ctx.text, ctx.offset)?;
        for (dep_path, dep_text) in forgels_core::dep_texts(ctx, 16) {
            if let Some(span) = goto::resolve_in_dep(&dep_text, &word) {
                return Some(GotoTarget::new(dep_path, span));
            }
        }
        None
    }

    fn diagnostics(&self, text: &str) -> Vec<DiagnosticData> {
        let tokens = SolidityLexer.tokenize(text);
        let (file, errors) = SolidityParser.parse_with_errors(&tokens);
        let scope = collect_symbols(&file);
        Self::diagnostics_for(&file, text, &errors, &scope, &[])
    }

    fn diagnostics_ctx(&self, ctx: &ProjectContext) -> Vec<DiagnosticData> {
        let tokens = SolidityLexer.tokenize(ctx.text);
        let (file, errors) = SolidityParser.parse_with_errors(&tokens);
        let scope = collect_symbols(&file);
        let deps = Self::dep_files(ctx);
        Self::diagnostics_for(&file, ctx.text, &errors, &scope, &deps)
    }

    fn imports(&self, text: &str) -> Vec<ImportRef> {
        imports::imports_in(text)
    }

    fn signature_help(&self, ctx: &ProjectContext) -> Option<Vec<forgels_core::SignatureData>> {
        let file = Self::parse_file(ctx.text);
        let deps = Self::dep_files(ctx);
        signature::signature_at(&file, ctx.text, ctx.offset, &deps)
    }

    fn document_symbols(&self, text: &str) -> Vec<forgels_core::DocumentSymbolData> {
        let file = Self::parse_file(text);
        symbols::document_symbols_in(&file)
    }

    fn references(&self, ctx: &ProjectContext) -> Option<Vec<forgels_core::ReferenceData>> {
        let file = Self::parse_file(ctx.text);
        let deps = Self::dep_files(ctx);
        references::find_references(&file, ctx.text, ctx.offset, ctx.path, &deps, ctx)
    }

    fn rename(
        &self,
        ctx: &ProjectContext,
        new_name: &str,
    ) -> Option<Vec<forgels_core::ReferenceData>> {
        let file = Self::parse_file(ctx.text);
        let deps = Self::dep_files(ctx);
        rename::find_rename(&file, ctx.text, ctx.offset, new_name, ctx.path, &deps, ctx)
    }
}
