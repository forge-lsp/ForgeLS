use forgels_core::{Analyzer, CompletionData, DiagnosticData, HoverData};
use forgels_syntax::{AstNode, Lexer, Span};

use crate::{ContractDef, FunctionDef, SolidityFile, SolidityLexer, SolidityParser};

mod completion;
mod goto;
mod hover;

#[cfg(test)]
mod tests;

pub use completion::{CompletionItem, CompletionKind, complete_at};
pub use goto::goto_definition;
pub use hover::{Hover, hover_at};

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
    fn hover(&self, text: &str, offset: u32) -> Option<HoverData> {
        let file = Self::parse_file(text)?;
        hover::hover_at(&file, text, offset).map(|found| HoverData {
            contents: found.contents,
            span: found.span,
        })
    }

    fn complete(&self, text: &str, offset: u32) -> Vec<CompletionData> {
        let Some(file) = Self::parse_file(text) else {
            return Vec::new();
        };
        completion::complete_at(&file, text, offset)
            .iter()
            .map(|item| CompletionData {
                label: item.label.clone(),
                detail: item.detail.clone(),
            })
            .collect()
    }

    fn goto(&self, text: &str, offset: u32) -> Option<Span> {
        let file = Self::parse_file(text)?;
        goto::goto_definition(&file, text, offset)
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
}
