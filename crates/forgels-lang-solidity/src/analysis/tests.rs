use super::{
    CompletionItem, Scope, SymbolKind, collect_symbols, complete_at, goto_definition, hover_at,
};
use crate::{SolidityLexer, SolidityParser};
use forgels_syntax::{Lexer, Span};

fn collect(text: &str) -> (String, Scope) {
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    (text.to_string(), collect_symbols(&file))
}

#[test]
fn resolves_state_var_from_function_body() {
    let text = "contract C {\nuint256 total;\nfunction f() public {\nuint x = 1;\n}\n}";
    let (_, scope) = collect(text);
    let at_body = text.find("uint x").expect("local") as u32;
    let total = scope.resolve("total", at_body).expect("found");
    assert_eq!(total.kind, SymbolKind::StateVar);
    assert_eq!(total.detail, "uint256 total");
}

#[test]
fn innermost_declaration_wins() {
    let text = "contract C {\nuint x;\nfunction f() public {\nuint x = 1;\nx;\n}\n}";
    let (_, scope) = collect(text);
    let at_use = text.rfind("\nx;\n").expect("use") as u32 + 1;
    let found = scope.resolve("x", at_use).expect("found");
    assert_eq!(found.kind, SymbolKind::LocalVar);
    let at_contract = 0;
    let outer = scope.resolve("x", at_contract).expect("found");
    assert_eq!(outer.kind, SymbolKind::StateVar);
}

#[test]
fn param_shadows_state_var() {
    let text = "contract C {\nuint amount;\nfunction f(uint amount) public {\n}\n}";
    let (_, scope) = collect(text);
    let at_body = text.find("public {").expect("body") as u32;
    let found = scope.resolve("amount", at_body).expect("found");
    assert_eq!(found.kind, SymbolKind::Param);
}

#[test]
fn function_detail_lists_signature() {
    let text = "contract C {\nfunction add(uint a, uint b) public pure returns (uint) {\n}\n}";
    let (_, scope) = collect(text);
    let found = scope.resolve("add", 0).expect("found");
    assert_eq!(found.kind, SymbolKind::Function);
    assert_eq!(found.detail, "function add(uint a, uint b) returns (uint)");
}

#[test]
fn contract_detail_lists_heritage() {
    let text = "abstract contract C is A, B {\n}";
    let (_, scope) = collect(text);
    let found = scope.resolve("C", 0).expect("found");
    assert_eq!(found.kind, SymbolKind::Contract);
    assert_eq!(found.detail, "contract C is A, B");
}

#[test]
fn visible_at_lists_inner_then_outer() {
    let text = "contract C {\nuint total;\nfunction f(uint a) public {\nuint x = 1;\n}\n}";
    let (_, scope) = collect(text);
    let at_local = text.find("uint x").expect("local") as u32;
    let names: Vec<&str> = scope
        .visible_at(at_local)
        .iter()
        .map(|symbol| symbol.name.as_str())
        .collect();
    assert!(names.contains(&"x"));
    assert!(names.contains(&"a"));
    assert!(names.contains(&"total"));
    assert!(names.contains(&"f"));
    assert!(names.contains(&"C"));
}

#[test]
fn unknown_name_resolves_to_none() {
    let text = "contract C {\nuint total;\n}";
    let (_, scope) = collect(text);
    assert!(scope.resolve("missing", 5).is_none());
}

#[test]
fn hover_shows_state_var_detail() {
    let text = "contract C {\nuint256 total;\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty());
    let at = text.find("total").expect("name") as u32;
    let hover = hover_at(&file, text, at).expect("hover");
    assert_eq!(hover.contents, "uint256 total");
}

#[test]
fn hover_resolves_usage_to_declaration() {
    let text = "contract C {\nuint256 total;\nfunction f() public {\ntotal;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty());
    let at = text.rfind("total").expect("use") as u32;
    let hover = hover_at(&file, text, at).expect("hover");
    assert_eq!(hover.contents, "uint256 total");
}

#[test]
fn hover_on_empty_position_returns_none() {
    let text = "contract C {\nuint256 total;\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty());
    let at_newline = text.find('\n').expect("newline") as u32;
    assert!(hover_at(&file, text, at_newline).is_none());
    let at_brace = text.find('{').expect("brace") as u32;
    assert!(hover_at(&file, text, at_brace).is_none());
}

#[test]
fn hover_on_undeclared_word_returns_none() {
    let text = "contract C {\nuint256 total;\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty());
    let at = text.find("uint256").expect("type") as u32;
    assert!(hover_at(&file, text, at).is_none());
}

fn complete_labels(text: &str, offset: u32) -> Vec<String> {
    let tokens = SolidityLexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    complete_at(&file, text, offset)
        .iter()
        .map(|item| item.label.clone())
        .collect()
}

#[test]
fn completion_filters_by_prefix() {
    let text = "contract C {\nuint256 total;\nuint256 tax;\naddress owner;\n}";
    let at = text.find("total").expect("name") as u32 + 3;
    let labels = complete_labels(text, at);
    assert!(labels.contains(&"total".to_string()));
    assert!(!labels.contains(&"tax".to_string()));
    assert!(!labels.contains(&"owner".to_string()));
}

#[test]
fn completion_lists_shadowed_name_once() {
    let text = "contract C {\nuint total;\nfunction f() public {\nuint total = 1;\nto\n}\n}";
    let at = text.find("\nto\n").expect("prefix") as u32 + 3;
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let items = complete_at(&file, text, at);
    let matches: Vec<&CompletionItem> = items.iter().filter(|item| item.label == "total").collect();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].detail, "uint total");
}

#[test]
fn completion_suggests_keywords_by_prefix() {
    let text = "contract C {\nfun";
    let at = text.len() as u32;
    let labels = complete_labels(text, at);
    assert!(labels.contains(&"function".to_string()));
}

#[test]
fn completion_hides_other_function_locals() {
    let text =
        "contract C {\nfunction f() public {\nuint hidden = 1;\n}\nfunction g() public {\n}\n}";
    let at = text.find("public {\n}\n}").expect("body") as u32 + 8;
    let labels = complete_labels(text, at);
    assert!(!labels.contains(&"hidden".to_string()));
    assert!(labels.contains(&"f".to_string()));
    assert!(labels.contains(&"g".to_string()));
}

fn goto_target(text: &str, offset: u32) -> Span {
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    goto_definition(&file, text, offset).expect("jump target")
}

#[test]
fn goto_jumps_usage_to_state_var() {
    let text = "contract C {\nuint256 total;\nfunction f() public {\ntotal;\n}\n}";
    let at_use = text.rfind("total").expect("use") as u32;
    let at_decl = text.find("total").expect("decl") as u32;
    let target = goto_target(text, at_use);
    assert!(target.start <= at_decl && at_decl < target.end);
}

#[test]
fn goto_prefers_shadowing_local() {
    let text = "contract C {\nuint x;\nfunction f() public {\nuint x = 1;\nx;\n}\n}";
    let at_use = text.rfind("\nx;\n").expect("use") as u32 + 1;
    let at_local = text.find("x = 1").expect("local") as u32;
    let target = goto_target(text, at_use);
    assert!(target.start <= at_local && at_local < target.end);
}

#[test]
fn goto_on_empty_returns_none() {
    let text = "contract C {\nuint256 total;\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty());
    let at_newline = text.find('\n').expect("newline") as u32;
    assert!(goto_definition(&file, text, at_newline).is_none());
}
