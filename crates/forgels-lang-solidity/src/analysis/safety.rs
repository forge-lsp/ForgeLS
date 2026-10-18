use std::collections::HashSet;

use forgels_core::DiagnosticData;
use forgels_lint::RuleId;
use forgels_syntax::{AstNode, Lexer, Span, SyntaxElement};

use super::{Scope, Symbol, SymbolKind, mutability, unknown};
use crate::{
    BlockNode, FunctionDef, MemberExpr, Mutability, SolidityFile, SolidityLexer, Visibility,
    lexer::SolidityKind,
};

// Safety batch plus foundry test naming: warn-only best-effort checks.
// False negatives beat false positives; every limit is noted at the check.
// No correctness changes: these never emit errors.
pub fn safety_diagnostics(file: &SolidityFile, text: &str, scope: &Scope) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    out.extend(tx_origin_diagnostics(file));
    out.extend(unchecked_call_diagnostics(file));
    out.extend(reentrancy_diagnostics(file, text, scope));
    out.extend(encode_packed_diagnostics(file, text, scope));
    out.extend(payable_fallback_diagnostics(file));
    out
}

// Any `tx.origin` member read warns, regardless of context. Limit: the
// skeleton stores only top-level member statements, so uses inside call
// args, conditions, or assignments stay silent.
fn tx_origin_diagnostics(file: &SolidityFile) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    for contract in file.contracts() {
        for function in contract.functions() {
            let Some(body) = function.body() else {
                continue;
            };
            for member in body.all_members() {
                let chain = member.chain();
                if chain.len() < 2 {
                    continue;
                }
                if chain[0] != "tx" || chain[1] != "origin" {
                    continue;
                }
                let span = member
                    .member_span()
                    .or_else(|| member.final_span())
                    .unwrap_or(member.syntax().span);
                out.push(DiagnosticData::warning(
                    span,
                    String::from("avoid tx.origin for authorization, use msg.sender"),
                    RuleId::TX_ORIGIN,
                ));
            }
        }
    }
    out
}

// Direct member statements only: members() plus recursed child blocks.
// Assigned, destructured, and arg forms parse with no Member node, so
// they stay silent by construction (see the parser fixture test).
// transfer is not a low-level call and stays silent; send warns.
fn unchecked_call_diagnostics(file: &SolidityFile) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    for contract in file.contracts() {
        for function in contract.functions() {
            let Some(body) = function.body() else {
                continue;
            };
            for member in direct_members(&body) {
                if !member.is_call() {
                    continue;
                }
                let Some(final_name) = member.final_member() else {
                    continue;
                };
                if final_name != "call"
                    && final_name != "delegatecall"
                    && final_name != "staticcall"
                    && final_name != "send"
                {
                    continue;
                }
                let span = member
                    .member_span()
                    .or_else(|| member.final_span())
                    .unwrap_or(member.syntax().span);
                out.push(DiagnosticData::warning(
                    span,
                    String::from("unchecked low-level call return value"),
                    RuleId::UNCHECKED_CALL,
                ));
            }
        }
    }
    out
}

// Same set as BlockNode::all_members, spelled out so the bare-statement
// invariant stays visible at the check site.
fn direct_members(block: &BlockNode) -> Vec<MemberExpr> {
    let mut out = block.members();
    for child in block.child_blocks() {
        out.extend(direct_members(&child));
    }
    out
}

// One warn per state write after the earliest .call/.delegatecall in the
// function. send, transfer, and staticcall never count as external calls.
// Writes reuse the mutability limits: mappings, longer chains, assembly,
// and call targets stay silent. Nested blocks report in source order.
fn reentrancy_diagnostics(file: &SolidityFile, text: &str, scope: &Scope) -> Vec<DiagnosticData> {
    let tokens = SolidityLexer.tokenize(text);
    let assemblies = unknown::assembly_ranges(&tokens);
    let decls = unknown::decl_spans(file);
    let decl_set: HashSet<(u32, u32)> = decls.iter().map(|span| (span.start, span.end)).collect();
    let mut out = Vec::new();
    for contract in file.contracts() {
        for function in contract.functions() {
            let Some(body) = function.body() else {
                continue;
            };
            let mut calls: Vec<u32> = Vec::new();
            for member in body.all_members() {
                if !member.is_call() {
                    continue;
                }
                let Some(final_name) = member.final_member() else {
                    continue;
                };
                if final_name != "call" && final_name != "delegatecall" {
                    continue;
                }
                calls.push(member.syntax().span.start);
            }
            let Some(first_call) = calls.iter().min().copied() else {
                continue;
            };
            let mut writes = state_write_spans(&tokens, &assemblies, &decl_set, scope, &body, text);
            writes.sort_by_key(|span| (span.start, span.end));
            for span in writes {
                if span.start <= first_call {
                    continue;
                }
                out.push(DiagnosticData::warning(
                    span,
                    String::from("state write after external call risks reentrancy"),
                    RuleId::REENTRANCY_PATTERN,
                ));
            }
        }
    }
    out
}

// Token scan for StateVar roots plus two-segment StateVar-rooted member
// writes. Shares helpers with mutability.rs, same silent limits there.
fn state_write_spans(
    tokens: &[forgels_syntax::Token],
    assemblies: &[Span],
    decls: &HashSet<(u32, u32)>,
    scope: &Scope,
    body: &BlockNode,
    text: &str,
) -> Vec<Span> {
    let mut out = Vec::new();
    let body_span = body.syntax().span;
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != SolidityKind::Ident.as_u16() {
            continue;
        }
        if token.text.is_empty() {
            continue;
        }
        let span = token.span;
        if !body_span.contains(span.start) {
            continue;
        }
        if decls.contains(&(span.start, span.end)) {
            continue;
        }
        if unknown::inside_any(assemblies, span.start) {
            continue;
        }
        let name = token.text.as_str();
        let after_dot = unknown::preceded_by_dot(text, span.start);
        if after_dot {
            let dot = span.start as usize - 1;
            let Some(receiver) = mutability::receiver_before(text, dot) else {
                continue;
            };
            if receiver != "this" {
                continue;
            }
        }
        let Some(symbol) = scope.resolve(name, span.start) else {
            continue;
        };
        if symbol.kind != SymbolKind::StateVar {
            continue;
        }
        if !after_dot && mutability::root_has_member_write(tokens, index) {
            continue;
        }
        let prev = mutability::prev_code(tokens, index);
        let next = mutability::next_code_token(tokens, index);
        let mut is_write = false;
        if let Some(before) = prev
            && (before.kind == SolidityKind::Delete.as_u16()
                || before.kind == SolidityKind::PlusPlus.as_u16()
                || before.kind == SolidityKind::MinusMinus.as_u16())
        {
            is_write = true;
        }
        if let Some(after) = next
            && mutability::is_write_kind(after.kind)
        {
            is_write = true;
        }
        if after_dot && !is_write {
            is_write = mutability::delete_before_this(tokens, index);
        }
        if !is_write {
            continue;
        }
        out.push(span);
    }
    for member in body.all_members() {
        if let Some(span) = member_write_span(tokens, assemblies, scope, &member) {
            out.push(span);
        }
    }
    out
}

// Direct `s.field` on the left of an assign family plus arr.push/pop.
// Mirrors mutability::check_member_write span logic as a span query.
fn member_write_span(
    tokens: &[forgels_syntax::Token],
    assemblies: &[Span],
    scope: &Scope,
    member: &MemberExpr,
) -> Option<Span> {
    let chain = member.chain();
    if chain.len() != 2 {
        return None;
    }
    let root = chain.first().cloned().unwrap_or_default();
    if root.is_empty() || root == "this" || root == "super" {
        return None;
    }
    let field = member.final_member()?;
    if field.is_empty() {
        return None;
    }
    let span = member.syntax().span;
    if unknown::inside_any(assemblies, span.start) {
        return None;
    }
    let symbol = scope.resolve(&root, span.start)?;
    if symbol.kind != SymbolKind::StateVar {
        return None;
    }
    if member.is_call() {
        if field == "push" || field == "pop" {
            return Some(span);
        }
        return None;
    }
    let next = mutability::next_code_after(tokens, span.end)?;
    if mutability::is_write_kind(next.kind) {
        Some(span)
    } else {
        None
    }
}

// Only abi.encodePacked calls. Args slice from source at top-level
// commas; each arg root ident resolves via scope to a type text.
// Dynamic means string, bytes, or T[]. Fixed bytesNN and T[N] stay
// silent; unresolvable or complex args stay silent. One warn per call.
fn encode_packed_diagnostics(
    file: &SolidityFile,
    text: &str,
    scope: &Scope,
) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    for contract in file.contracts() {
        for function in contract.functions() {
            let Some(body) = function.body() else {
                continue;
            };
            for member in body.all_members() {
                if !member.is_call() {
                    continue;
                }
                let chain = member.chain();
                if chain.len() != 2 {
                    continue;
                }
                if chain[0] != "abi" || chain[1] != "encodePacked" {
                    continue;
                }
                let span = member.syntax().span;
                let (start, end) = (span.start as usize, span.end as usize);
                let Some(src) = text.get(start..end) else {
                    continue;
                };
                let Some(open) = src.find('(') else {
                    continue;
                };
                let Some(close) = src.rfind(')') else {
                    continue;
                };
                if close <= open {
                    continue;
                }
                let Some(args) = src.get(open + 1..close) else {
                    continue;
                };
                let mut dynamic = false;
                for arg in split_top_args(args) {
                    let Some(root) = arg_root(arg) else {
                        continue;
                    };
                    let Some(symbol) = scope.resolve(&root, span.start) else {
                        continue;
                    };
                    let Some(type_text) = symbol_type(symbol, &root) else {
                        continue;
                    };
                    if is_dynamic_type(&type_text) {
                        dynamic = true;
                        break;
                    }
                }
                if !dynamic {
                    continue;
                }
                let warn_span = member
                    .member_span()
                    .or_else(|| member.final_span())
                    .unwrap_or(span);
                out.push(DiagnosticData::warning(
                    warn_span,
                    String::from("abi.encodePacked with dynamic types can collide, use abi.encode"),
                    RuleId::ENCODE_PACKED_DYNAMIC,
                ));
            }
        }
    }
    out
}

fn split_top_args(args: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0u32;
    let mut start = 0;
    for (index, mark) in args.char_indices() {
        match mark {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                out.push(args.get(start..index).unwrap_or(""));
                start = index + 1;
            }
            _ => {}
        }
    }
    out.push(args.get(start..).unwrap_or(""));
    out
}

fn arg_root(arg: &str) -> Option<String> {
    let clean = arg.trim_start();
    let mut end = 0;
    for (index, mark) in clean.char_indices() {
        if mark.is_ascii_alphanumeric() || mark == '_' || mark == '$' {
            end = index + mark.len_utf8();
        } else {
            break;
        }
    }
    if end == 0 {
        return None;
    }
    clean.get(..end).map(|root| root.to_string())
}

// Symbol detail carries "type name" with optional " = init" for state
// vars, so strip the init head then the trailing name. Anything else
// (getter details, user types) yields None and stays silent.
fn symbol_type(symbol: &Symbol, root: &str) -> Option<String> {
    let head = symbol.detail.split('=').next()?.trim();
    let without = head.strip_suffix(root)?.trim();
    if without.is_empty() {
        return None;
    }
    Some(without.to_string())
}

fn is_dynamic_type(type_text: &str) -> bool {
    let mut clean = type_text.trim();
    for suffix in [" memory", " calldata", " storage"] {
        if let Some(base) = clean.strip_suffix(suffix) {
            clean = base;
            break;
        }
    }
    if clean == "string" || clean == "bytes" {
        return true;
    }
    clean.ends_with("[]")
}

// Fires when a contract declares fallback without payable and no
// receive exists. Receive accepts ether, so fallback-plus-receive
// stays silent; payable fallback stays silent.
fn payable_fallback_diagnostics(file: &SolidityFile) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    for contract in file.contracts() {
        let functions = contract.functions();
        if functions
            .iter()
            .any(|function| function.name().as_deref() == Some("receive"))
        {
            continue;
        }
        for function in &functions {
            if function.name().as_deref() != Some("fallback") {
                continue;
            }
            if function.mutability() == Some(Mutability::Payable) {
                continue;
            }
            out.push(DiagnosticData::warning(
                function_name_span(function),
                String::from("fallback without payable and without receive cannot accept ether"),
                RuleId::PAYABLE_FALLBACK,
            ));
        }
    }
    out
}

// Keyword names (fallback, receive, constructor) lex as keywords, not
// Ident, so name_span misses them: first word token covers the keyword.
fn function_name_span(function: &FunctionDef) -> Span {
    if let Some(span) = function.name_span() {
        return span;
    }
    for element in &function.syntax().children {
        if let SyntaxElement::Token(token) = element
            && token
                .text
                .chars()
                .next()
                .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        {
            return token.span;
        }
    }
    function.syntax().span
}

// Foundry test naming: only contracts inheriting Test, only
// public/external functions. test, invariant, and setUp prefixes stay
// silent. Bare helpers in plain contracts stay silent (no heritage).
pub fn test_naming_diagnostics(file: &SolidityFile) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    for contract in file.contracts() {
        if !super::forge::inherits_test(file, &contract) {
            continue;
        }
        for function in contract.functions() {
            let Some(name) = function.name() else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            if !matches!(
                function.visibility(),
                Some(Visibility::Public) | Some(Visibility::External)
            ) {
                continue;
            }
            if name.starts_with("test")
                || name.starts_with("invariant")
                || name.starts_with("setUp")
            {
                continue;
            }
            out.push(DiagnosticData::warning(
                function_name_span(&function),
                format!("test function '{name}' should start with test, invariant, or setUp"),
                RuleId::TEST_NAMING,
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SolidityLexer, SolidityParser};

    fn safety_of(text: &str) -> Vec<DiagnosticData> {
        let tokens = SolidityLexer.tokenize(text);
        let (file, _) = SolidityParser.parse_with_errors(&tokens);
        let scope = super::super::collect_symbols(&file);
        safety_diagnostics(&file, text, &scope)
    }

    fn naming_of(text: &str) -> Vec<DiagnosticData> {
        let tokens = SolidityLexer.tokenize(text);
        let (file, _) = SolidityParser.parse_with_errors(&tokens);
        test_naming_diagnostics(&file)
    }

    fn only_rule(text: &str, rule: RuleId) -> Vec<DiagnosticData> {
        safety_of(text)
            .into_iter()
            .filter(|diag| diag.rule == rule)
            .collect()
    }

    #[test]
    fn tx_origin_bare_read_warns() {
        let text = "contract C {\nfunction f() public {\ntx.origin;\n}\n}";
        let found = only_rule(text, RuleId::TX_ORIGIN);
        assert_eq!(found.len(), 1, "diags: {found:?}");
        assert_eq!(found[0].rule.as_str(), "safety/tx-origin");
        assert_eq!(found[0].severity, forgels_lint::Severity::Warning);
        assert_eq!(
            found[0].message,
            "avoid tx.origin for authorization, use msg.sender"
        );
    }

    #[test]
    fn tx_origin_msg_sender_stays_silent() {
        let text = "contract C {\nfunction f() public {\nmsg.sender;\n}\n}";
        assert!(
            only_rule(text, RuleId::TX_ORIGIN).is_empty(),
            "diags: {:?}",
            safety_of(text)
        );
    }

    #[test]
    fn unchecked_call_bare_warns() {
        let text = "contract C {\nfunction f() public {\naddr.call(\"\");\n}\n}";
        let found = only_rule(text, RuleId::UNCHECKED_CALL);
        assert_eq!(found.len(), 1, "diags: {found:?}");
        assert_eq!(found[0].rule.as_str(), "safety/unchecked-call");
        assert_eq!(found[0].severity, forgels_lint::Severity::Warning);
        assert_eq!(found[0].message, "unchecked low-level call return value");
    }

    #[test]
    fn unchecked_call_assigned_stays_silent() {
        let text = "contract C {\nfunction f() public {\nbool ok = addr.call(\"\");\n}\n}";
        assert!(
            only_rule(text, RuleId::UNCHECKED_CALL).is_empty(),
            "diags: {:?}",
            safety_of(text)
        );
    }

    #[test]
    fn unchecked_call_destructured_stays_silent() {
        let text = "contract C {\nfunction f() public {\nbool ok;\nbytes memory data;\n(ok, data) = addr.call(\"\");\n}\n}";
        assert!(
            only_rule(text, RuleId::UNCHECKED_CALL).is_empty(),
            "diags: {:?}",
            safety_of(text)
        );
    }

    #[test]
    fn unchecked_call_transfer_stays_silent() {
        let text = "contract C {\nfunction f() public {\naddr.transfer(1);\n}\n}";
        assert!(
            only_rule(text, RuleId::UNCHECKED_CALL).is_empty(),
            "diags: {:?}",
            safety_of(text)
        );
    }

    #[test]
    fn reentrancy_write_after_call_warns() {
        let text =
            "contract C {\nuint public x;\nfunction f() public {\naddr.call(\"\");\nx = 1;\n}\n}";
        let found = only_rule(text, RuleId::REENTRANCY_PATTERN);
        assert_eq!(found.len(), 1, "diags: {found:?}");
        assert_eq!(found[0].rule.as_str(), "safety/reentrancy-pattern");
        assert_eq!(found[0].severity, forgels_lint::Severity::Warning);
        assert_eq!(
            found[0].message,
            "state write after external call risks reentrancy"
        );
    }

    #[test]
    fn reentrancy_write_without_call_stays_silent() {
        let text = "contract C {\nuint public x;\nfunction f() public {\nx = 1;\n}\n}";
        assert!(
            only_rule(text, RuleId::REENTRANCY_PATTERN).is_empty(),
            "diags: {:?}",
            safety_of(text)
        );
    }

    #[test]
    fn reentrancy_write_before_call_stays_silent() {
        let text =
            "contract C {\nuint public x;\nfunction f() public {\nx = 1;\naddr.call(\"\");\n}\n}";
        assert!(
            only_rule(text, RuleId::REENTRANCY_PATTERN).is_empty(),
            "diags: {:?}",
            safety_of(text)
        );
    }

    #[test]
    fn encode_packed_string_warns() {
        let text = "contract C {\nfunction f(string memory s) public {\nabi.encodePacked(s);\n}\n}";
        let found = only_rule(text, RuleId::ENCODE_PACKED_DYNAMIC);
        assert_eq!(found.len(), 1, "diags: {found:?}");
        assert_eq!(found[0].rule.as_str(), "safety/encode-packed-dynamic");
        assert_eq!(found[0].severity, forgels_lint::Severity::Warning);
        assert_eq!(
            found[0].message,
            "abi.encodePacked with dynamic types can collide, use abi.encode"
        );
    }

    #[test]
    fn encode_packed_fixed_bytes_stays_silent() {
        let text = "contract C {\nfunction f(bytes32 b) public {\nabi.encodePacked(b);\n}\n}";
        assert!(
            only_rule(text, RuleId::ENCODE_PACKED_DYNAMIC).is_empty(),
            "diags: {:?}",
            safety_of(text)
        );
    }

    #[test]
    fn payable_fallback_without_payable_warns() {
        let text = "contract C {\nfallback() external {}\n}";
        let found = only_rule(text, RuleId::PAYABLE_FALLBACK);
        assert_eq!(found.len(), 1, "diags: {found:?}");
        assert_eq!(found[0].rule.as_str(), "safety/payable-fallback");
        assert_eq!(found[0].severity, forgels_lint::Severity::Warning);
        assert_eq!(
            found[0].message,
            "fallback without payable and without receive cannot accept ether"
        );
    }

    #[test]
    fn payable_fallback_with_payable_stays_silent() {
        let text = "contract C {\nfallback() external payable {}\n}";
        assert!(
            only_rule(text, RuleId::PAYABLE_FALLBACK).is_empty(),
            "diags: {:?}",
            safety_of(text)
        );
    }

    #[test]
    fn payable_fallback_with_receive_stays_silent() {
        let text = "contract C {\nfallback() external {}\nreceive() external payable {}\n}";
        assert!(
            only_rule(text, RuleId::PAYABLE_FALLBACK).is_empty(),
            "diags: {:?}",
            safety_of(text)
        );
    }

    #[test]
    fn test_naming_helper_warns() {
        let text = "contract T is Test {\nfunction helper() public {}\n}";
        let found = naming_of(text);
        assert_eq!(found.len(), 1, "diags: {found:?}");
        assert_eq!(found[0].rule, RuleId::TEST_NAMING);
        assert_eq!(found[0].rule.as_str(), "foundry/test-naming");
        assert_eq!(found[0].severity, forgels_lint::Severity::Warning);
        assert_eq!(
            found[0].message,
            "test function 'helper' should start with test, invariant, or setUp"
        );
    }

    #[test]
    fn test_naming_prefixes_stay_silent() {
        let text = "contract T is Test {\nfunction testA() public {}\nfunction invariantB() public {}\nfunction setUp() public {}\n}";
        assert!(naming_of(text).is_empty(), "diags: {:?}", naming_of(text));
    }

    #[test]
    fn test_naming_plain_contract_stays_silent() {
        let text = "contract C {\nfunction helper() public {}\n}";
        assert!(naming_of(text).is_empty(), "diags: {:?}", naming_of(text));
    }
}
