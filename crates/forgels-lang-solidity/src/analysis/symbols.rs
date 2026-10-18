use forgels_core::DocumentSymbolData;
use forgels_protocol::SymbolKind;
use forgels_syntax::{AstNode, Span, SyntaxElement};

use crate::{
    ContractDef, EnumDef, ErrorDef, EventDef, FunctionDef, ModifierDef, Param, SolidityFile,
    SolidityNode, StateVar, StructDef, UserTypeDef,
};

fn selection_of(name_span: Option<Span>, full: Span) -> Span {
    name_span.unwrap_or(full)
}

fn named(name: Option<String>) -> Option<String> {
    match name {
        Some(text) if !text.is_empty() => Some(text),
        _ => None,
    }
}

fn param_detail(param: &Param, name: &str) -> String {
    match param.type_node() {
        Some(node) => format!("{} {name}", node.text()),
        None => name.to_string(),
    }
}

/// Outline entries for the whole file in source order.
///
/// Recovery output is used as is: nameless nodes are skipped so broken
/// rows never surface, while valid siblings still outline.
pub fn document_symbols_in(file: &SolidityFile) -> Vec<DocumentSymbolData> {
    let mut out = Vec::new();
    for element in &file.syntax().children {
        let SyntaxElement::Node(node) = element else {
            continue;
        };
        if node.kind == SolidityNode::Contract.as_u16() {
            let Some(contract) = ContractDef::cast(node.clone()) else {
                continue;
            };
            if let Some(symbol) = contract_symbol(&contract) {
                out.push(symbol);
            }
        } else if node.kind == SolidityNode::Struct.as_u16() {
            let Some(item) = StructDef::cast(node.clone()) else {
                continue;
            };
            if let Some(symbol) = struct_symbol(&item) {
                out.push(symbol);
            }
        } else if node.kind == SolidityNode::Enum.as_u16() {
            let Some(item) = EnumDef::cast(node.clone()) else {
                continue;
            };
            if let Some(symbol) = enum_symbol(&item) {
                out.push(symbol);
            }
        } else if node.kind == SolidityNode::Error.as_u16() {
            let Some(item) = ErrorDef::cast(node.clone()) else {
                continue;
            };
            if let Some(symbol) = error_symbol(&item) {
                out.push(symbol);
            }
        } else if node.kind == SolidityNode::UserType.as_u16() {
            let Some(item) = UserTypeDef::cast(node.clone()) else {
                continue;
            };
            if let Some(symbol) = user_type_symbol(&item) {
                out.push(symbol);
            }
        } else if node.kind == SolidityNode::Event.as_u16() {
            let Some(item) = EventDef::cast(node.clone()) else {
                continue;
            };
            if let Some(symbol) = event_symbol(&item) {
                out.push(symbol);
            }
        }
    }
    out
}

fn contract_symbol(contract: &ContractDef) -> Option<DocumentSymbolData> {
    let name = named(contract.name())?;
    let full = contract.syntax().span;
    let mut detail = String::from(contract.kind_label());
    detail.push(' ');
    detail.push_str(&name);
    if !contract.heritage().is_empty() {
        detail.push_str(" is ");
        detail.push_str(&contract.heritage().join(", "));
    }
    Some(DocumentSymbolData::new(
        name,
        detail,
        SymbolKind::CLASS,
        full,
        selection_of(contract.name_span(), full),
        contract_children(contract),
    ))
}

fn contract_children(contract: &ContractDef) -> Vec<DocumentSymbolData> {
    let mut out = Vec::new();
    for element in &contract.syntax().children {
        let SyntaxElement::Node(node) = element else {
            continue;
        };
        if node.kind == SolidityNode::Function.as_u16() {
            let Some(function) = FunctionDef::cast(node.clone()) else {
                continue;
            };
            if let Some(symbol) = function_symbol(&function) {
                out.push(symbol);
            }
        } else if node.kind == SolidityNode::StateVar.as_u16() {
            let Some(var) = StateVar::cast(node.clone()) else {
                continue;
            };
            if let Some(symbol) = state_var_symbol(&var) {
                out.push(symbol);
            }
        } else if node.kind == SolidityNode::Struct.as_u16() {
            let Some(item) = StructDef::cast(node.clone()) else {
                continue;
            };
            if let Some(symbol) = struct_symbol(&item) {
                out.push(symbol);
            }
        } else if node.kind == SolidityNode::Enum.as_u16() {
            let Some(item) = EnumDef::cast(node.clone()) else {
                continue;
            };
            if let Some(symbol) = enum_symbol(&item) {
                out.push(symbol);
            }
        } else if node.kind == SolidityNode::Error.as_u16() {
            let Some(item) = ErrorDef::cast(node.clone()) else {
                continue;
            };
            if let Some(symbol) = error_symbol(&item) {
                out.push(symbol);
            }
        } else if node.kind == SolidityNode::UserType.as_u16() {
            let Some(item) = UserTypeDef::cast(node.clone()) else {
                continue;
            };
            if let Some(symbol) = user_type_symbol(&item) {
                out.push(symbol);
            }
        } else if node.kind == SolidityNode::Event.as_u16() {
            let Some(item) = EventDef::cast(node.clone()) else {
                continue;
            };
            if let Some(symbol) = event_symbol(&item) {
                out.push(symbol);
            }
        } else if node.kind == SolidityNode::Modifier.as_u16() {
            let Some(item) = ModifierDef::cast(node.clone()) else {
                continue;
            };
            if let Some(symbol) = modifier_symbol(&item) {
                out.push(symbol);
            }
        }
    }
    out
}

fn function_symbol(function: &FunctionDef) -> Option<DocumentSymbolData> {
    let name = named(function.name())?;
    let full = function.syntax().span;
    let params: Vec<String> = function
        .params()
        .iter()
        .map(|param| {
            let mut text = param
                .type_node()
                .map(|node| node.text())
                .unwrap_or_default();
            if let Some(param_name) = param.name()
                && !param_name.is_empty()
            {
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
    Some(DocumentSymbolData::new(
        name,
        detail,
        SymbolKind::METHOD,
        full,
        selection_of(function.name_span(), full),
        function_children(function),
    ))
}

fn function_children(function: &FunctionDef) -> Vec<DocumentSymbolData> {
    // Outline keeps params only by design: locals (including nested
    // block and tuple decls) resolve through scopes for hover, goto,
    // and completion, but never outline. V1 keeps the function shape
    // to its signature.
    let mut out = Vec::new();
    for param in function.params() {
        if let Some(symbol) = param_symbol(&param) {
            out.push(symbol);
        }
    }
    out
}

fn param_symbol(param: &Param) -> Option<DocumentSymbolData> {
    let name = named(param.name())?;
    let full = param.syntax().span;
    Some(DocumentSymbolData::new(
        name.clone(),
        param_detail(param, &name),
        SymbolKind::VARIABLE,
        full,
        selection_of(param.name_span(), full),
        Vec::new(),
    ))
}

fn state_var_symbol(var: &StateVar) -> Option<DocumentSymbolData> {
    let name = named(var.name())?;
    let full = var.syntax().span;
    let mut detail = var.type_node().map(|node| node.text()).unwrap_or_default();
    detail.push(' ');
    detail.push_str(&name);
    Some(DocumentSymbolData::new(
        name,
        detail,
        SymbolKind::FIELD,
        full,
        selection_of(var.name_span(), full),
        Vec::new(),
    ))
}

fn struct_symbol(item: &StructDef) -> Option<DocumentSymbolData> {
    let name = named(item.name())?;
    let full = item.syntax().span;
    let mut children = Vec::new();
    for field in item.fields() {
        if let Some(symbol) = struct_field_symbol(&field) {
            children.push(symbol);
        }
    }
    Some(DocumentSymbolData::new(
        name.clone(),
        format!("struct {name}"),
        SymbolKind::STRUCT,
        full,
        selection_of(item.name_span(), full),
        children,
    ))
}

fn struct_field_symbol(field: &Param) -> Option<DocumentSymbolData> {
    let name = named(field.name())?;
    let full = field.syntax().span;
    Some(DocumentSymbolData::new(
        name.clone(),
        param_detail(field, &name),
        SymbolKind::FIELD,
        full,
        selection_of(field.name_span(), full),
        Vec::new(),
    ))
}

fn enum_symbol(item: &EnumDef) -> Option<DocumentSymbolData> {
    let name = named(item.name())?;
    let full = item.syntax().span;
    let children = item
        .variants()
        .iter()
        .zip(item.variant_spans().iter())
        .filter_map(|(variant, span)| {
            if variant.is_empty() {
                return None;
            }
            Some(DocumentSymbolData::new(
                variant.clone(),
                String::new(),
                SymbolKind::ENUM_MEMBER,
                *span,
                *span,
                Vec::new(),
            ))
        })
        .collect();
    Some(DocumentSymbolData::new(
        name.clone(),
        format!("enum {name}"),
        SymbolKind::ENUM,
        full,
        selection_of(item.name_span(), full),
        children,
    ))
}

fn error_symbol(item: &ErrorDef) -> Option<DocumentSymbolData> {
    let name = named(item.name())?;
    let full = item.syntax().span;
    Some(DocumentSymbolData::new(
        name.clone(),
        format!("error {name}"),
        SymbolKind::VARIABLE,
        full,
        selection_of(item.name_span(), full),
        Vec::new(),
    ))
}

fn user_type_symbol(item: &UserTypeDef) -> Option<DocumentSymbolData> {
    let name = named(item.name())?;
    let full = item.syntax().span;
    let mut detail = format!("type {name}");
    if let Some(underlying) = item.underlying() {
        detail.push_str(" is ");
        detail.push_str(&underlying.text());
    }
    Some(DocumentSymbolData::new(
        name,
        detail,
        SymbolKind::TYPE_PARAMETER,
        full,
        selection_of(item.name_span(), full),
        Vec::new(),
    ))
}

fn event_symbol(item: &EventDef) -> Option<DocumentSymbolData> {
    let name = named(item.name())?;
    let full = item.syntax().span;
    let mut children = Vec::new();
    for param in item.params() {
        if let Some(symbol) = param_symbol(&param) {
            children.push(symbol);
        }
    }
    Some(DocumentSymbolData::new(
        name.clone(),
        format!("event {name}"),
        SymbolKind::EVENT,
        full,
        selection_of(item.name_span(), full),
        children,
    ))
}

fn modifier_symbol(item: &ModifierDef) -> Option<DocumentSymbolData> {
    let name = named(item.name())?;
    let full = item.syntax().span;
    Some(DocumentSymbolData::new(
        name.clone(),
        format!("modifier {name}"),
        SymbolKind::METHOD,
        full,
        selection_of(item.name_span(), full),
        Vec::new(),
    ))
}
