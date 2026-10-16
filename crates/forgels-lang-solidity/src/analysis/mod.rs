use forgels_core::{
    Analyzer, CompletionData, DiagnosticData, GotoTarget, HoverData, ProjectContext,
    analyzer::ImportRef,
};
use forgels_syntax::{AstNode, Lexer, Span};

use crate::{ContractDef, FunctionDef, SolidityFile, SolidityLexer, SolidityParser};

mod completion;
mod goto;
mod hover;
mod imports;

#[cfg(test)]
mod tests;

pub use completion::{CompletionItem, CompletionKind, complete_at, exports_of};
pub use goto::{goto_definition, resolve_in_dep};
pub use hover::{Hover, hover_at};
pub use imports::imports_in;

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

/// Word under the cursor: longest ident-char run containing `offset`.
fn word_at(text: &str, offset: u32) -> Option<(String, Span)> {
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
    StateVar,
    LocalVar,
    Param,
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
    /// shadow params and params shadow state variables.
    pub fn resolve(&self, name: &str, offset: u32) -> Option<&Symbol> {
        for child in &self.children {
            if child.span.contains(offset)
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
            if child.span.contains(offset) {
                child.collect_visible(offset, out);
            }
        }
    }

    /// Contract level symbols: contracts plus their functions.
    ///
    /// Used for cross file completion. State vars, params, and locals
    /// stay local and never cross files.
    pub fn export_symbols(&self) -> Vec<&Symbol> {
        let mut out: Vec<&Symbol> = Vec::new();
        for symbol in &self.symbols {
            if matches!(symbol.kind, SymbolKind::Contract | SymbolKind::Function) {
                out.push(symbol);
            }
        }
        for child in &self.children {
            for symbol in &child.symbols {
                if matches!(symbol.kind, SymbolKind::Contract | SymbolKind::Function) {
                    out.push(symbol);
                }
            }
        }
        out
    }
}

/// Collect every declaration in the file into nested scopes.
pub fn collect_symbols(file: &SolidityFile) -> Scope {
    let mut root = Scope::new(file.syntax().span);
    for contract in file.contracts() {
        root.symbols.push(contract_symbol(&contract));
        root.children.push(contract_scope(&contract));
    }
    root
}

fn contract_symbol(contract: &ContractDef) -> Symbol {
    let name = contract.name().unwrap_or_default();
    let mut detail = String::from("contract ");
    detail.push_str(&name);
    if !contract.heritage().is_empty() {
        detail.push_str(" is ");
        detail.push_str(&contract.heritage().join(", "));
    }
    Symbol::new(name, SymbolKind::Contract, contract.syntax().span, detail)
}

fn contract_scope(contract: &ContractDef) -> Scope {
    let mut scope = Scope::new(contract.syntax().span);
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
        scope.symbols.push(function_symbol(&function));
        scope.children.push(function_scope(&function));
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

    fn parse_file(text: &str) -> Option<SolidityFile> {
        let tokens = SolidityLexer.tokenize(text);
        let (file, errors) = SolidityParser.parse_with_errors(&tokens);
        if errors.is_empty() { Some(file) } else { None }
    }
}

impl Analyzer for SolidityAnalyzer {
    fn hover(&self, ctx: &ProjectContext) -> Option<HoverData> {
        let local = Self::parse_file(ctx.text)
            .and_then(|file| hover::hover_at(&file, ctx.text, ctx.offset));
        if let Some(found) = local {
            return Some(HoverData {
                contents: found.contents,
                span: found.span,
            });
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
        let prefix = completion::prefix_at(ctx.text, ctx.offset);
        let mut seen: Vec<String> = Vec::new();
        let mut out: Vec<CompletionData> = Vec::new();
        if let Some(file) = Self::parse_file(ctx.text) {
            for item in completion::complete_at(&file, ctx.text, ctx.offset) {
                if seen.contains(&item.label) {
                    continue;
                }
                seen.push(item.label.clone());
                out.push(CompletionData {
                    label: item.label,
                    detail: item.detail,
                });
            }
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
                });
            }
        }
        out
    }

    fn goto(&self, ctx: &ProjectContext) -> Option<Span> {
        // Single file only; cross file lookup uses goto_location.
        let file = Self::parse_file(ctx.text)?;
        goto::goto_definition(&file, ctx.text, ctx.offset)
    }

    fn goto_location(&self, ctx: &ProjectContext) -> Option<GotoTarget> {
        if let Some(file) = Self::parse_file(ctx.text)
            && let Some(span) = goto::goto_definition(&file, ctx.text, ctx.offset)
        {
            return Some(GotoTarget::new(ctx.path.to_path_buf(), span));
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
        let (_, errors) = SolidityParser.parse_with_errors(&tokens);
        errors
            .iter()
            .map(|error| DiagnosticData {
                span: error.span,
                message: error.message.clone(),
            })
            .collect()
    }

    fn imports(&self, text: &str) -> Vec<ImportRef> {
        imports::imports_in(text)
    }
}
