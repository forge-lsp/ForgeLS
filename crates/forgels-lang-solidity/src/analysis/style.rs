use forgels_core::DiagnosticData;
use forgels_lint::RuleId;
use forgels_syntax::{AstNode, Lexer, Span, SyntaxElement};

use crate::{
    Mutability, SolidityFile, SolidityLexer, SolidityNode, Visibility, lexer::SolidityKind,
};

// Style batch 1 plus batch 2: warn-only naming, ordering, glue,
// blank lines, natspec, and line length checks.
// Every push is a warning; config overrides via the lint runner.
pub fn style_diagnostics(file: &SolidityFile, text: &str) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    out.extend(naming_diagnostics(file));
    out.extend(modifier_order_diagnostics(file));
    out.extend(function_order_diagnostics(file));
    out.extend(layout_order_diagnostics(file));
    out.extend(import_position_diagnostics(file));
    out.extend(mapping_glue_diagnostics(text));
    out.extend(array_glue_diagnostics(text));
    out.extend(blank_lines_diagnostics(file, text));
    out.extend(natspec_missing_diagnostics(file, text));
    out.extend(natspec_param_diagnostics(file, text));
    out.extend(line_length_diagnostics(text));
    out
}

fn is_cap_words(name: &str) -> bool {
    name.trim_start_matches(['_', '$'])
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_uppercase())
}

fn is_mixed_case(name: &str) -> bool {
    name.trim_start_matches(['_', '$'])
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_lowercase())
}

// All-caps with at least one letter, for example MAX_SUPPLY.
// No underscore rule: digits and underscores never decide.
fn is_upper_case(name: &str) -> bool {
    let mut has_alpha = false;
    for mark in name.chars() {
        if mark.is_ascii_alphabetic() {
            has_alpha = true;
            if !mark.is_ascii_uppercase() {
                return false;
            }
        }
    }
    has_alpha
}

fn push_naming(
    out: &mut Vec<DiagnosticData>,
    kind: &str,
    name: Option<String>,
    span: Option<Span>,
    fallback: Span,
    convention: &str,
    ok: fn(&str) -> bool,
) {
    let Some(name) = name else { return };
    if name.is_empty() || ok(&name) {
        return;
    }
    out.push(DiagnosticData::warning(
        span.unwrap_or(fallback),
        format!("{kind} '{name}' should use {convention}"),
        RuleId::NAMING,
    ));
}

fn naming_diagnostics(file: &SolidityFile) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    for item in file.structs() {
        push_naming(
            &mut out,
            "struct",
            item.name(),
            item.name_span(),
            item.syntax().span,
            "CapWords",
            is_cap_words,
        );
    }
    for item in file.enums() {
        push_naming(
            &mut out,
            "enum",
            item.name(),
            item.name_span(),
            item.syntax().span,
            "CapWords",
            is_cap_words,
        );
    }
    for item in file.errors() {
        push_naming(
            &mut out,
            "error",
            item.name(),
            item.name_span(),
            item.syntax().span,
            "CapWords",
            is_cap_words,
        );
    }
    for item in file.events() {
        push_naming(
            &mut out,
            "event",
            item.name(),
            item.name_span(),
            item.syntax().span,
            "CapWords",
            is_cap_words,
        );
        for param in item.params() {
            push_naming(
                &mut out,
                "parameter",
                param.name(),
                param.name_span(),
                param.syntax().span,
                "mixedCase",
                is_mixed_case,
            );
        }
    }
    for item in file.user_types() {
        push_naming(
            &mut out,
            "user type",
            item.name(),
            item.name_span(),
            item.syntax().span,
            "CapWords",
            is_cap_words,
        );
    }
    for contract in file.contracts() {
        let kind = contract.kind_label();
        push_naming(
            &mut out,
            kind,
            contract.name(),
            contract.name_span(),
            contract.syntax().span,
            "CapWords",
            is_cap_words,
        );
        naming_contract(&contract, &mut out);
    }
    out
}

fn naming_contract(contract: &crate::ContractDef, out: &mut Vec<DiagnosticData>) {
    for item in contract.structs() {
        push_naming(
            out,
            "struct",
            item.name(),
            item.name_span(),
            item.syntax().span,
            "CapWords",
            is_cap_words,
        );
    }
    for item in contract.enums() {
        push_naming(
            out,
            "enum",
            item.name(),
            item.name_span(),
            item.syntax().span,
            "CapWords",
            is_cap_words,
        );
    }
    for item in contract.errors() {
        push_naming(
            out,
            "error",
            item.name(),
            item.name_span(),
            item.syntax().span,
            "CapWords",
            is_cap_words,
        );
    }
    for item in contract.events() {
        push_naming(
            out,
            "event",
            item.name(),
            item.name_span(),
            item.syntax().span,
            "CapWords",
            is_cap_words,
        );
        for param in item.params() {
            push_naming(
                out,
                "parameter",
                param.name(),
                param.name_span(),
                param.syntax().span,
                "mixedCase",
                is_mixed_case,
            );
        }
    }
    for item in contract.user_types() {
        push_naming(
            out,
            "user type",
            item.name(),
            item.name_span(),
            item.syntax().span,
            "CapWords",
            is_cap_words,
        );
    }
    for item in contract.modifiers() {
        push_naming(
            out,
            "modifier",
            item.name(),
            item.name_span(),
            item.syntax().span,
            "mixedCase",
            is_mixed_case,
        );
    }
    for var in contract.state_vars() {
        let Some(name) = var.name() else { continue };
        if name.is_empty() {
            continue;
        }
        // Parser consumes constant and immutable without storing them,
        // so UPPER_CASE cannot prove constness: skip it as a constant.
        if is_upper_case(&name) {
            continue;
        }
        if is_mixed_case(&name) {
            continue;
        }
        out.push(DiagnosticData::warning(
            var.name_span().unwrap_or(var.syntax().span),
            format!("state variable '{name}' should use mixedCase"),
            RuleId::NAMING,
        ));
    }
    for function in contract.functions() {
        push_naming(
            out,
            "function",
            function.name(),
            function.name_span(),
            function.syntax().span,
            "mixedCase",
            is_mixed_case,
        );
        for param in function.params() {
            push_naming(
                out,
                "parameter",
                param.name(),
                param.name_span(),
                param.syntax().span,
                "mixedCase",
                is_mixed_case,
            );
        }
        let Some(body) = function.body() else {
            continue;
        };
        for local in body.all_locals() {
            push_naming(
                out,
                "variable",
                local.name(),
                local.name_span(),
                local.syntax().span,
                "mixedCase",
                is_mixed_case,
            );
        }
        for param in body.all_params() {
            push_naming(
                out,
                "parameter",
                param.name(),
                param.name_span(),
                param.syntax().span,
                "mixedCase",
                is_mixed_case,
            );
        }
    }
}

// Stored order rank: visibility 0, mutability 1, virtual 2, override 3.
// Custom modifiers are never stored (parser skips them), so they
// cannot break the order and stay invisible here by design.
fn modifier_rank(kind: u16) -> Option<u8> {
    if kind == SolidityKind::Public.as_u16()
        || kind == SolidityKind::Private.as_u16()
        || kind == SolidityKind::Internal.as_u16()
        || kind == SolidityKind::External.as_u16()
    {
        Some(0)
    } else if kind == SolidityKind::Pure.as_u16()
        || kind == SolidityKind::View.as_u16()
        || kind == SolidityKind::Payable.as_u16()
        || kind == SolidityKind::Constant.as_u16()
    {
        Some(1)
    } else if kind == SolidityKind::Virtual.as_u16() {
        Some(2)
    } else if kind == SolidityKind::Override.as_u16() {
        Some(3)
    } else {
        None
    }
}

fn modifier_order_diagnostics(file: &SolidityFile) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    for contract in file.contracts() {
        for function in contract.functions() {
            let Some(name) = function.name() else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            let mut max = 0u8;
            let mut seen = false;
            let mut bad = false;
            for element in &function.syntax().children {
                let SyntaxElement::Token(token) = element else {
                    continue;
                };
                let Some(rank) = modifier_rank(token.kind) else {
                    continue;
                };
                if !seen || rank >= max {
                    max = rank;
                    seen = true;
                } else {
                    bad = true;
                    break;
                }
            }
            if bad {
                out.push(DiagnosticData::warning(
                    function.name_span().unwrap_or(function.syntax().span),
                    format!(
                        "function '{name}' has modifiers out of order: expected visibility, mutability, virtual, override"
                    ),
                    RuleId::MODIFIER_ORDER,
                ));
            }
        }
    }
    out
}

// Primary rank by kind and visibility, secondary pushes view and pure
// after other functions with the same visibility. Missing visibility
// counts as public; payable counts as non-view.
fn function_rank(
    name: &str,
    visibility: Option<Visibility>,
    mutability: Option<Mutability>,
) -> (u8, u8) {
    if name == "constructor" {
        return (0, 0);
    }
    // receive and fallback arrive via name(): the parser stores the
    // keyword token and first_ident_text matches its spelling.
    if name == "receive" {
        return (1, 0);
    }
    if name == "fallback" {
        return (2, 0);
    }
    let primary = match visibility {
        Some(Visibility::External) => 3,
        Some(Visibility::Public) | None => 4,
        Some(Visibility::Internal) => 5,
        Some(Visibility::Private) => 6,
    };
    let secondary = match mutability {
        Some(Mutability::View) | Some(Mutability::Pure) => 1,
        _ => 0,
    };
    (primary, secondary)
}

fn function_order_diagnostics(file: &SolidityFile) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    for contract in file.contracts() {
        let mut max: (u8, u8) = (0, 0);
        let mut first = true;
        for function in contract.functions() {
            let Some(name) = function.name() else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            let rank = function_rank(&name, function.visibility(), function.mutability());
            if first || rank >= max {
                max = rank;
                first = false;
            } else {
                out.push(DiagnosticData::warning(
                    function.name_span().unwrap_or(function.syntax().span),
                    format!("function '{name}' breaks function order"),
                    RuleId::FUNCTION_ORDER,
                ));
            }
        }
    }
    out
}

fn file_rank(node: &forgels_syntax::SyntaxNode, contract_kind: &str) -> Option<u8> {
    if node.kind == SolidityNode::Pragma.as_u16() {
        Some(0)
    } else if node.kind == SolidityNode::Import.as_u16() {
        Some(1)
    } else if node.kind == SolidityNode::Using.as_u16() {
        Some(2)
    } else if node.kind == SolidityNode::Event.as_u16() {
        Some(3)
    } else if node.kind == SolidityNode::Error.as_u16() {
        Some(4)
    } else if node.kind == SolidityNode::UserType.as_u16() {
        Some(5)
    } else if node.kind == SolidityNode::Contract.as_u16() {
        match contract_kind {
            "interface" => Some(6),
            "library" => Some(7),
            _ => Some(8),
        }
    } else {
        None
    }
}

fn member_rank(node: &forgels_syntax::SyntaxNode) -> Option<u8> {
    if node.kind == SolidityNode::Using.as_u16() {
        Some(0)
    } else if node.kind == SolidityNode::Struct.as_u16()
        || node.kind == SolidityNode::Enum.as_u16()
        || node.kind == SolidityNode::UserType.as_u16()
    {
        Some(1)
    } else if node.kind == SolidityNode::StateVar.as_u16() {
        Some(2)
    } else if node.kind == SolidityNode::Event.as_u16() {
        Some(3)
    } else if node.kind == SolidityNode::Error.as_u16() {
        Some(4)
    } else if node.kind == SolidityNode::Modifier.as_u16() {
        Some(5)
    } else if node.kind == SolidityNode::Function.as_u16() {
        Some(6)
    } else {
        None
    }
}

fn layout_order_diagnostics(file: &SolidityFile) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    let mut max = 0u8;
    let mut first = true;
    for element in &file.syntax().children {
        let SyntaxElement::Node(node) = element else {
            continue;
        };
        let kind_label = if node.kind == SolidityNode::Contract.as_u16() {
            contract_kind_of(node)
        } else {
            ""
        };
        let Some(rank) = file_rank(node, kind_label) else {
            continue;
        };
        if first || rank >= max {
            max = rank;
            first = false;
            continue;
        }
        let (name, span) = file_item_name(node);
        out.push(DiagnosticData::warning(
            span.unwrap_or(node.span),
            format!("'{name}' breaks file layout order"),
            RuleId::LAYOUT_ORDER,
        ));
    }
    for contract in file.contracts() {
        let mut inner_max = 0u8;
        let mut inner_first = true;
        for element in &contract.syntax().children {
            let SyntaxElement::Node(node) = element else {
                continue;
            };
            let Some(rank) = member_rank(node) else {
                continue;
            };
            if inner_first || rank >= inner_max {
                inner_max = rank;
                inner_first = false;
                continue;
            }
            let (name, span) = member_item_name(node);
            out.push(DiagnosticData::warning(
                span.unwrap_or(node.span),
                format!("'{name}' breaks contract layout order"),
                RuleId::LAYOUT_ORDER,
            ));
        }
    }
    out
}

fn contract_kind_of(node: &forgels_syntax::SyntaxNode) -> &str {
    for element in &node.children {
        if let SyntaxElement::Token(token) = element {
            if token.kind == SolidityKind::Interface.as_u16() {
                return "interface";
            }
            if token.kind == SolidityKind::Library.as_u16() {
                return "library";
            }
        }
    }
    "contract"
}

fn first_ident(node: &forgels_syntax::SyntaxNode) -> (String, Option<Span>) {
    for element in &node.children {
        if let SyntaxElement::Token(token) = element
            && token.kind == SolidityKind::Ident.as_u16()
        {
            return (token.text.clone(), Some(token.span));
        }
    }
    (String::from("<unknown>"), None)
}

fn file_item_name(node: &forgels_syntax::SyntaxNode) -> (String, Option<Span>) {
    if node.kind == SolidityNode::Import.as_u16() {
        for element in &node.children {
            if let SyntaxElement::Token(token) = element
                && (token.text.starts_with('"') || token.text.starts_with('\''))
            {
                return (token.text.clone(), Some(token.span));
            }
        }
        return (String::from("import"), Some(node.span));
    }
    first_ident(node)
}

fn member_item_name(node: &forgels_syntax::SyntaxNode) -> (String, Option<Span>) {
    if node.kind == SolidityNode::Function.as_u16() {
        for element in &node.children {
            if let SyntaxElement::Token(token) = element
                && is_word_text(&token.text)
            {
                return (token.text.clone(), Some(token.span));
            }
        }
        return (String::from("function"), Some(node.span));
    }
    first_ident(node)
}

fn is_word_text(text: &str) -> bool {
    text.chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
}

// A late import fires both import-position and layout-order by design:
// one marks the top-of-file rule, the other marks the file order rank.
fn import_position_diagnostics(file: &SolidityFile) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    let mut seen_code = false;
    for element in &file.syntax().children {
        let SyntaxElement::Node(node) = element else {
            continue;
        };
        if node.kind == SolidityNode::Contract.as_u16()
            || node.kind == SolidityNode::Struct.as_u16()
            || node.kind == SolidityNode::Enum.as_u16()
        {
            seen_code = true;
        } else if node.kind == SolidityNode::Import.as_u16() && seen_code {
            let (path, span) = import_path_of(node);
            out.push(DiagnosticData::warning(
                span.unwrap_or(node.span),
                format!("import {path} should be at the top of the file"),
                RuleId::IMPORT_POSITION,
            ));
        }
    }
    out
}

fn import_path_of(node: &forgels_syntax::SyntaxNode) -> (String, Option<Span>) {
    for element in &node.children {
        if let SyntaxElement::Token(token) = element
            && (token.text.starts_with('"') || token.text.starts_with('\''))
        {
            return (token.text.clone(), Some(token.span));
        }
    }
    (String::from("<unknown>"), None)
}

// Any gap between `mapping` and `(` counts. Comments are tokens, so a
// comment in between always leaves a span gap and fires by design.
fn mapping_glue_diagnostics(text: &str) -> Vec<DiagnosticData> {
    let tokens = SolidityLexer.tokenize(text);
    let mut out = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        if tokens[index].kind != SolidityKind::Mapping.as_u16() {
            index += 1;
            continue;
        }
        let mut next = index + 1;
        while next < tokens.len() && tokens[next].kind == SolidityKind::Comment.as_u16() {
            next += 1;
        }
        let Some(open) = tokens.get(next) else {
            index += 1;
            continue;
        };
        if open.kind == SolidityKind::LParen.as_u16() && open.span.start > tokens[index].span.end {
            out.push(DiagnosticData::warning(
                tokens[index].span,
                String::from("mapping should be directly followed by '('"),
                RuleId::MAPPING_GLUE,
            ));
        }
        index += 1;
    }
    out
}

// A gap before `[` counts only for array types: the next token is a
// number or `]`. Index access never fires: a lowercase ident receiver
// (for example `a [0]`) is an expression, not a type. Uppercase idents
// count as user types and still fire.
fn array_glue_diagnostics(text: &str) -> Vec<DiagnosticData> {
    let tokens = SolidityLexer.tokenize(text);
    let mut out = Vec::new();
    for index in 0..tokens.len() {
        if tokens[index].kind != SolidityKind::LBracket.as_u16() || index == 0 {
            continue;
        }
        let prev = &tokens[index - 1];
        if tokens[index].span.start <= prev.span.end {
            continue;
        }
        let Some(next) = tokens.get(index + 1) else {
            continue;
        };
        let is_array_close = next.kind == SolidityKind::RBracket.as_u16()
            || next.kind == SolidityKind::Number.as_u16()
            || next.kind == SolidityKind::OctalNumber.as_u16();
        if !is_array_close {
            continue;
        }
        if prev.kind == SolidityKind::Ident.as_u16()
            && prev
                .text
                .chars()
                .next()
                .is_some_and(|first| first.is_ascii_lowercase())
        {
            continue;
        }
        out.push(DiagnosticData::warning(
            prev.span,
            String::from("array brackets should directly follow the type name"),
            RuleId::ARRAY_GLUE,
        ));
    }
    out
}

fn function_name_span(function: &crate::FunctionDef) -> Span {
    if let Some(span) = function.name_span() {
        return span;
    }
    for element in &function.syntax().children {
        if let SyntaxElement::Token(token) = element
            && is_word_text(&token.text)
        {
            return token.span;
        }
    }
    function.syntax().span
}

fn is_public_or_external(function: &crate::FunctionDef) -> bool {
    matches!(
        function.visibility(),
        Some(Visibility::Public) | Some(Visibility::External)
    )
}

fn natspec_target(function: &crate::FunctionDef) -> Option<String> {
    let name = function.name()?;
    if name.is_empty() || name == "constructor" {
        return None;
    }
    if !is_public_or_external(function) {
        return None;
    }
    Some(name)
}

fn natspec_missing_diagnostics(file: &SolidityFile, text: &str) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    for contract in file.contracts() {
        for function in contract.functions() {
            let Some(name) = natspec_target(&function) else {
                continue;
            };
            let start = function.syntax().span.start;
            if super::natspec::natspec_for(text, start).is_none() {
                out.push(DiagnosticData::warning(
                    function_name_span(&function),
                    format!("function '{name}' is missing NatSpec documentation"),
                    RuleId::NATSPEC_MISSING,
                ));
            }
        }
    }
    out
}

fn natspec_tag_names(docs: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in docs.lines() {
        let clean = line.trim_start().trim_start_matches('*').trim_start();
        let Some(rest) = clean.strip_prefix("@param") else {
            continue;
        };
        let rest = rest.trim_start();
        if rest.is_empty() {
            continue;
        }
        let raw = rest.split_whitespace().next().unwrap_or("");
        let name: String = raw
            .chars()
            .take_while(|mark| mark.is_ascii_alphanumeric() || *mark == '_' || *mark == '$')
            .collect();
        if name.is_empty() {
            continue;
        }
        out.push(name);
    }
    out
}

fn natspec_param_diagnostics(file: &SolidityFile, text: &str) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    for contract in file.contracts() {
        for function in contract.functions() {
            if natspec_target(&function).is_none() {
                continue;
            };
            let start = function.syntax().span.start;
            let tags = super::natspec::natspec_for(text, start)
                .map(|docs| natspec_tag_names(&docs))
                .unwrap_or_default();
            let params = function.params();
            for param in &params {
                let Some(param_name) = param.name() else {
                    continue;
                };
                if param_name.is_empty() {
                    continue;
                }
                if !tags.iter().any(|tag| tag == &param_name) {
                    out.push(DiagnosticData::warning(
                        function_name_span(&function),
                        format!("parameter '{param_name}' is missing NatSpec @param"),
                        RuleId::NATSPEC_PARAM,
                    ));
                }
            }
            for tag in &tags {
                if !params
                    .iter()
                    .any(|param| param.name().as_deref() == Some(tag))
                {
                    out.push(DiagnosticData::warning(
                        function_name_span(&function),
                        format!("NatSpec @param '{tag}' does not match any parameter"),
                        RuleId::NATSPEC_PARAM,
                    ));
                }
            }
        }
    }
    out
}

fn element_span(element: &SyntaxElement) -> Span {
    match element {
        SyntaxElement::Node(node) => node.span,
        SyntaxElement::Token(token) => token.span,
    }
}

fn blank_count(gap: &str) -> usize {
    let parts: Vec<&str> = gap.split('\n').collect();
    if parts.len() < 3 {
        return 0;
    }
    parts[1..parts.len() - 1]
        .iter()
        .filter(|line| line.trim().is_empty())
        .count()
}

fn next_top_span(node: &forgels_syntax::SyntaxNode) -> Option<Span> {
    let (_, span) = file_item_name(node);
    span
}

fn next_member_span(node: &forgels_syntax::SyntaxNode) -> Option<Span> {
    let (_, span) = member_item_name(node);
    span
}

fn blank_lines_diagnostics(file: &SolidityFile, text: &str) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    let top = &file.syntax().children;
    for pair in top.windows(2) {
        let prev = element_span(&pair[0]);
        let next = element_span(&pair[1]);
        if next.start < prev.end {
            continue;
        }
        let (prev_end, next_start) = (prev.end as usize, next.start as usize);
        if prev_end > next_start {
            continue;
        }
        let Some(gap) = text.get(prev_end..next_start) else {
            continue;
        };
        if blank_count(gap) >= 2 {
            continue;
        }
        let fallback = Span::new(next.start, next.start);
        let span = match &pair[1] {
            SyntaxElement::Node(node) => next_top_span(node).unwrap_or(fallback),
            SyntaxElement::Token(token) => token.span,
        };
        let found = blank_count(gap);
        out.push(DiagnosticData::warning(
            span,
            format!("expected 2 blank lines between top-level declarations, found {found}"),
            RuleId::BLANK_LINES,
        ));
    }
    for contract in file.contracts() {
        let members: Vec<forgels_syntax::SyntaxNode> = contract
            .syntax()
            .children
            .iter()
            .filter_map(|element| match element {
                SyntaxElement::Node(node) => Some(node.clone()),
                _ => None,
            })
            .collect();
        for pair in members.windows(2) {
            if pair[0].kind != SolidityNode::Function.as_u16()
                || pair[1].kind != SolidityNode::Function.as_u16()
            {
                continue;
            }
            let prev_span = pair[0].span;
            let next_span = pair[1].span;
            if next_span.start < prev_span.end {
                continue;
            }
            let (prev_end, next_start) = (prev_span.end as usize, next_span.start as usize);
            if prev_end > next_start {
                continue;
            }
            let Some(gap) = text.get(prev_end..next_start) else {
                continue;
            };
            if blank_count(gap) >= 1 {
                continue;
            }
            let Some(prev_src) = text.get(prev_span.start as usize..prev_span.end as usize) else {
                continue;
            };
            let Some(next_src) = text.get(next_span.start as usize..next_span.end as usize) else {
                continue;
            };
            if !prev_src.contains('\n') && !next_src.contains('\n') {
                continue;
            }
            let fallback = Span::new(next_span.start, next_span.start);
            let span = next_member_span(&pair[1]).unwrap_or(fallback);
            let found = blank_count(gap);
            out.push(DiagnosticData::warning(
                span,
                format!("expected 1 blank line between functions, found {found}"),
                RuleId::BLANK_LINES,
            ));
        }
    }
    out
}

fn line_length_diagnostics(text: &str) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    let mut offset: u32 = 0;
    for line in text.split('\n') {
        let clean = line.strip_suffix('\r').unwrap_or(line);
        let count = clean.chars().count();
        if count > 120 {
            let span = Span::new(offset, offset.saturating_add(clean.len() as u32));
            out.push(DiagnosticData::warning(
                span,
                format!("line exceeds 120 characters ({count} found)"),
                RuleId::LINE_LENGTH,
            ));
        }
        offset = offset.saturating_add(line.len() as u32 + 1);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SolidityLexer, SolidityParser};
    use forgels_lint::Severity;

    fn style_of(text: &str) -> Vec<DiagnosticData> {
        let tokens = SolidityLexer.tokenize(text);
        let (file, _) = SolidityParser.parse_with_errors(&tokens);
        style_diagnostics(&file, text)
    }

    fn rules_of(text: &str) -> Vec<&'static str> {
        style_of(text)
            .iter()
            .map(|diag| diag.rule.as_str())
            .collect()
    }

    #[test]
    fn naming_good_shapes_stay_silent() {
        let text = "contract MyToken {\nuint256 public totalSupply;\n/// @notice Transfers.\n/// @param to Recipient.\nfunction transfer(address to) public {}\n}";
        assert!(style_of(text).is_empty(), "diags: {:?}", style_of(text));
    }

    #[test]
    fn naming_bad_shapes_warn_with_convention() {
        let text = "contract mytoken {\nuint256 public TotalSupply;\n/// @notice Does work.\n/// @param to Recipient.\nfunction Transfer(address to) public {}\n}";
        let found = style_of(text);
        assert_eq!(found.len(), 3, "diags: {found:?}");
        for diag in &found {
            assert_eq!(diag.rule, RuleId::NAMING);
            assert_eq!(diag.rule.as_str(), "style/naming");
            assert_eq!(diag.severity, Severity::Warning);
        }
        assert!(
            found
                .iter()
                .any(|diag| diag.message == "contract 'mytoken' should use CapWords"),
            "messages: {:?}",
            found.iter().map(|diag| &diag.message).collect::<Vec<_>>()
        );
        assert!(
            found
                .iter()
                .any(|diag| diag.message == "function 'Transfer' should use mixedCase"),
            "messages: {:?}",
            found.iter().map(|diag| &diag.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn naming_upper_case_state_var_stays_silent() {
        let text = "contract C {\nuint256 constant MAX_SUPPLY = 1;\n}";
        assert!(style_of(text).is_empty(), "diags: {:?}", style_of(text));
    }

    #[test]
    fn modifier_order_canonical_stays_silent() {
        let text =
            "contract C {\n/// @notice Does f.\nfunction f() public view override onlyOwner {}\n}";
        assert!(style_of(text).is_empty(), "diags: {:?}", style_of(text));
    }

    #[test]
    fn modifier_order_shuffled_warns() {
        let text =
            "contract C {\n/// @notice Does f.\nfunction f() public override view onlyOwner {}\n}";
        let found = style_of(text);
        assert_eq!(found.len(), 1, "diags: {found:?}");
        assert_eq!(found[0].rule, RuleId::MODIFIER_ORDER);
        assert_eq!(found[0].rule.as_str(), "style/modifier-order");
        assert_eq!(found[0].severity, Severity::Warning);
    }

    #[test]
    fn function_order_canonical_stays_silent() {
        let text = "contract C {\nconstructor() {}\n/// @notice External.\nfunction ext() external {}\n/// @notice Public.\nfunction pubf() public {}\nfunction inter() internal {}\nfunction priv() private {}\n}";
        assert!(style_of(text).is_empty(), "diags: {:?}", style_of(text));
    }

    #[test]
    fn function_order_private_before_constructor_warns() {
        let text = "contract C {\nfunction priv() private {}\nconstructor() {}\n}";
        let found = style_of(text);
        assert_eq!(found.len(), 1, "diags: {found:?}");
        assert_eq!(found[0].rule, RuleId::FUNCTION_ORDER);
        assert_eq!(found[0].rule.as_str(), "style/function-order");
        assert_eq!(found[0].severity, Severity::Warning);
    }

    #[test]
    fn function_order_plain_before_view_stays_silent() {
        let text = "contract C {\n/// @notice Does b.\nfunction b() public {}\n/// @notice Does a.\nfunction a() public view {}\n}";
        assert!(style_of(text).is_empty(), "diags: {:?}", style_of(text));
    }

    #[test]
    fn function_order_view_before_plain_warns() {
        let text = "contract C {\n/// @notice Does a.\nfunction a() public view {}\n/// @notice Does b.\nfunction b() public {}\n}";
        let found = style_of(text);
        assert_eq!(found.len(), 1, "diags: {found:?}");
        assert_eq!(found[0].rule, RuleId::FUNCTION_ORDER);
        assert_eq!(found[0].severity, Severity::Warning);
    }

    #[test]
    fn layout_order_canonical_stays_silent() {
        let text = "pragma solidity ^0.8.0;\n\n\nimport \"./B.sol\";\n\n\nevent E();\n\n\nerror Err();\n\n\ncontract C {\nuint256 x;\nevent F();\n/// @notice Does g.\nfunction g() public {}\n}";
        assert!(style_of(text).is_empty(), "diags: {:?}", style_of(text));
    }

    #[test]
    fn layout_order_var_after_function_warns() {
        let text = "contract C {\n/// @notice Does g.\nfunction g() public {}\nuint256 x;\n}";
        let found = style_of(text);
        assert_eq!(found.len(), 1, "diags: {found:?}");
        assert_eq!(found[0].rule, RuleId::LAYOUT_ORDER);
        assert_eq!(found[0].rule.as_str(), "style/layout-order");
        assert_eq!(found[0].severity, Severity::Warning);
    }

    #[test]
    fn import_position_top_stays_silent() {
        let text = "pragma solidity ^0.8.0;\n\n\nimport \"./B.sol\";\n\n\ncontract C {\n}\n";
        assert!(style_of(text).is_empty(), "diags: {:?}", style_of(text));
    }

    #[test]
    fn import_position_after_contract_fires_both_rules() {
        let text = "contract C {\n}\n\n\nimport \"./B.sol\";\n";
        let found = style_of(text);
        let rules = rules_of(text);
        assert!(rules.contains(&"style/import-position"), "rules: {rules:?}");
        assert!(rules.contains(&"style/layout-order"), "rules: {rules:?}");
        assert_eq!(found.len(), 2, "diags: {found:?}");
        for diag in &found {
            assert_eq!(diag.severity, Severity::Warning);
        }
        let import = found
            .iter()
            .find(|diag| diag.rule == RuleId::IMPORT_POSITION)
            .expect("import diagnostic");
        assert_eq!(import.rule.as_str(), "style/import-position");
    }

    #[test]
    fn mapping_glue_tight_stays_silent() {
        let text = "contract C {\nmapping(uint256 => uint256) map;\n}";
        assert!(style_of(text).is_empty(), "diags: {:?}", style_of(text));
    }

    #[test]
    fn mapping_glue_spaced_warns() {
        let text = "contract C {\nmapping (uint256 => uint256) map;\n}";
        let found = style_of(text);
        assert_eq!(found.len(), 1, "diags: {found:?}");
        assert_eq!(found[0].rule, RuleId::MAPPING_GLUE);
        assert_eq!(found[0].rule.as_str(), "style/mapping-glue");
        assert_eq!(found[0].severity, Severity::Warning);
    }

    #[test]
    fn mapping_glue_comment_counts_as_gap() {
        let text = "contract C {\nmapping/*c*/(uint256 => uint256) map;\n}";
        let found = style_of(text);
        assert_eq!(found.len(), 1, "diags: {found:?}");
        assert_eq!(found[0].rule, RuleId::MAPPING_GLUE);
    }

    #[test]
    fn array_glue_tight_stays_silent() {
        let text = "contract C {\nuint256[] x;\n}";
        assert!(style_of(text).is_empty(), "diags: {:?}", style_of(text));
    }

    #[test]
    fn array_glue_spaced_warns() {
        let text = "contract C {\nuint256 [] x;\n}";
        let found = style_of(text);
        assert_eq!(found.len(), 1, "diags: {found:?}");
        assert_eq!(found[0].rule, RuleId::ARRAY_GLUE);
        assert_eq!(found[0].rule.as_str(), "style/array-glue");
        assert_eq!(found[0].severity, Severity::Warning);
    }

    #[test]
    fn array_glue_index_access_with_space_stays_silent() {
        let text = "contract C {\nfunction f() public {\na [0];\n}\n}";
        assert!(
            style_of(text)
                .iter()
                .all(|diag| diag.rule != RuleId::ARRAY_GLUE),
            "diags: {:?}",
            style_of(text)
        );
    }

    fn only_rule(text: &str, rule: RuleId) -> Vec<DiagnosticData> {
        style_of(text)
            .into_iter()
            .filter(|diag| diag.rule == rule)
            .collect()
    }

    #[test]
    fn blank_lines_top_level_two_blanks_stay_silent() {
        let text = "contract A {\n}\n\n\ncontract B {\n}\n";
        assert!(
            only_rule(text, RuleId::BLANK_LINES).is_empty(),
            "diags: {:?}",
            style_of(text)
        );
    }

    #[test]
    fn blank_lines_top_level_missing_warns() {
        let text = "contract A {\n}\ncontract B {\n}\n";
        let found = only_rule(text, RuleId::BLANK_LINES);
        assert_eq!(found.len(), 1, "diags: {found:?}");
        assert_eq!(found[0].rule, RuleId::BLANK_LINES);
        assert_eq!(found[0].rule.as_str(), "style/blank-lines");
        assert_eq!(found[0].severity, Severity::Warning);
    }

    #[test]
    fn blank_lines_contract_one_blank_stays_silent() {
        let text = "contract C {\nfunction a() internal {\nuint256 x;\n}\n\nfunction b() internal {\nuint256 y;\n}\n}";
        assert!(
            only_rule(text, RuleId::BLANK_LINES).is_empty(),
            "diags: {:?}",
            style_of(text)
        );
    }

    #[test]
    fn blank_lines_contract_missing_warns() {
        let text = "contract C {\nfunction a() internal {\nuint256 x;\n}\nfunction b() internal {\nuint256 y;\n}\n}";
        let found = only_rule(text, RuleId::BLANK_LINES);
        assert_eq!(found.len(), 1, "diags: {found:?}");
        assert_eq!(found[0].rule, RuleId::BLANK_LINES);
        assert_eq!(found[0].severity, Severity::Warning);
    }

    #[test]
    fn blank_lines_single_line_pair_stays_silent() {
        let text = "contract C {\nfunction a() internal {}\nfunction b() internal {}\n}";
        assert!(
            only_rule(text, RuleId::BLANK_LINES).is_empty(),
            "diags: {:?}",
            style_of(text)
        );
    }

    #[test]
    fn natspec_missing_warns() {
        let text = "contract C {\nfunction f() public {}\n}";
        let found = only_rule(text, RuleId::NATSPEC_MISSING);
        assert_eq!(found.len(), 1, "diags: {found:?}");
        assert_eq!(found[0].rule.as_str(), "style/natspec-missing");
        assert_eq!(found[0].severity, Severity::Warning);
        assert_eq!(
            found[0].message,
            "function 'f' is missing NatSpec documentation"
        );
    }

    #[test]
    fn natspec_present_stays_silent() {
        let text = "contract C {\n/// @notice Does f.\nfunction f() public {}\n}";
        assert!(
            only_rule(text, RuleId::NATSPEC_MISSING).is_empty(),
            "diags: {:?}",
            style_of(text)
        );
        assert!(
            only_rule(text, RuleId::NATSPEC_PARAM).is_empty(),
            "diags: {:?}",
            style_of(text)
        );
    }

    #[test]
    fn natspec_param_missing_warns() {
        let text = "contract C {\n/// @notice Does f.\nfunction f(uint256 amount) public {}\n}";
        let found = only_rule(text, RuleId::NATSPEC_PARAM);
        assert_eq!(found.len(), 1, "diags: {found:?}");
        assert_eq!(found[0].rule.as_str(), "style/natspec-param");
        assert_eq!(found[0].severity, Severity::Warning);
        assert_eq!(
            found[0].message,
            "parameter 'amount' is missing NatSpec @param"
        );
    }

    #[test]
    fn natspec_param_extra_warns() {
        let text =
            "contract C {\n/// @notice Does f.\n/// @param wrong Does.\nfunction f() public {}\n}";
        let found = only_rule(text, RuleId::NATSPEC_PARAM);
        assert_eq!(found.len(), 1, "diags: {found:?}");
        assert_eq!(found[0].rule, RuleId::NATSPEC_PARAM);
        assert_eq!(found[0].severity, Severity::Warning);
        assert_eq!(
            found[0].message,
            "NatSpec @param 'wrong' does not match any parameter"
        );
    }

    #[test]
    fn natspec_param_unnamed_skipped() {
        let text = "contract C {\n/// @notice Does f.\nfunction f(uint256) public {}\n}";
        assert!(
            only_rule(text, RuleId::NATSPEC_PARAM).is_empty(),
            "diags: {:?}",
            style_of(text)
        );
    }

    #[test]
    fn line_length_120_ok_121_fires() {
        let ok_line = format!("// {}", "a".repeat(117));
        let long_line = format!("// {}", "a".repeat(118));
        assert_eq!(ok_line.chars().count(), 120);
        assert_eq!(long_line.chars().count(), 121);
        assert!(
            only_rule(&ok_line, RuleId::LINE_LENGTH).is_empty(),
            "diags: {:?}",
            style_of(&ok_line)
        );
        let found = only_rule(&long_line, RuleId::LINE_LENGTH);
        assert_eq!(found.len(), 1, "diags: {found:?}");
        assert_eq!(found[0].rule, RuleId::LINE_LENGTH);
        assert_eq!(found[0].rule.as_str(), "style/line-length");
        assert_eq!(found[0].severity, Severity::Warning);
        assert_eq!(found[0].message, "line exceeds 120 characters (121 found)");
    }
}
