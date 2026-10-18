use super::forge::{ASSERT_TABLE, CONSOLE_TABLE, VM_TABLE};
use super::{
    CompletionItem, CompletionKind, Scope, SolidityAnalyzer, SymbolKind, collect_symbols,
    complete_at, complete_bare_imports, complete_import_paths, completion_item_kind, exports_of,
    goto_definition, hover_at, import_inner_span, import_path_at, imports_in,
};
use crate::{SolidityLexer, SolidityParser};
use forgels_core::{Analyzer, ProjectContext, Workspace, routing::Language};
use forgels_syntax::{AstNode, Lexer, Span};
use std::path::PathBuf;

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
fn completion_suggests_builtin_types_for_uin() {
    let text = "contract C {\nfunction f() public {\nuin";
    let at = text.len() as u32;
    let labels = complete_labels(text, at);
    for want in ["uint8", "uint128", "uint256"] {
        assert!(labels.contains(&want.to_string()), "labels: {labels:?}");
    }
}

#[test]
fn completion_suggests_builtin_types_for_byte() {
    let text = "contract C {\nfunction f() public {\nbyte";
    let at = text.len() as u32;
    let labels = complete_labels(text, at);
    for want in ["bytes1", "bytes32"] {
        assert!(labels.contains(&want.to_string()), "labels: {labels:?}");
    }
}

#[test]
fn completion_suggests_builtin_types_for_uint2() {
    let text = "contract C {\nfunction f() public {\nuint2";
    let at = text.len() as u32;
    let labels = complete_labels(text, at);
    assert!(
        labels.contains(&"uint256".to_string()),
        "labels: {labels:?}"
    );
}

#[test]
fn completion_builtin_type_kind_and_detail() {
    let text = "contract C {\nfunction f() public {\nuin";
    let at = text.len() as u32;
    let tokens = SolidityLexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    let hit = complete_at(&file, text, at)
        .into_iter()
        .find(|item| item.label == "uint8")
        .expect("builtin hit");
    assert_eq!(hit.detail, "builtin type");
    assert_eq!(hit.kind, CompletionKind::BuiltinType);
    assert_eq!(completion_item_kind(hit.kind).value(), 14);
}

#[test]
fn completion_builtin_types_have_no_duplicates() {
    let text = "contract C {\nuint256 uintVar;\nfunction f() public {\nuint";
    let at = text.len() as u32;
    let tokens = SolidityLexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    let labels: Vec<String> = complete_at(&file, text, at)
        .iter()
        .map(|item| item.label.clone())
        .collect();
    assert!(labels.contains(&"uint8".to_string()), "labels: {labels:?}");
    assert!(
        labels.contains(&"uintVar".to_string()),
        "labels: {labels:?}"
    );
    let mut seen = std::collections::HashSet::new();
    for label in &labels {
        assert!(seen.insert(label.clone()), "duplicate: {label}");
    }
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

fn open_workspace(files: &[(&str, &str)]) -> Workspace {
    let mut workspace = Workspace::new();
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

fn complete_multi(
    workspace: &Workspace,
    path: &str,
    text: &str,
    prefix: &str,
) -> Vec<(String, String)> {
    let owned = text.to_string();
    let buf = PathBuf::from(path);
    let at = owned.rfind(prefix).expect("prefix") as u32 + prefix.len() as u32;
    let ctx = ProjectContext::new(buf.as_path(), &owned, at, workspace);
    SolidityAnalyzer
        .complete(&ctx)
        .into_iter()
        .map(|item| (item.label, item.detail))
        .collect()
}

#[test]
fn member_completion_resolves_receiver_at_scope_end() {
    // Typing `owner.` at EOF: the contract scope ends exactly at the
    // cursor, so resolve must treat the end edge as inside.
    let text = "// SPDX-License-Identifier: UNLICENSED\npragma solidity ^0.8.34;\nstruct CounterStruct {\nuint x;\n}\ncontract Counter {\nuint public x;\naddress public owner;\nCounterStruct public counterStruct;\nevent Increment(uint by);\nevent Decrement(uint by);\nfunction inc() public {\nx++;\nowner.";
    let workspace = open_workspace(&[("/repo/Counter.sol", text)]);
    let owned = text.to_string();
    let buf = PathBuf::from("/repo/Counter.sol");
    let at = owned.len() as u32;
    let ctx = ProjectContext::new(buf.as_path(), &owned, at, &workspace);
    let out = SolidityAnalyzer.complete(&ctx);
    let labels: Vec<&str> = out.iter().map(|item| item.label.as_str()).collect();
    for member in [
        "balance",
        "code",
        "codehash",
        "call",
        "delegatecall",
        "staticcall",
    ] {
        assert!(labels.contains(&member), "{member} missing: {labels:?}");
    }
}

#[test]
fn scope_resolve_hits_end_edge() {
    let text = "contract C {\nuint256 total;\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let scope = super::collect_symbols(&file);
    let at_decl = text.find("total").expect("decl") as u32;
    let found = scope.resolve("total", at_decl).expect("found");
    assert_eq!(found.detail, "uint256 total");
    // Cursor exactly at the contract end edge still resolves inside.
    let end = file.syntax().span.end;
    let edge = scope.resolve("total", end).expect("edge resolves");
    assert_eq!(edge.detail, "uint256 total");
}

#[test]
fn completion_suggests_state_var_with_modifiers() {
    // Modifiers between type and name must not drop the declaration.
    for modifier in [
        "public",
        "private",
        "internal",
        "constant",
        "immutable",
        "override",
        "transient",
    ] {
        let text = format!(
            "contract HelloWorld {{\nPerson[] {modifier} people;\nfunction greet2(string memory a) public {{\npeo"
        );
        let workspace = open_workspace(&[("/repo/C.sol", text.as_str())]);
        let items = complete_multi(&workspace, "/repo/C.sol", &text, "peo");
        let labels: Vec<&str> = items.iter().map(|item| item.0.as_str()).collect();
        assert!(labels.contains(&"people"), "{modifier}: {labels:?}");
    }
}

#[test]
fn state_var_name_skips_modifiers() {
    let text = "contract C {\nPerson[] public constant people;\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let contracts = file.contracts();
    assert_eq!(contracts.len(), 1);
    let vars = contracts[0].state_vars();
    assert_eq!(vars.len(), 1);
    assert_eq!(vars[0].name().as_deref(), Some("people"));
}

#[test]
fn completion_suggests_symbols_when_file_has_errors() {
    // Typing state: incomplete contract, partial word, no closing brace.
    let text = "contract C {\nuint256 total;\ntot";
    let workspace = open_workspace(&[("/repo/C.sol", text)]);
    let items = complete_multi(&workspace, "/repo/C.sol", text, "tot");
    let labels: Vec<&str> = items.iter().map(|item| item.0.as_str()).collect();
    assert!(labels.contains(&"total"), "labels: {labels:?}");
    assert!(!labels.iter().any(|label| label.is_empty()));
    // Keywords still suggest while the file is broken.
    let keyword_text = "contract C {\nuint256 total;\nfun";
    let keyword_workspace = open_workspace(&[("/repo/C.sol", keyword_text)]);
    let keyword_items = complete_multi(&keyword_workspace, "/repo/C.sol", keyword_text, "fun");
    let keyword_labels: Vec<&str> = keyword_items.iter().map(|item| item.0.as_str()).collect();
    assert!(
        keyword_labels.contains(&"function"),
        "labels: {keyword_labels:?}"
    );
}

#[test]
fn hover_resolves_word_when_file_has_errors() {
    let text = "contract C {\nuint256 total;\ntotal";
    let workspace = open_workspace(&[("/repo/C.sol", text)]);
    let owned = text.to_string();
    let buf = PathBuf::from("/repo/C.sol");
    let at = owned.rfind("total").expect("use") as u32;
    let ctx = ProjectContext::new(buf.as_path(), &owned, at, &workspace);
    let found = SolidityAnalyzer.hover(&ctx).expect("hover");
    assert_eq!(found.contents, "uint256 total");
}

#[test]
fn goto_resolves_word_when_file_has_errors() {
    let text = "contract C {\nuint256 total;\ntotal";
    let workspace = open_workspace(&[("/repo/C.sol", text)]);
    let owned = text.to_string();
    let buf = PathBuf::from("/repo/C.sol");
    let at = owned.rfind("total").expect("use") as u32;
    let ctx = ProjectContext::new(buf.as_path(), &owned, at, &workspace);
    let target = SolidityAnalyzer.goto_location(&ctx).expect("goto");
    assert_eq!(target.path, buf);
    let decl = text.find("total").expect("decl") as u32;
    assert!(target.span.start <= decl && decl < target.span.end);
}

#[test]
fn exports_of_lists_contracts_and_functions_without_locals() {
    let text = "contract Vault {\nuint256 total;\nfunction withdraw(uint a) public {}\nfunction deposit() public {\nuint x = 1;\n}\n}";
    let items = exports_of(text);
    let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
    assert!(labels.contains(&"Vault"));
    assert!(labels.contains(&"withdraw"));
    assert!(labels.contains(&"deposit"));
    assert!(!labels.contains(&"total"));
    assert!(!labels.contains(&"x"));
    assert!(!labels.contains(&"a"));
}

#[test]
fn exports_of_lists_all_eight_kinds() {
    let text = "struct Person {\nuint age;\n}\nenum Color {\nRed\n}\nerror Oops();\nevent Transfer(address a);\ntype MyNumber is uint256;\ncontract Vault {\nfunction withdraw(uint a) public {}\nmodifier onlyOwner() { _; }\n}";
    let items = exports_of(text);
    let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
    for want in [
        "Person",
        "Color",
        "Oops",
        "Transfer",
        "MyNumber",
        "Vault",
        "withdraw",
        "onlyOwner",
    ] {
        assert!(labels.contains(&want), "labels: {labels:?}");
    }
    let detail_of = |name: &str| {
        items
            .iter()
            .find(|item| item.label == name)
            .expect("hit")
            .detail
            .clone()
    };
    assert_eq!(detail_of("Person"), "struct Person");
    assert_eq!(detail_of("Color"), "enum Color");
    assert_eq!(detail_of("Oops"), "error Oops");
    assert_eq!(detail_of("Transfer"), "event Transfer");
    assert_eq!(detail_of("MyNumber"), "type MyNumber is uint256");
    assert!(detail_of("Vault").starts_with("contract Vault"));
    assert!(detail_of("withdraw").starts_with("function withdraw"));
    assert_eq!(detail_of("onlyOwner"), "modifier onlyOwner");
    assert!(!labels.contains(&"age"));
    assert!(!labels.contains(&"Red"));
    assert!(!labels.contains(&"a"));
}

#[test]
fn dep_completion_includes_origin() {
    let b = "contract Vault {\nfunction withdraw(uint a) public {}\n}";
    let a = "import \"./B.sol\";\ncontract A {\nfunction f() public {\nwith\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    let found = complete_multi(&workspace, "/A.sol", a, "with");
    let hit = found
        .iter()
        .find(|(label, _)| label == "withdraw")
        .expect("dep hit");
    assert!(hit.1.contains("from B.sol"), "detail: {}", hit.1);
}

#[test]
fn completion_kind_maps_to_lsp_numbers() {
    use super::SymbolKind as Kind;
    let cases = [
        (CompletionKind::Symbol(Kind::Contract), 7),
        (CompletionKind::Symbol(Kind::Function), 3),
        (CompletionKind::Symbol(Kind::StateVar), 6),
        (CompletionKind::Symbol(Kind::LocalVar), 6),
        (CompletionKind::Symbol(Kind::Param), 6),
        (CompletionKind::Symbol(Kind::Field), 5),
        (CompletionKind::Symbol(Kind::Struct), 22),
        (CompletionKind::Symbol(Kind::Enum), 13),
        (CompletionKind::Symbol(Kind::EnumMember), 20),
        (CompletionKind::Keyword, 14),
        (CompletionKind::Path, 17),
        (CompletionKind::Package, 19),
    ];
    for (kind, want) in cases {
        assert_eq!(completion_item_kind(kind).value(), want);
    }
}

#[test]
fn dep_completion_withdraw_carries_function_kind() {
    let b = "contract Vault {\nfunction withdraw(uint a) public {}\n}";
    let a = "import \"./B.sol\";\ncontract A {\nfunction f() public {\nwith\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    let owned = a.to_string();
    let buf = PathBuf::from("/A.sol");
    let at = owned.rfind("with").expect("prefix") as u32 + 4;
    let ctx = ProjectContext::new(buf.as_path(), &owned, at, &workspace);
    let hit = SolidityAnalyzer
        .complete(&ctx)
        .into_iter()
        .find(|item| item.label == "withdraw")
        .expect("dep hit");
    assert_eq!(hit.kind.value(), 3);
}

#[test]
fn scope_completion_carries_kind() {
    let text = "contract C {\nuint256 total;\nfunction withdraw() public {}\nwith\n}";
    let workspace = open_workspace(&[("/repo/C.sol", text)]);
    let owned = text.to_string();
    let buf = PathBuf::from("/repo/C.sol");
    let at = owned.rfind("with").expect("prefix") as u32 + 4;
    let ctx = ProjectContext::new(buf.as_path(), &owned, at, &workspace);
    let hit = SolidityAnalyzer
        .complete(&ctx)
        .into_iter()
        .find(|item| item.label == "withdraw")
        .expect("scope hit");
    assert_eq!(hit.kind.value(), 3);
}

#[test]
fn local_symbol_shadows_dep() {
    let b = "contract Vault {\nfunction withdraw(uint a) public {}\n}";
    let a = "import \"./B.sol\";\ncontract A {\nfunction withdraw() public {\nwith\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    let found = complete_multi(&workspace, "/A.sol", a, "with");
    let hits: Vec<&(String, String)> = found
        .iter()
        .filter(|(label, _)| label == "withdraw")
        .collect();
    assert_eq!(hits.len(), 1);
    assert!(!hits[0].1.contains("from"), "local wins: {}", hits[0].1);
}

#[test]
fn transitive_dep_completion() {
    let c = "contract C {\nfunction settle() public {}\n}";
    let b = "import \"./C.sol\";\ncontract B {\n}";
    let a = "import \"./B.sol\";\ncontract A {\nfunction f() public {\nset\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b), ("/C.sol", c)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    workspace.set_file_deps(PathBuf::from("/B.sol"), vec![PathBuf::from("/C.sol")]);
    let found = complete_multi(&workspace, "/A.sol", a, "set");
    let hit = found
        .iter()
        .find(|(label, _)| label == "settle")
        .expect("transitive hit");
    assert!(hit.1.contains("from C.sol"), "detail: {}", hit.1);
}

#[test]
fn untracked_dep_is_skipped() {
    let a = "import \"./Missing.sol\";\ncontract A {\nfunction f() public {\nwith\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/Missing.sol")]);
    let found = complete_multi(&workspace, "/A.sol", a, "with");
    assert!(!found.iter().any(|(label, _)| label == "withdraw"));
}

#[test]
fn cyclic_deps_terminate() {
    let b = "import \"./A.sol\";\ncontract B {\nfunction beacon() public {}\n}";
    let a = "import \"./B.sol\";\ncontract A {\nfunction f() public {\nbea\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    workspace.set_file_deps(PathBuf::from("/B.sol"), vec![PathBuf::from("/A.sol")]);
    let found = complete_multi(&workspace, "/A.sol", a, "bea");
    assert!(found.iter().any(|(label, _)| label == "beacon"));
}

fn hover_multi(
    workspace: &Workspace,
    path: &str,
    text: &str,
    needle: &str,
) -> Option<forgels_core::HoverData> {
    let owned = text.to_string();
    let buf = PathBuf::from(path);
    let at = owned.rfind(needle).expect("needle") as u32;
    let ctx = ProjectContext::new(buf.as_path(), &owned, at, workspace);
    SolidityAnalyzer.hover(&ctx)
}

#[test]
fn dep_hover_includes_origin() {
    let b = "contract Vault {\nfunction withdraw(uint a) public {}\n}";
    let a = "import \"./B.sol\";\ncontract A {\nfunction f() public {\nwithdraw;\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    let hit = hover_multi(&workspace, "/A.sol", a, "withdraw").expect("dep hover");
    assert!(
        hit.contents.contains("function withdraw"),
        "contents: {}",
        hit.contents
    );
    assert!(
        hit.contents.contains("from B.sol"),
        "contents: {}",
        hit.contents
    );
    let at = a.rfind("withdraw").expect("word") as u32;
    assert!(hit.span.contains(at));
}

#[test]
fn local_hover_shadows_dep() {
    let b = "contract Vault {\nfunction withdraw(uint a) public {}\n}";
    let a = "import \"./B.sol\";\ncontract A {\nfunction withdraw() public {\nwithdraw;\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    let hit = hover_multi(&workspace, "/A.sol", a, "withdraw").expect("local hover");
    assert!(
        !hit.contents.contains("from"),
        "local wins: {}",
        hit.contents
    );
    assert_eq!(hit.contents, "function withdraw()");
}

#[test]
fn transitive_dep_hover() {
    let c = "contract C {\nfunction settle() public {}\n}";
    let b = "import \"./C.sol\";\ncontract B {\n}";
    let a = "import \"./B.sol\";\ncontract A {\nfunction f() public {\nsettle;\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b), ("/C.sol", c)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    workspace.set_file_deps(PathBuf::from("/B.sol"), vec![PathBuf::from("/C.sol")]);
    let hit = hover_multi(&workspace, "/A.sol", a, "settle").expect("transitive hover");
    assert!(
        hit.contents.contains("from C.sol"),
        "contents: {}",
        hit.contents
    );
}

#[test]
fn untracked_dep_hover_returns_none() {
    let a = "import \"./Missing.sol\";\ncontract A {\nfunction f() public {\nwithdraw;\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/Missing.sol")]);
    assert!(hover_multi(&workspace, "/A.sol", a, "withdraw").is_none());
}

#[test]
fn cyclic_dep_hover_terminates() {
    let b = "import \"./A.sol\";\ncontract B {\nfunction beacon() public {}\n}";
    let a = "import \"./B.sol\";\ncontract A {\nfunction f() public {\nbeacon;\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    workspace.set_file_deps(PathBuf::from("/B.sol"), vec![PathBuf::from("/A.sol")]);
    let hit = hover_multi(&workspace, "/A.sol", a, "beacon").expect("cycle hover");
    assert!(
        hit.contents.contains("from B.sol"),
        "contents: {}",
        hit.contents
    );
}

#[test]
fn broken_dep_hover_still_works() {
    let b = "contract Vault {\nfunction withdraw(uint a) public {}\n}\n@@@ broken";
    let a = "import \"./B.sol\";\ncontract A {\nfunction f() public {\nwithdraw;\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    let hit = hover_multi(&workspace, "/A.sol", a, "withdraw").expect("broken dep hover");
    assert!(
        hit.contents.contains("function withdraw"),
        "contents: {}",
        hit.contents
    );
    assert!(
        hit.contents.contains("from B.sol"),
        "contents: {}",
        hit.contents
    );
}

fn goto_multi(
    workspace: &Workspace,
    path: &str,
    text: &str,
    needle: &str,
) -> Option<forgels_core::GotoTarget> {
    let owned = text.to_string();
    let buf = PathBuf::from(path);
    let at = owned.rfind(needle).expect("needle") as u32;
    let ctx = ProjectContext::new(buf.as_path(), &owned, at, workspace);
    SolidityAnalyzer.goto_location(&ctx)
}

#[test]
fn dep_goto_jumps_to_other_file() {
    let b = "contract Vault {\nfunction withdraw(uint a) public {}\n}";
    let a = "import \"./B.sol\";\ncontract A {\nfunction f() public {\nwithdraw;\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    let hit = goto_multi(&workspace, "/A.sol", a, "withdraw").expect("dep goto");
    assert_eq!(hit.path, PathBuf::from("/B.sol"));
    let want = b.find("withdraw").expect("decl") as u32;
    assert!(hit.span.contains(want));
}

#[test]
fn local_goto_shadows_dep() {
    let b = "contract Vault {\nfunction withdraw(uint a) public {}\n}";
    let a = "import \"./B.sol\";\ncontract A {\nfunction withdraw() public {\nwithdraw;\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    let hit = goto_multi(&workspace, "/A.sol", a, "withdraw").expect("local goto");
    assert_eq!(hit.path, PathBuf::from("/A.sol"));
}

#[test]
fn transitive_dep_goto() {
    let c = "contract C {\nfunction settle() public {}\n}";
    let b = "import \"./C.sol\";\ncontract B {\n}";
    let a = "import \"./B.sol\";\ncontract A {\nfunction f() public {\nsettle;\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b), ("/C.sol", c)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    workspace.set_file_deps(PathBuf::from("/B.sol"), vec![PathBuf::from("/C.sol")]);
    let hit = goto_multi(&workspace, "/A.sol", a, "settle").expect("transitive goto");
    assert_eq!(hit.path, PathBuf::from("/C.sol"));
    let want = c.find("settle").expect("decl") as u32;
    assert!(hit.span.contains(want));
}

#[test]
fn untracked_dep_goto_returns_none() {
    let a = "import \"./Missing.sol\";\ncontract A {\nfunction f() public {\nwithdraw;\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/Missing.sol")]);
    assert!(goto_multi(&workspace, "/A.sol", a, "withdraw").is_none());
}

#[test]
fn cyclic_dep_goto_terminates() {
    let b = "import \"./A.sol\";\ncontract B {\nfunction beacon() public {}\n}";
    let a = "import \"./B.sol\";\ncontract A {\nfunction f() public {\nbeacon;\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    workspace.set_file_deps(PathBuf::from("/B.sol"), vec![PathBuf::from("/A.sol")]);
    let hit = goto_multi(&workspace, "/A.sol", a, "beacon").expect("cycle goto");
    assert_eq!(hit.path, PathBuf::from("/B.sol"));
    let want = b.find("beacon").expect("decl") as u32;
    assert!(hit.span.contains(want));
}

#[test]
fn broken_dep_goto_still_works() {
    let b = "contract Vault {\nfunction withdraw(uint a) public {}\n}\n@@@ broken";
    let a = "import \"./B.sol\";\ncontract A {\nfunction f() public {\nwithdraw;\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    let hit = goto_multi(&workspace, "/A.sol", a, "withdraw").expect("broken dep goto");
    assert_eq!(hit.path, PathBuf::from("/B.sol"));
    let want = b.find("withdraw").expect("decl") as u32;
    assert!(hit.span.contains(want));
}

fn diagnostics_of(text: &str) -> Vec<forgels_core::DiagnosticData> {
    SolidityAnalyzer.diagnostics(text)
}

#[test]
fn too_few_args_reports_exact_message() {
    let text = "contract C {\nfunction greet(uint a, uint b) public { a; b; }\nfunction f() public {\ngreet(1);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].message,
        "this function 'greet' takes 2 arguments but 1 supplied"
    );
    let covered = &text[found[0].span.start as usize..found[0].span.end as usize];
    assert!(
        covered.starts_with("greet") && covered.ends_with(')'),
        "span: {covered}"
    );
}

#[test]
fn too_many_args_reports_exact_message() {
    let text = "contract C {\nfunction greet(uint a) public { a; }\nfunction f() public {\ngreet(1, 2);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].message,
        "this function 'greet' takes 1 arguments but 2 supplied"
    );
}

#[test]
fn correct_arg_count_has_no_diagnostic() {
    let text = "contract C {\nfunction greet(uint a, uint b) public { a; b; }\nfunction f() public {\ngreet(1, 2);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn unknown_callee_reports_unresolved() {
    let text = "contract C {\nfunction f() public {\nmissing(1, 2, 3);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(found[0].message, "unresolved identifier 'missing'");
    let covered = &text[found[0].span.start as usize..found[0].span.end as usize];
    assert_eq!(covered, "missing");
}

#[test]
fn unknown_member_receiver_reports_receiver_only() {
    let text = "contract C {\nfunction greet(uint a, uint b) public { a; b; }\nfunction f() public {\nobj.greet(1);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(found[0].message, "unresolved identifier 'obj'");
    let covered = &text[found[0].span.start as usize..found[0].span.end as usize];
    assert_eq!(covered, "obj");
}

#[test]
fn unknown_bare_usage_reports_name_span() {
    let text = "contract C {\nfunction f() public {\nuint x = 1;\nx;\nmissing;\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(found[0].message, "unresolved identifier 'missing'");
    let at = text.find("missing;").expect("use") as u32;
    assert_eq!(found[0].span.start, at);
    assert_eq!(found[0].span.end, at + 7);
}

#[test]
fn unknown_known_names_stay_silent() {
    let text =
        "contract C {\nuint total;\nfunction f(uint a) public {\nuint x = 1;\ntotal;\na;\nx;\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn unknown_decl_names_stay_silent() {
    let text = "contract C {\nuint total;\nfunction f() public {}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn unknown_this_member_stays_silent() {
    let text = "contract C {\nfunction f() public {\nthis.missing;\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn unknown_emit_has_single_event_error() {
    let text = "contract C {\nfunction f() public {\nemit Missing(1);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(found[0].message, "unresolved event 'Missing'");
}

#[test]
fn unknown_dep_symbol_stays_silent() {
    let dep = "contract Vault {\nfunction withdraw(uint a) public {}\n}";
    let cur = "import \"./B.sol\";\ncontract A {\nfunction f() public {\nwithdraw;\n}\n}";
    let mut workspace = open_workspace(&[("/B.sol", dep), ("/A.sol", cur)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    assert!(diagnostics_multi(&workspace, "/A.sol", cur).is_empty());
}

#[test]
fn unknown_forge_assert_stays_silent_in_test() {
    let text = "contract T is Test {\nfunction f() public {\nassertEq(1, 2);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn unknown_global_receivers_stay_silent() {
    let text = "contract C {\nfunction f() public {\nmsg;\nblock;\nabi;\nvm;\nconsole;\nthis;\nsuper;\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn unknown_global_functions_stay_silent() {
    let text = "contract C {\nfunction f() public {\nrequire;\nrevert;\nkeccak256;\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn unknown_typing_prefix_stays_silent() {
    let text = "contract C {\nuint256 total;\nfunction f() public {\ntot\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn unknown_past_known_still_errors() {
    let text = "contract C {\nuint256 total;\nfunction f() public {\ntotalX;\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(found[0].message, "unresolved identifier 'totalX'");
}

#[test]
fn unknown_assembly_idents_stay_silent() {
    let text = "contract C {\nfunction f() public {\nassembly {\nlet x := bogus\n}\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn unknown_trivia_words_stay_silent() {
    let text = "contract C {\nfunction f() public {}\n}\n// missing\n/* bogus */";
    assert!(diagnostics_of(text).is_empty());
    let with_string =
        "contract C {\nfunction f() public {\nstring memory s = \"missing\";\ns;\n}\n}";
    assert!(diagnostics_of(with_string).is_empty());
}

#[test]
fn unknown_multiple_errors_sort_by_span() {
    let text = "contract C {\nfunction f() public {\nzzz;\naaa;\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 2, "diags: {found:?}");
    assert!(found[0].span.start < found[1].span.start);
    assert_eq!(found[0].message, "unresolved identifier 'zzz'");
    assert_eq!(found[1].message, "unresolved identifier 'aaa'");
}

#[test]
fn member_call_correct_count_stays_silent() {
    let text = "contract Hello {\nfunction greet(uint a, uint b) public { a; b; }\n}\ncontract C {\nfunction f() public {\nHello h;\nh.greet(1, 2);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn member_call_too_few_reports_exact_message() {
    let text = "contract Hello {\nfunction greet(uint a, uint b) public { a; b; }\n}\ncontract C {\nfunction f() public {\nHello h;\nh.greet(1);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this function 'greet' takes 2 arguments but 1 supplied"
    );
    let covered = &text[found[0].span.start as usize..found[0].span.end as usize];
    assert!(
        covered.starts_with("h.greet") && covered.ends_with(')'),
        "span: {covered}"
    );
}

#[test]
fn member_call_too_many_reports_exact_message() {
    let text = "contract Hello {\nfunction greet(uint a) public { a; }\n}\ncontract C {\nfunction f() public {\nHello h;\nh.greet(1, 2);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this function 'greet' takes 1 arguments but 2 supplied"
    );
}

#[test]
fn member_plain_access_stays_silent() {
    let text = "contract Hello {\nfunction greet(uint a) public { a; }\n}\ncontract C {\nfunction f() public {\nHello h;\nh.greet;\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn member_empty_call_reports_zero_supplied() {
    let text = "contract Hello {\nfunction greet(uint a) public { a; }\n}\ncontract C {\nfunction f() public {\nHello h;\nh.greet();\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this function 'greet' takes 1 arguments but 0 supplied"
    );
}

#[test]
fn member_this_call_reports_exact_message() {
    let text = "contract C {\nfunction greet(uint a, uint b) public { a; b; }\nfunction f() public {\nthis.greet(1);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this function 'greet' takes 2 arguments but 1 supplied"
    );
}

#[test]
fn member_this_correct_count_stays_silent() {
    let text = "contract C {\nfunction greet(uint a) public { a; }\nfunction f() public {\nthis.greet(1);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn member_inherited_arity_uses_base_signature() {
    let text = "contract B {\nfunction greet(uint a, uint b) public { a; b; }\n}\ncontract D is B {\nfunction f() public {\nthis.greet(1);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this function 'greet' takes 2 arguments but 1 supplied"
    );
}

#[test]
fn member_shadow_uses_derived_signature() {
    let text = "contract B {\nfunction greet(uint a, uint b) public { a; b; }\n}\ncontract D is B {\nfunction greet(uint a) public { a; }\nfunction f() public {\nthis.greet(1, 2);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this function 'greet' takes 1 arguments but 2 supplied"
    );
}

#[test]
fn member_builtin_calls_stay_silent() {
    let text = "contract T is Test {\nfunction f() public {\nvm.prank(a, b, c);\nvm.stopPrank(1);\nconsole.log(a, b);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn member_struct_field_call_stays_silent() {
    let text = "struct Person {\nuint age;\n}\ncontract C {\nfunction f() public {\nPerson p;\np.age(1);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn member_unknown_function_stays_silent() {
    let text = "contract Hello {\nfunction greet(uint a) public { a; }\n}\ncontract C {\nfunction f() public {\nHello h;\nh.missing(1, 2);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn member_dep_arity_reports() {
    let dep = "contract Hello {\nfunction greet(uint a, uint b) public { a; b; }\n}";
    let cur =
        "import \"./H.sol\";\ncontract C {\nfunction f() public {\nHello h;\nh.greet(1);\n}\n}";
    let mut workspace = open_workspace(&[("/H.sol", dep), ("/C.sol", cur)]);
    workspace.set_file_deps(PathBuf::from("/C.sol"), vec![PathBuf::from("/H.sol")]);
    let found = diagnostics_multi(&workspace, "/C.sol", cur);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this function 'greet' takes 2 arguments but 1 supplied"
    );
}

#[test]
fn member_dep_correct_count_stays_silent() {
    let dep = "contract Hello {\nfunction greet(uint a) public { a; }\n}";
    let cur =
        "import \"./H.sol\";\ncontract C {\nfunction f() public {\nHello h;\nh.greet(1);\n}\n}";
    let mut workspace = open_workspace(&[("/H.sol", dep), ("/C.sol", cur)]);
    workspace.set_file_deps(PathBuf::from("/C.sol"), vec![PathBuf::from("/H.sol")]);
    assert!(diagnostics_multi(&workspace, "/C.sol", cur).is_empty());
}

#[test]
fn member_dep_inherited_arity_uses_base() {
    let base = "contract B {\nfunction greet(uint a, uint b) public { a; b; }\n}";
    let cur = "import \"./B.sol\";\ncontract D is B {\nfunction f() public {\nthis.greet(1);\n}\n}";
    let mut workspace = open_workspace(&[("/B.sol", base), ("/D.sol", cur)]);
    workspace.set_file_deps(PathBuf::from("/D.sol"), vec![PathBuf::from("/B.sol")]);
    let found = diagnostics_multi(&workspace, "/D.sol", cur);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this function 'greet' takes 2 arguments but 1 supplied"
    );
}

#[test]
fn super_and_constructor_calls_stay_silent() {
    let text = "contract C {\nfunction greet(uint a, uint b) public { a; b; }\nfunction f() public {\nsuper.greet(1);\nsuper(1);\nconstructor(1);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn overload_uses_first_definition() {
    let text = "contract C {\nfunction greet(uint a) public { a; }\nfunction greet(uint a, uint b) public { a; b; }\nfunction f() public {\ngreet(1, 2);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].message,
        "this function 'greet' takes 1 arguments but 2 supplied"
    );
}

fn parse_recovered(text: &str) -> crate::SolidityFile {
    let tokens = SolidityLexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    file
}

#[test]
fn import_path_hits_inside_quotes() {
    let text = "import \"./B.sol\";\ncontract A {}";
    let file = parse_recovered(text);
    let at = text.find("./B").expect("path") as u32 + 1;
    assert!(import_path_at(&file, text, at).is_some());
}

#[test]
fn import_path_fallback_covers_incomplete_row() {
    let text = "import \"./B";
    let file = parse_recovered(text);
    let at = text.len() as u32;
    assert!(import_path_at(&file, text, at).is_some());
}

#[test]
fn import_path_misses_code_position() {
    let text = "import \"./B.sol\";\ncontract A {}";
    let file = parse_recovered(text);
    let at = text.find("contract").expect("code") as u32;
    assert!(import_path_at(&file, text, at).is_none());
}

fn fresh_sol_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("temp root");
    for rel in ["A.sol", "B.sol", "sub/C.sol", "skip.txt"] {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("parent");
        }
        std::fs::write(&path, "contract X {}").expect("write");
    }
    root
}

#[test]
fn import_paths_list_relative_and_filter_prefix() {
    let root = fresh_sol_root("forgels-sol-import-list");
    let current = root.join("A.sol");
    let all = complete_import_paths(&current, std::slice::from_ref(&root), "");
    let labels: Vec<&str> = all.iter().map(|item| item.label.as_str()).collect();
    assert!(labels.contains(&"./B.sol"));
    assert!(labels.contains(&"./sub/C.sol"));
    assert!(!labels.iter().any(|label| label.contains("A.sol")));
    assert!(all.iter().all(|item| item.detail == "file"));
    let filtered = complete_import_paths(&current, std::slice::from_ref(&root), "./B");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].label, "./B.sol");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn analyzer_complete_prefers_paths_in_import() {
    let root = fresh_sol_root("forgels-sol-import-branch");
    let current = root.join("A.sol");
    let text = "import \"./B";
    let mut workspace = Workspace::new();
    workspace.set_roots(vec![root.clone()]);
    workspace
        .open(current.clone(), Language::Solidity, text.to_string(), 1)
        .expect("open works");
    let owned = text.to_string();
    let ctx = ProjectContext::new(&current, &owned, owned.len() as u32, &workspace);
    let found = SolidityAnalyzer.complete(&ctx);
    assert!(!found.is_empty());
    assert!(found.iter().all(|item| item.detail == "file"));
    assert!(found.iter().all(|item| item.kind.value() == 17));
    assert!(found.iter().any(|item| item.label == "./B.sol"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn completion_suggests_contract_struct_by_prefix() {
    let text = "contract C {\nstruct Person {\nuint age;\n}\nfunction f() public {\nPer\n}\n}";
    let at = text.rfind("Per").expect("prefix") as u32 + 3;
    let labels = complete_labels(text, at);
    assert!(labels.contains(&"Person".to_string()), "labels: {labels:?}");
}

#[test]
fn completion_suggests_contract_enum_by_prefix() {
    let text = "contract C {\nenum Color {\nRed\n}\nfunction f() public {\nCol\n}\n}";
    let at = text.rfind("Col").expect("prefix") as u32 + 3;
    let labels = complete_labels(text, at);
    assert!(labels.contains(&"Color".to_string()), "labels: {labels:?}");
}

#[test]
fn typing_state_without_closing_brace_still_suggests_struct() {
    let text = "contract C {\nstruct Person {\nuint age;\nPer";
    let workspace = open_workspace(&[("/repo/C.sol", text)]);
    let items = complete_multi(&workspace, "/repo/C.sol", text, "Per");
    let labels: Vec<&str> = items.iter().map(|item| item.0.as_str()).collect();
    assert!(labels.contains(&"Person"), "labels: {labels:?}");
    let hit = items
        .iter()
        .find(|(label, _)| label == "Person")
        .expect("struct hit");
    assert_eq!(hit.1, "struct Person");
}

#[test]
fn file_struct_visible_inside_contract() {
    let text = "struct Person {\nuint age;\n}\ncontract C {\nfunction f() public {\nPer\n}\n}";
    let at = text.rfind("Per").expect("prefix") as u32 + 3;
    let labels = complete_labels(text, at);
    assert!(labels.contains(&"Person".to_string()), "labels: {labels:?}");
}

#[test]
fn contract_struct_stays_inside_its_contract() {
    let text = "contract A {\nstruct Person {\nuint age;\n}\n}\ncontract B {\nfunction f() public {\nPer\n}\n}";
    let at = text.rfind("Per").expect("prefix") as u32 + 3;
    let labels = complete_labels(text, at);
    assert!(
        !labels.contains(&"Person".to_string()),
        "labels: {labels:?}"
    );
}

#[test]
fn exports_include_struct_and_enum() {
    let text = "struct Person {\nuint age;\n}\nenum Color {\nRed\n}\ncontract C {\n}";
    let items = exports_of(text);
    let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
    assert!(labels.contains(&"Person"), "labels: {labels:?}");
    assert!(labels.contains(&"Color"), "labels: {labels:?}");
    let person = items
        .iter()
        .find(|item| item.label == "Person")
        .expect("person");
    assert_eq!(person.detail, "struct Person");
    let color = items
        .iter()
        .find(|item| item.label == "Color")
        .expect("color");
    assert_eq!(color.detail, "enum Color");
}

#[test]
fn dep_completion_suggests_struct_with_origin() {
    let b = "struct Person {\nuint age;\n}\ncontract B {\n}";
    let a = "import \"./B.sol\";\ncontract A {\nfunction f() public {\nPer\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    let found = complete_multi(&workspace, "/A.sol", a, "Per");
    let hit = found
        .iter()
        .find(|(label, _)| label == "Person")
        .expect("dep struct hit");
    assert!(hit.1.contains("struct Person"), "detail: {}", hit.1);
    assert!(hit.1.contains("from B.sol"), "detail: {}", hit.1);
}

#[test]
fn hover_shows_struct_detail() {
    let text = "contract C {\nstruct Person {\nuint age;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.find("Person").expect("name") as u32;
    let hover = hover_at(&file, text, at).expect("hover");
    assert_eq!(hover.contents, "struct Person");
}

#[test]
fn goto_jumps_to_struct_declaration() {
    let text = "contract C {\nstruct Person {\nuint age;\n}\nfunction f() public {\nPerson;\n}\n}";
    let at_use = text.rfind("Person").expect("use") as u32;
    let at_decl = text.find("Person").expect("decl") as u32;
    let target = goto_target(text, at_use);
    assert!(target.start <= at_decl && at_decl < target.end);
}

#[test]
fn struct_field_completion_suggests_fields() {
    let text = "struct Person {\nuint age;\nstring name;\n}\ncontract C {\nfunction f() public {\nPerson p;\np.\n}\n}";
    let at = text.rfind("p.").expect("dot") as u32 + 2;
    let labels = complete_labels(text, at);
    assert!(labels.contains(&"age".to_string()), "labels: {labels:?}");
    assert!(labels.contains(&"name".to_string()), "labels: {labels:?}");
    assert!(!labels.contains(&"p".to_string()), "labels: {labels:?}");
}

#[test]
fn struct_field_completion_filters_by_prefix() {
    let text = "struct Person {\nuint age;\nstring name;\n}\ncontract C {\nfunction f() public {\nPerson p;\np.a\n}\n}";
    let at = text.rfind("p.a").expect("dot") as u32 + 3;
    let labels = complete_labels(text, at);
    assert!(labels.contains(&"age".to_string()), "labels: {labels:?}");
    assert!(!labels.contains(&"name".to_string()), "labels: {labels:?}");
}

#[test]
fn contract_member_completion_via_this() {
    let text =
        "contract C {\nuint total;\nfunction foo() public {}\nfunction f() public {\nthis.\n}\n}";
    let at = text.rfind("this.").expect("dot") as u32 + 5;
    let labels = complete_labels(text, at);
    assert!(labels.contains(&"total".to_string()), "labels: {labels:?}");
    assert!(labels.contains(&"foo".to_string()), "labels: {labels:?}");
}

#[test]
fn member_hover_shows_field_detail() {
    let text = "struct Person {\nuint age;\n}\ncontract C {\nfunction f() public {\nPerson p;\np.age;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("p.age").expect("use") as u32 + 2;
    let hover = hover_at(&file, text, at).expect("hover");
    assert_eq!(hover.contents, "uint age");
}

#[test]
fn member_goto_jumps_to_field() {
    let text = "struct Person {\nuint age;\n}\ncontract C {\nfunction f() public {\nPerson p;\np.age;\n}\n}";
    let at_use = text.rfind("p.age").expect("use") as u32 + 2;
    let at_decl = text.find("age").expect("decl") as u32;
    let target = goto_target(text, at_use);
    assert!(target.start <= at_decl && at_decl < target.end);
}

#[test]
fn member_hover_shows_contract_state() {
    let text = "contract C {\nuint total;\nfunction f() public {\nthis.total;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("this.total").expect("use") as u32 + 5;
    let hover = hover_at(&file, text, at).expect("hover");
    assert_eq!(hover.contents, "uint total");
}

#[test]
fn member_goto_jumps_to_contract_state() {
    let text = "contract C {\nuint total;\nfunction f() public {\nthis.total;\n}\n}";
    let at_use = text.rfind("this.total").expect("use") as u32 + 5;
    let at_decl = text.find("total").expect("decl") as u32;
    let target = goto_target(text, at_use);
    assert!(target.start <= at_decl && at_decl < target.end);
}

#[test]
fn unknown_receiver_member_has_no_items() {
    let text = "contract C {\nfunction f() public {\nunknown.\n}\n}";
    let at = text.rfind("unknown.").expect("dot") as u32 + 8;
    let labels = complete_labels(text, at);
    assert!(labels.is_empty(), "labels: {labels:?}");
}

#[test]
fn chained_member_returns_scope_fallback() {
    let text = "struct Person {\nuint age;\n}\ncontract C {\nfunction f() public {\nPerson p;\na.b.c\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("a.b.c").expect("chain") as u32 + 4;
    assert!(hover_at(&file, text, at).is_none());
}

#[test]
fn chained_struct_field_resolves_final_only() {
    let text = "struct Inner {\nuint x;\n}\nstruct Outer {\nInner inner;\n}\ncontract C {\nfunction f() public {\nOuter o;\no.inner.x\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("o.inner.x").expect("chain") as u32 + 8;
    let found = hover_at(&file, text, at).expect("hover");
    assert!(
        found.contents.contains("uint"),
        "contents: {}",
        found.contents
    );
    assert!(found.contents.contains("x"), "contents: {}", found.contents);
}

#[test]
fn chained_builtin_parent_resolves_final() {
    let text = "contract C {\nfunction f() public {\naddress owner;\nowner.balance\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("owner.balance").expect("chain") as u32 + 8;
    let found = hover_at(&file, text, at).expect("hover");
    assert!(
        found.contents.contains("balance"),
        "contents: {}",
        found.contents
    );
}

#[test]
fn chained_unknown_intermediate_stays_silent() {
    let text = "struct Inner {\nuint x;\n}\ncontract C {\nfunction f() public {\nInner o;\no.missing.x\n}\n}";
    let found = diagnostics_of(text);
    assert!(found.is_empty(), "diags: {found:?}");
}

#[test]
fn try_returns_param_resolves_in_body() {
    let text = "contract C {\nfunction f() public {\ntry this.greet() returns (uint amount) {\namount;\n}\ncatch {\n}\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("amount;").expect("use") as u32;
    let found = hover_at(&file, text, at).expect("hover");
    assert!(
        found.contents.contains("uint"),
        "contents: {}",
        found.contents
    );
    assert!(
        found.contents.contains("amount"),
        "contents: {}",
        found.contents
    );
}

#[test]
fn try_catch_param_resolves_in_body() {
    let text = "contract C {\nfunction f() public {\ntry this.greet() {\n} catch (bytes memory e) {\ne;\n}\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("\ne;\n").expect("use") as u32 + 1;
    let found = hover_at(&file, text, at).expect("hover");
    assert!(
        found.contents.contains("bytes"),
        "contents: {}",
        found.contents
    );
    assert!(found.contents.contains("e"), "contents: {}", found.contents);
}

#[test]
fn try_new_reuses_deploy_arity() {
    let text = "contract Vault {\nconstructor(uint a, uint b) { a; b; }\n}\ncontract C {\nfunction f() public {\ntry new Vault(1) {\n} catch {\n}\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this constructor for 'Vault' takes 2 arguments but 1 supplied"
    );
}

#[test]
fn inherited_plain_completion_shows_base_function() {
    let text = "contract B {\nfunction greet(uint a) public { a; }\n}\ncontract D is B {\nfunction f() public {\ngre\n}\n}";
    let at = text.rfind("gre").expect("prefix") as u32 + 3;
    let labels = complete_labels(text, at);
    assert!(labels.contains(&"greet".to_string()), "labels: {labels:?}");
    let tokens = SolidityLexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    let items = complete_at(&file, text, at);
    let hit = items
        .iter()
        .find(|item| item.label == "greet")
        .expect("hit");
    assert!(hit.detail.contains("from B"), "detail: {}", hit.detail);
}

#[test]
fn inherited_plain_hover_shows_origin() {
    let text = "contract B {\nfunction greet(uint a) public { a; }\n}\ncontract D is B {\nfunction f() public {\ngreet;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("greet;").expect("use") as u32;
    let found = hover_at(&file, text, at).expect("hover");
    assert!(
        found.contents.contains("function greet"),
        "contents: {}",
        found.contents
    );
    assert!(
        found.contents.contains("from B"),
        "contents: {}",
        found.contents
    );
}

#[test]
fn inherited_plain_goto_jumps_to_base() {
    let text = "contract B {\nfunction greet(uint a) public { a; }\n}\ncontract D is B {\nfunction f() public {\ngreet;\n}\n}";
    let at_use = text.rfind("greet;").expect("use") as u32;
    let at_decl = text.find("greet").expect("decl") as u32;
    let target = goto_target(text, at_use);
    assert!(target.start <= at_decl && at_decl < target.end);
}

#[test]
fn inherited_this_completion_shows_base_members() {
    let text = "contract B {\nuint total;\nfunction foo() public {}\n}\ncontract D is B {\nfunction f() public {\nthis.\n}\n}";
    let at = text.rfind("this.").expect("dot") as u32 + 5;
    let labels = complete_labels(text, at);
    assert!(labels.contains(&"total".to_string()), "labels: {labels:?}");
    assert!(labels.contains(&"foo".to_string()), "labels: {labels:?}");
    let tokens = SolidityLexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    let items = complete_at(&file, text, at);
    let hit = items.iter().find(|item| item.label == "foo").expect("hit");
    assert!(hit.detail.contains("from B"), "detail: {}", hit.detail);
}

#[test]
fn inherited_diamond_keeps_first_origin_once() {
    let text = "contract A {\nfunction settle() public {}\n}\ncontract B is A {\n}\ncontract C is A {\n}\ncontract D is B, C {\nfunction f() public {\nthis.\n}\n}";
    let at = text.rfind("this.").expect("dot") as u32 + 5;
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let items = complete_at(&file, text, at);
    let hits: Vec<&CompletionItem> = items.iter().filter(|item| item.label == "settle").collect();
    assert_eq!(hits.len(), 1, "hits: {}", hits.len());
    assert!(
        hits[0].detail.contains("from A"),
        "detail: {}",
        hits[0].detail
    );
}

#[test]
fn inherited_c3_direct_before_transitive() {
    use super::inheritance;
    let text = "contract A {}\ncontract B is A {}\ncontract C is B {}\n";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let chain = inheritance::linearize(&file, "C");
    let names: Vec<String> = chain.into_iter().filter_map(|def| def.name()).collect();
    assert_eq!(names, vec!["B", "A"]);
}

#[test]
fn inherited_c3_diamond_orders_directs_first() {
    use super::inheritance;
    let text = "contract A {}\ncontract B is A {}\ncontract C is A {}\ncontract D is B, C {}\n";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let chain = inheritance::linearize(&file, "D");
    let names: Vec<String> = chain.into_iter().filter_map(|def| def.name()).collect();
    assert_eq!(names, vec!["B", "C", "A"]);
}

#[test]
fn inherited_inconsistent_falls_back_with_diagnostic() {
    let text = "contract A {}\ncontract B is A {}\ncontract C is B {}\ncontract D is A, C {}\n";
    let found = diagnostics_of(text);
    assert!(
        found
            .iter()
            .any(|diag| diag.message.contains("inconsistent linearization")),
        "diags: {found:?}"
    );
}

#[test]
fn inherited_shadow_derived_wins_without_suffix() {
    let text = "contract B {\nfunction greet(uint a) public { a; }\n}\ncontract D is B {\nfunction greet() public {}\nfunction f() public {\ngreet;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("greet;").expect("use") as u32;
    let found = hover_at(&file, text, at).expect("hover");
    assert_eq!(found.contents, "function greet()");
    let target = goto_target(text, at);
    let own_decl = text.rfind("function greet()").expect("own") as u32 + 9;
    assert!(target.start <= own_decl && own_decl < target.end);
    let at_complete = text.rfind("greet;").expect("prefix") as u32 + 3;
    let items = complete_at(&file, text, at_complete);
    let hits: Vec<&CompletionItem> = items.iter().filter(|item| item.label == "greet").collect();
    assert_eq!(hits.len(), 1);
    assert!(
        !hits[0].detail.contains("from"),
        "detail: {}",
        hits[0].detail
    );
}

#[test]
fn inherited_unknown_base_is_ignored() {
    let text = "contract D is Missing {\nfunction foo() public {}\nfunction f() public {\nfoo;\nthis.foo;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    let at = text.rfind("foo;").expect("use") as u32;
    assert!(hover_at(&file, text, at).is_some());
    let at_member = text.rfind("this.foo").expect("member") as u32 + 5;
    assert!(hover_at(&file, text, at_member).is_some());
}

#[test]
fn inherited_cycle_terminates() {
    let text = "contract A is B {\nfunction fa() public {}\n}\ncontract B is A {\nfunction fb() public {}\nfunction f() public {\nthis.fa;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    let at = text.rfind("this.fa").expect("use") as u32 + 5;
    let found = hover_at(&file, text, at).expect("hover");
    assert!(
        found.contents.contains("fa"),
        "contents: {}",
        found.contents
    );
    assert!(
        found.contents.contains("from A"),
        "contents: {}",
        found.contents
    );
}

#[test]
fn inherited_arity_uses_base_signature() {
    let text = "contract B {\nfunction greet(uint a, uint b) public { a; b; }\n}\ncontract D is B {\nfunction f() public {\ngreet(1);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this function 'greet' takes 2 arguments but 1 supplied"
    );
}

#[test]
fn inherited_arity_shadow_uses_derived_signature() {
    let text = "contract B {\nfunction greet(uint a, uint b) public { a; b; }\n}\ncontract D is B {\nfunction greet(uint a) public { a; }\nfunction f() public {\ngreet(1, 2);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this function 'greet' takes 1 arguments but 2 supplied"
    );
}

fn member_complete_multi(
    workspace: &Workspace,
    path: &str,
    text: &str,
    needle: &str,
) -> Vec<(String, String)> {
    let owned = text.to_string();
    let buf = PathBuf::from(path);
    let at = owned.rfind(needle).expect("needle") as u32 + needle.len() as u32;
    let ctx = ProjectContext::new(buf.as_path(), &owned, at, workspace);
    SolidityAnalyzer
        .complete(&ctx)
        .into_iter()
        .map(|item| (item.label, item.detail))
        .collect()
}

#[test]
fn cross_file_member_completion_hello_world() {
    let dep = "contract Hello {\nfunction greet(string memory name) public {}\nuint total;\n}";
    let cur =
        "import \"./a.sol\";\ncontract HelloWorld is Hello {\nfunction f() public {\nthis.\n}\n}";
    let mut workspace = open_workspace(&[("/a.sol", dep), ("/hello.sol", cur)]);
    workspace.set_file_deps(PathBuf::from("/hello.sol"), vec![PathBuf::from("/a.sol")]);
    let found = member_complete_multi(&workspace, "/hello.sol", cur, "this.");
    let greet = found
        .iter()
        .find(|(label, _)| label == "greet")
        .expect("greet hit");
    assert!(
        greet.1.contains("from Hello in a.sol"),
        "detail: {}",
        greet.1
    );
    let total = found
        .iter()
        .find(|(label, _)| label == "total")
        .expect("total hit");
    assert!(
        total.1.contains("from Hello in a.sol"),
        "detail: {}",
        total.1
    );
}

#[test]
fn cross_file_member_hover_hello_world() {
    let dep = "contract Hello {\nfunction greet(string memory name) public {}\n}";
    let cur = "import \"./a.sol\";\ncontract HelloWorld is Hello {\nfunction f() public {\nthis.greet;\n}\n}";
    let mut workspace = open_workspace(&[("/a.sol", dep), ("/hello.sol", cur)]);
    workspace.set_file_deps(PathBuf::from("/hello.sol"), vec![PathBuf::from("/a.sol")]);
    let hit = hover_multi(&workspace, "/hello.sol", cur, "greet").expect("hover");
    assert!(
        hit.contents.contains("function greet"),
        "contents: {}",
        hit.contents
    );
    assert!(
        hit.contents.contains("from Hello in a.sol"),
        "contents: {}",
        hit.contents
    );
}

#[test]
fn cross_file_member_goto_hello_world() {
    let dep = "contract Hello {\nfunction greet(string memory name) public {}\n}";
    let cur = "import \"./a.sol\";\ncontract HelloWorld is Hello {\nfunction f() public {\nthis.greet;\n}\n}";
    let mut workspace = open_workspace(&[("/a.sol", dep), ("/hello.sol", cur)]);
    workspace.set_file_deps(PathBuf::from("/hello.sol"), vec![PathBuf::from("/a.sol")]);
    let hit = goto_multi(&workspace, "/hello.sol", cur, "greet").expect("goto");
    assert_eq!(hit.path, PathBuf::from("/a.sol"));
    let want = dep.find("greet").expect("decl") as u32;
    assert!(hit.span.contains(want));
}

#[test]
fn cross_file_member_receiver_type_in_dep() {
    let dep = "contract Hello {\nfunction greet() public {}\n}";
    let cur =
        "import \"./a.sol\";\ncontract HelloWorld {\nfunction f() public {\nHello h;\nh.\n}\n}";
    let mut workspace = open_workspace(&[("/a.sol", dep), ("/hello.sol", cur)]);
    workspace.set_file_deps(PathBuf::from("/hello.sol"), vec![PathBuf::from("/a.sol")]);
    let found = member_complete_multi(&workspace, "/hello.sol", cur, "h.");
    let hit = found
        .iter()
        .find(|(label, _)| label == "greet")
        .expect("greet hit");
    assert!(hit.1.contains("from Hello in a.sol"), "detail: {}", hit.1);
}

#[test]
fn cross_file_member_shadow_local_wins() {
    let dep = "contract Hello {\nfunction greet(uint a) public { a; }\n}";
    let cur = "import \"./a.sol\";\ncontract HelloWorld is Hello {\nfunction greet() public {}\nfunction f() public {\nthis.greet;\nthis.\n}\n}";
    let mut workspace = open_workspace(&[("/a.sol", dep), ("/hello.sol", cur)]);
    workspace.set_file_deps(PathBuf::from("/hello.sol"), vec![PathBuf::from("/a.sol")]);
    let found = member_complete_multi(&workspace, "/hello.sol", cur, "this.");
    let hits: Vec<&(String, String)> = found.iter().filter(|(label, _)| label == "greet").collect();
    assert_eq!(hits.len(), 1);
    assert!(!hits[0].1.contains("from"), "local wins: {}", hits[0].1);
    let goto_hit = goto_multi(&workspace, "/hello.sol", cur, "greet");
    if let Some(target) = goto_hit {
        assert_eq!(target.path, PathBuf::from("/hello.sol"));
    }
}

#[test]
fn cross_file_member_transitive_chain() {
    let c = "contract Hello {\nfunction greet() public {}\n}";
    let b = "import \"./C.sol\";\ncontract Mid is Hello {\n}";
    let a = "import \"./B.sol\";\ncontract HelloWorld is Mid {\nfunction f() public {\nthis.greet;\nthis.\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b), ("/C.sol", c)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    workspace.set_file_deps(PathBuf::from("/B.sol"), vec![PathBuf::from("/C.sol")]);
    let found = member_complete_multi(&workspace, "/A.sol", a, "this.");
    let hit = found
        .iter()
        .find(|(label, _)| label == "greet")
        .expect("transitive hit");
    assert!(hit.1.contains("from Hello in C.sol"), "detail: {}", hit.1);
    let goto_hit = goto_multi(&workspace, "/A.sol", a, "greet").expect("goto");
    assert_eq!(goto_hit.path, PathBuf::from("/C.sol"));
}

#[test]
fn cross_file_member_untracked_is_skipped() {
    let cur = "import \"./Missing.sol\";\ncontract HelloWorld is Hello {\nfunction f() public {\nthis.greet;\nthis.\n}\n}";
    let mut workspace = open_workspace(&[("/hello.sol", cur)]);
    workspace.set_file_deps(
        PathBuf::from("/hello.sol"),
        vec![PathBuf::from("/Missing.sol")],
    );
    let found = member_complete_multi(&workspace, "/hello.sol", cur, "this.");
    assert!(!found.iter().any(|(label, _)| label == "greet"));
    assert!(hover_multi(&workspace, "/hello.sol", cur, "greet").is_none());
    assert!(goto_multi(&workspace, "/hello.sol", cur, "greet").is_none());
}

#[test]
fn cross_file_member_cycle_terminates() {
    let b = "import \"./A.sol\";\ncontract Hello {\nfunction greet() public {}\n}";
    let a =
        "import \"./B.sol\";\ncontract HelloWorld is Hello {\nfunction f() public {\nthis.\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    workspace.set_file_deps(PathBuf::from("/B.sol"), vec![PathBuf::from("/A.sol")]);
    let found = member_complete_multi(&workspace, "/A.sol", a, "this.");
    assert!(found.iter().any(|(label, _)| label == "greet"));
}

#[test]
fn cross_file_member_broken_dep_still_works() {
    let b = "contract Hello {\nfunction greet() public {}\n}\n@@@ broken";
    let a = "import \"./B.sol\";\ncontract HelloWorld is Hello {\nfunction f() public {\nthis.greet;\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    let found = member_complete_multi(&workspace, "/A.sol", a, "this.");
    let hit = found
        .iter()
        .find(|(label, _)| label == "greet")
        .expect("broken dep hit");
    assert!(hit.1.contains("from Hello in B.sol"), "detail: {}", hit.1);
    let goto_hit = goto_multi(&workspace, "/A.sol", a, "greet").expect("goto");
    assert_eq!(goto_hit.path, PathBuf::from("/B.sol"));
}

fn fresh_node_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(name);
    let _ = std::fs::remove_dir_all(&root);
    for rel in [
        "contracts/A.sol",
        "node_modules/beta/Token.sol",
        "node_modules/beta/sub/Deep.sol",
        "node_modules/@scope/pkg/contracts/T.sol",
        "node_modules/@scope/pkg/nested/Deep.sol",
        "node_modules/@scope/other/Token.sol",
        "node_modules/empty-pkg/README.txt",
        "node_modules/@scope/empty-pkg/README.txt",
    ] {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("parent");
        }
        std::fs::write(&path, "contract X {}").expect("write");
    }
    root
}

#[test]
fn bare_scaffolds_top_level_packages() {
    let root = fresh_node_root("forgels-sol-bare-scaffold");
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        root.join("node_modules/beta"),
        root.join("node_modules/link-pkg"),
    )
    .expect("symlink");
    let importer = root.join("contracts/A.sol");
    let found = complete_bare_imports(&importer, std::slice::from_ref(&root), "");
    let labels: Vec<&str> = found.iter().map(|item| item.label.as_str()).collect();
    for want in [
        "beta/",
        "empty-pkg/",
        "@scope/pkg/",
        "@scope/other/",
        "@scope/empty-pkg/",
    ] {
        assert!(labels.contains(&want), "labels: {labels:?}");
    }
    assert!(
        !labels.iter().any(|label| label.starts_with("link-pkg")),
        "labels: {labels:?}"
    );
    assert!(found.iter().all(|item| item.detail == "package"));
    assert!(
        found
            .iter()
            .all(|item| completion_item_kind(item.kind).value() == 19)
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn bare_scaffold_kind_is_folder_file_kind_is_file() {
    let root = fresh_node_root("forgels-sol-bare-kinds");
    let importer = root.join("contracts/A.sol");
    let top = complete_bare_imports(&importer, std::slice::from_ref(&root), "");
    assert!(!top.is_empty());
    assert!(
        top.iter()
            .all(|item| completion_item_kind(item.kind).value() == 19)
    );
    let files = complete_bare_imports(&importer, std::slice::from_ref(&root), "beta/");
    assert!(!files.is_empty());
    assert!(files.iter().all(|item| item.detail == "file"));
    assert!(
        files
            .iter()
            .all(|item| completion_item_kind(item.kind).value() == 17)
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn bare_filters_by_typed_prefix() {
    let root = fresh_node_root("forgels-sol-bare-filter");
    let importer = root.join("contracts/A.sol");
    let scoped = complete_bare_imports(&importer, std::slice::from_ref(&root), "@scope/p");
    let labels: Vec<&str> = scoped.iter().map(|item| item.label.as_str()).collect();
    assert_eq!(labels, vec!["@scope/pkg/"]);
    let plain = complete_bare_imports(&importer, std::slice::from_ref(&root), "bet");
    let plain_labels: Vec<&str> = plain.iter().map(|item| item.label.as_str()).collect();
    assert_eq!(plain_labels, vec!["beta/"]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn bare_subpath_lists_package_files() {
    let root = fresh_node_root("forgels-sol-bare-files");
    let importer = root.join("contracts/A.sol");
    let scoped = complete_bare_imports(&importer, std::slice::from_ref(&root), "@scope/pkg/");
    let labels: Vec<&str> = scoped.iter().map(|item| item.label.as_str()).collect();
    assert!(
        labels.contains(&"@scope/pkg/contracts/T.sol"),
        "labels: {labels:?}"
    );
    assert!(
        labels.contains(&"@scope/pkg/nested/Deep.sol"),
        "labels: {labels:?}"
    );
    assert!(scoped.iter().all(|item| item.detail == "file"));
    let nested = complete_bare_imports(&importer, std::slice::from_ref(&root), "beta/sub/");
    let nested_labels: Vec<&str> = nested.iter().map(|item| item.label.as_str()).collect();
    assert_eq!(nested_labels, vec!["beta/sub/Deep.sol"]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn bare_file_labels_roundtrip_resolve() {
    let root = fresh_node_root("forgels-sol-bare-roundtrip");
    let importer = root.join("contracts/A.sol");
    let roots = vec![root.clone()];
    let mut files = 0;
    for prefix in [
        "",
        "bet",
        "beta/",
        "beta/sub/",
        "@scope/",
        "@scope/p",
        "@scope/pkg/",
        "@scope/pkg/nested/",
    ] {
        for item in complete_bare_imports(&importer, &roots, prefix) {
            if item.detail == "file" {
                assert_eq!(
                    completion_item_kind(item.kind).value(),
                    17,
                    "kind: {}",
                    item.label
                );
                files += 1;
                let hit = forgels_common::paths::resolve_import_full(
                    &importer,
                    &item.label,
                    &root,
                    &[],
                    forgels_common::fs::is_trackable_file,
                );
                assert!(hit.is_some(), "unresolvable: {}", item.label);
            } else {
                assert_eq!(item.detail, "package");
                assert_eq!(
                    completion_item_kind(item.kind).value(),
                    19,
                    "kind: {}",
                    item.label
                );
            }
        }
    }
    assert!(files > 0, "expected file labels");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn bare_dot_prefix_stays_empty() {
    let root = fresh_node_root("forgels-sol-bare-dot");
    let importer = root.join("contracts/A.sol");
    assert!(complete_bare_imports(&importer, std::slice::from_ref(&root), ".").is_empty());
    assert!(complete_bare_imports(&importer, std::slice::from_ref(&root), "./").is_empty());
    assert!(complete_bare_imports(&importer, std::slice::from_ref(&root), "../").is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn bare_package_without_sol_has_no_file_labels() {
    let root = fresh_node_root("forgels-sol-bare-empty");
    let importer = root.join("contracts/A.sol");
    assert!(complete_bare_imports(&importer, std::slice::from_ref(&root), "empty-pkg/").is_empty());
    assert!(
        complete_bare_imports(&importer, std::slice::from_ref(&root), "@scope/empty-pkg/")
            .is_empty()
    );
    let top = complete_bare_imports(&importer, std::slice::from_ref(&root), "");
    let labels: Vec<&str> = top.iter().map(|item| item.label.as_str()).collect();
    assert!(labels.contains(&"empty-pkg/"), "labels: {labels:?}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn bare_scaffolds_skip_result_cap() {
    let root = std::env::temp_dir().join("forgels-sol-bare-cap");
    let _ = std::fs::remove_dir_all(&root);
    for index in 0..40 {
        let dir = root.join(format!("node_modules/pkg{index:02}"));
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("T.sol"), "contract X {}").expect("write");
    }
    let contracts = root.join("contracts");
    std::fs::create_dir_all(&contracts).expect("dir");
    let importer = contracts.join("A.sol");
    let found = complete_bare_imports(&importer, std::slice::from_ref(&root), "");
    assert_eq!(found.len(), 40);
    assert!(found.iter().all(|item| item.detail == "package"));
    assert!(
        found
            .iter()
            .all(|item| completion_item_kind(item.kind).value() == 19)
    );
    let mut sorted = found
        .iter()
        .map(|item| item.label.clone())
        .collect::<Vec<_>>();
    sorted.sort();
    let labels: Vec<&str> = found.iter().map(|item| item.label.as_str()).collect();
    assert_eq!(labels, sorted);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn bare_scaffolds_return_all_200_packages() {
    let root = std::env::temp_dir().join("forgels-sol-bare-200");
    let _ = std::fs::remove_dir_all(&root);
    for index in 0..200 {
        let dir = root.join(format!("node_modules/pkg{index:03}"));
        std::fs::create_dir_all(&dir).expect("dir");
    }
    let target = root.join("node_modules/@openzeppelin/contracts");
    std::fs::create_dir_all(&target).expect("dir");
    let contracts = root.join("contracts");
    std::fs::create_dir_all(&contracts).expect("dir");
    let importer = contracts.join("A.sol");
    let found = complete_bare_imports(&importer, std::slice::from_ref(&root), "");
    assert_eq!(found.len(), 201);
    let labels: Vec<&str> = found.iter().map(|item| item.label.as_str()).collect();
    assert!(
        labels.contains(&"@openzeppelin/contracts/"),
        "labels: {labels:?}"
    );
    assert!(
        found
            .iter()
            .all(|item| completion_item_kind(item.kind).value() == 19)
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn bare_file_labels_cap_at_32() {
    let root = std::env::temp_dir().join("forgels-sol-bare-file-cap");
    let _ = std::fs::remove_dir_all(&root);
    let pkg = root.join("node_modules/bigpkg");
    std::fs::create_dir_all(&pkg).expect("dir");
    for index in 0..40 {
        std::fs::write(pkg.join(format!("File{index:02}.sol")), "contract X {}").expect("write");
    }
    let contracts = root.join("contracts");
    std::fs::create_dir_all(&contracts).expect("dir");
    let importer = contracts.join("A.sol");
    let found = complete_bare_imports(&importer, std::slice::from_ref(&root), "bigpkg/");
    assert_eq!(found.len(), 32);
    assert!(found.iter().all(|item| item.detail == "file"));
    assert!(
        found
            .iter()
            .all(|item| completion_item_kind(item.kind).value() == 17)
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn bare_finds_scoped_package_beyond_dir_cap() {
    let root = std::env::temp_dir().join("forgels-sol-bare-beyond-cap");
    let _ = std::fs::remove_dir_all(&root);
    for index in 0..200 {
        let dir = root.join(format!("node_modules/pkg{index:03}"));
        std::fs::create_dir_all(&dir).expect("dir");
    }
    let target = root.join("node_modules/@openzeppelin/contracts");
    std::fs::create_dir_all(&target).expect("dir");
    std::fs::write(target.join("Token.sol"), "contract X {}").expect("write");
    let contracts = root.join("contracts");
    std::fs::create_dir_all(&contracts).expect("dir");
    let importer = contracts.join("A.sol");
    let found = complete_bare_imports(&importer, std::slice::from_ref(&root), "@open");
    let labels: Vec<&str> = found.iter().map(|item| item.label.as_str()).collect();
    assert!(
        labels.contains(&"@openzeppelin/contracts/"),
        "labels: {labels:?}"
    );
    let hit = found
        .iter()
        .find(|item| item.label == "@openzeppelin/contracts/")
        .expect("oz hit");
    assert_eq!(hit.detail, "package");
    assert_eq!(completion_item_kind(hit.kind).value(), 19);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn bare_finds_scope_subprefix_beyond_scope_cap() {
    let root = std::env::temp_dir().join("forgels-sol-bare-scope-cap");
    let _ = std::fs::remove_dir_all(&root);
    for index in 0..150 {
        let dir = root.join(format!("node_modules/@types/lib{index:03}"));
        std::fs::create_dir_all(&dir).expect("dir");
    }
    let target = root.join("node_modules/@types/wanted-pkg");
    std::fs::create_dir_all(&target).expect("dir");
    let contracts = root.join("contracts");
    std::fs::create_dir_all(&contracts).expect("dir");
    let importer = contracts.join("A.sol");
    let found = complete_bare_imports(&importer, std::slice::from_ref(&root), "@types/want");
    let labels: Vec<&str> = found.iter().map(|item| item.label.as_str()).collect();
    assert_eq!(labels, vec!["@types/wanted-pkg/"]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn bare_finds_subpath_beyond_walk_cap() {
    let root = std::env::temp_dir().join("forgels-sol-bare-walk-cap");
    let _ = std::fs::remove_dir_all(&root);
    let pkg = root.join("node_modules/bigpkg");
    std::fs::create_dir_all(&pkg).expect("dir");
    for index in 0..250 {
        std::fs::write(pkg.join(format!("Filler{index:03}.sol")), "contract X {}").expect("write");
    }
    std::fs::write(pkg.join("Wanted.sol"), "contract X {}").expect("write");
    let contracts = root.join("contracts");
    std::fs::create_dir_all(&contracts).expect("dir");
    let importer = contracts.join("A.sol");
    let found = complete_bare_imports(&importer, std::slice::from_ref(&root), "bigpkg/Want");
    let labels: Vec<&str> = found.iter().map(|item| item.label.as_str()).collect();
    assert_eq!(labels, vec!["bigpkg/Wanted.sol"]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn path_prefix_keeps_scoped_bare_prefix() {
    let text = "import \"@scope/p";
    let prefix = super::completion::path_prefix_at(text, text.len() as u32);
    assert_eq!(prefix, "@scope/p");
    let relative = "import \"./B";
    let rel_prefix = super::completion::path_prefix_at(relative, relative.len() as u32);
    assert_eq!(rel_prefix, "./B");
}

#[test]
fn analyzer_complete_merges_bare_node_imports() {
    let root = fresh_node_root("forgels-sol-bare-branch");
    let current = root.join("contracts/A.sol");
    let text = "import \"beta/";
    let mut workspace = Workspace::new();
    workspace.set_roots(vec![root.clone()]);
    workspace
        .open(current.clone(), Language::Solidity, text.to_string(), 1)
        .expect("open works");
    let owned = text.to_string();
    let ctx = ProjectContext::new(&current, &owned, owned.len() as u32, &workspace);
    let found = SolidityAnalyzer.complete(&ctx);
    let hit = found
        .iter()
        .find(|item| item.label == "beta/Token.sol")
        .expect("bare hit");
    assert_eq!(hit.detail, "file");
    assert_eq!(hit.kind.value(), 17);
    let _ = std::fs::remove_dir_all(&root);
}

fn apply_edit(text: &str, span: Span, replacement: &str) -> String {
    let mut out = text.to_string();
    out.replace_range(span.start as usize..span.end as usize, replacement);
    out
}

#[test]
fn path_completion_replaces_full_inner() {
    let root = fresh_sol_root("forgels-sol-import-edit");
    let current = root.join("A.sol");
    let text = "import \"./B\";";
    let mut workspace = Workspace::new();
    workspace.set_roots(vec![root.clone()]);
    workspace
        .open(current.clone(), Language::Solidity, text.to_string(), 1)
        .expect("open works");
    let owned = text.to_string();
    let at = owned.find("./B").expect("prefix") as u32 + 3;
    let ctx = ProjectContext::new(&current, &owned, at, &workspace);
    let hit = SolidityAnalyzer
        .complete(&ctx)
        .into_iter()
        .find(|item| item.label == "./B.sol")
        .expect("path hit");
    let edit = hit.text_edit.expect("path carries edit");
    assert_eq!(edit.new_text, "./B.sol");
    let applied = apply_edit(&owned, edit.span, &edit.new_text);
    assert_eq!(applied, "import \"./B.sol\";");
    let found = imports_in(&applied);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].raw, "./B.sol");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn symbol_and_keyword_have_no_edit() {
    let text = "contract C {\nuint256 total;\ntot";
    let workspace = open_workspace(&[("/repo/C.sol", text)]);
    let owned = text.to_string();
    let buf = PathBuf::from("/repo/C.sol");
    let ctx = ProjectContext::new(&buf, &owned, owned.len() as u32, &workspace);
    let symbol = SolidityAnalyzer
        .complete(&ctx)
        .into_iter()
        .find(|item| item.label == "total")
        .expect("symbol hit");
    assert!(symbol.text_edit.is_none());
    let keyword_text = "contract C {\nfun";
    let keyword_workspace = open_workspace(&[("/repo/C.sol", keyword_text)]);
    let keyword_owned = keyword_text.to_string();
    let keyword_ctx = ProjectContext::new(
        &buf,
        &keyword_owned,
        keyword_owned.len() as u32,
        &keyword_workspace,
    );
    let keyword = SolidityAnalyzer
        .complete(&keyword_ctx)
        .into_iter()
        .find(|item| item.label == "function")
        .expect("keyword hit");
    assert!(keyword.text_edit.is_none());
}

#[test]
fn mid_string_cursor_replaces_suffix() {
    let root = fresh_sol_root("forgels-sol-import-mid");
    let current = root.join("A.sol");
    let text = "import \"./B.sol\";";
    let mut workspace = Workspace::new();
    workspace.set_roots(vec![root.clone()]);
    workspace
        .open(current.clone(), Language::Solidity, text.to_string(), 1)
        .expect("open works");
    let owned = text.to_string();
    let at = owned.find("./B").expect("prefix") as u32 + 3;
    let ctx = ProjectContext::new(&current, &owned, at, &workspace);
    let hit = SolidityAnalyzer
        .complete(&ctx)
        .into_iter()
        .find(|item| item.label == "./B.sol")
        .expect("path hit");
    let edit = hit.text_edit.expect("path carries edit");
    assert!(edit.span.end > at, "edit covers suffix");
    let applied = apply_edit(&owned, edit.span, &edit.new_text);
    assert_eq!(applied, "import \"./B.sol\";");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn bare_completion_carries_edit() {
    let root = fresh_node_root("forgels-sol-bare-edit");
    let current = root.join("contracts/A.sol");
    let text = "import \"beta/\";";
    let mut workspace = Workspace::new();
    workspace.set_roots(vec![root.clone()]);
    workspace
        .open(current.clone(), Language::Solidity, text.to_string(), 1)
        .expect("open works");
    let owned = text.to_string();
    let at = owned.find("beta/").expect("prefix") as u32 + 5;
    let ctx = ProjectContext::new(&current, &owned, at, &workspace);
    let hit = SolidityAnalyzer
        .complete(&ctx)
        .into_iter()
        .find(|item| item.label == "beta/Token.sol")
        .expect("bare hit");
    let edit = hit.text_edit.expect("bare carries edit");
    assert_eq!(edit.new_text, "beta/Token.sol");
    let applied = apply_edit(&owned, edit.span, &edit.new_text);
    assert!(applied.contains("\"beta/Token.sol\""), "applied: {applied}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn inner_span_rejects_out_of_bounds() {
    assert!(import_inner_span("short", Span::new(100, 110)).is_none());
    let text = "import \"./B.sol\";";
    assert!(import_inner_span(text, Span::new(7, 100)).is_none());
    assert!(import_inner_span(text, Span::new(0, 6)).is_none());
}

#[test]
fn emit_completion_lists_only_events() {
    let text = "contract C {\nevent Transfer(address a);\nfunction greet() public {}\nfunction f() public {\nemit \n}\n}";
    let at = text.rfind("emit ").expect("emit") as u32 + 5;
    let labels = complete_labels(text, at);
    assert!(
        labels.contains(&"Transfer".to_string()),
        "labels: {labels:?}"
    );
    assert!(!labels.contains(&"greet".to_string()), "labels: {labels:?}");
    assert!(
        !labels.contains(&"function".to_string()),
        "labels: {labels:?}"
    );
}

#[test]
fn emit_completion_filters_by_prefix() {
    let text = "contract C {\nevent Transfer(address a);\nevent Approval(address b);\nfunction f() public {\nemit Tra\n}\n}";
    let at = text.rfind("Tra").expect("prefix") as u32 + 3;
    let labels = complete_labels(text, at);
    assert!(
        labels.contains(&"Transfer".to_string()),
        "labels: {labels:?}"
    );
    assert!(
        !labels.contains(&"Approval".to_string()),
        "labels: {labels:?}"
    );
}

#[test]
fn emit_hover_resolves_to_event() {
    let text =
        "contract C {\nevent Transfer(address a);\nfunction f() public {\nemit Transfer(a);\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("Transfer(a)").expect("use") as u32;
    let found = hover_at(&file, text, at).expect("hover");
    assert_eq!(found.contents, "event Transfer");
}

#[test]
fn emit_goto_jumps_to_event() {
    let text =
        "contract C {\nevent Transfer(address a);\nfunction f() public {\nemit Transfer(a);\n}\n}";
    let at_use = text.rfind("Transfer(a)").expect("use") as u32;
    let at_decl = text.find("Transfer").expect("decl") as u32;
    let target = goto_target(text, at_use);
    assert!(target.start <= at_decl && at_decl < target.end);
}

#[test]
fn emit_arity_reports_exact_message() {
    let text = "contract C {\nevent Transfer(address a, address b);\nfunction f() public {\nemit Transfer(a);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this event 'Transfer' takes 2 arguments but 1 supplied"
    );
}

#[test]
fn emit_correct_count_stays_silent() {
    let text =
        "contract C {\nevent Transfer(address a);\nfunction f() public {\nemit Transfer(a);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

fn diagnostics_multi(
    workspace: &Workspace,
    path: &str,
    text: &str,
) -> Vec<forgels_core::DiagnosticData> {
    let owned = text.to_string();
    let buf = PathBuf::from(path);
    let ctx = ProjectContext::new(buf.as_path(), &owned, 0, workspace);
    SolidityAnalyzer.diagnostics_ctx(&ctx)
}

#[test]
fn emit_unknown_reports_name_span() {
    let text = "contract C {\nevent Increment(uint a);\nevent Decrement(uint a);\nfunction f() public {\nemit Inc(a);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(found[0].message, "unresolved event 'Inc'");
    let at = text.find("Inc(a)").expect("use") as u32;
    assert_eq!(found[0].span.start, at);
    assert_eq!(found[0].span.end, at + 3);
    let covered = &text[found[0].span.start as usize..found[0].span.end as usize];
    assert_eq!(covered, "Inc");
}

#[test]
fn emit_typo_prefix_reports() {
    let text = "contract C {\nevent Increment(uint a);\nevent Decrement(uint a);\nfunction f() public {\nemit Incremente(a);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(found[0].message, "unresolved event 'Incremente'");
}

#[test]
fn emit_known_events_stay_silent() {
    let text = "contract C {\nevent Increment(uint a);\nevent Decrement(uint a);\nfunction f() public {\nemit Increment(a);\nemit Decrement(a);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn emit_bare_stays_silent() {
    let bare = "contract C {\nfunction f() public {\nemit\n}\n}";
    assert!(diagnostics_of(bare).is_empty());
    let open = "contract C {\nfunction f() public {\nemit (\n}\n}";
    let found: Vec<_> = diagnostics_of(open)
        .into_iter()
        .filter(|item| item.message.starts_with("unresolved event"))
        .collect();
    assert!(found.is_empty(), "diags: {found:?}");
}

#[test]
fn emit_partial_name_still_lints() {
    let text = "contract C {\nevent Increment(uint a);\nfunction f() public {\nemit Inc\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(found[0].message, "unresolved event 'Inc'");
}

#[test]
fn emit_dep_event_stays_silent() {
    let b = "contract B {\nevent Transfer(address a);\n}";
    let a = "import \"./B.sol\";\ncontract A {\nfunction f() public {\nemit Transfer(a);\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    assert!(diagnostics_multi(&workspace, "/A.sol", a).is_empty());
}

#[test]
fn emit_inherited_event_stays_silent() {
    let text = "contract B {\nevent Transfer(address a);\n}\ncontract D is B {\nfunction f() public {\nemit Transfer(a);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn emit_untracked_dep_still_errors() {
    let a =
        "import \"./Missing.sol\";\ncontract A {\nfunction f() public {\nemit Transfer(a);\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/Missing.sol")]);
    let found = diagnostics_multi(&workspace, "/A.sol", a);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(found[0].message, "unresolved event 'Transfer'");
}

#[test]
fn emit_inherited_arity_uses_base() {
    let text = "contract B {\nevent Transfer(address a, address b);\n}\ncontract D is B {\nfunction f() public {\nemit Transfer(a);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this event 'Transfer' takes 2 arguments but 1 supplied"
    );
}

#[test]
fn emit_dep_completion_includes_origin() {
    let b = "contract B {\nevent Transfer(address a);\n}";
    let a = "import \"./B.sol\";\ncontract A {\nfunction f() public {\nemit Tra\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    let found = complete_multi(&workspace, "/A.sol", a, "Tra");
    let hit = found
        .iter()
        .find(|(label, _)| label == "Transfer")
        .expect("dep emit hit");
    assert!(hit.1.contains("from B.sol"), "detail: {}", hit.1);
}

#[test]
fn emit_dep_hover_includes_origin() {
    let b = "contract B {\nevent Transfer(address a);\n}";
    let a = "import \"./B.sol\";\ncontract A {\nfunction f() public {\nemit Transfer(a);\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    let hit = hover_multi(&workspace, "/A.sol", a, "Transfer(a)").expect("dep emit hover");
    assert!(
        hit.contents.contains("event Transfer"),
        "contents: {}",
        hit.contents
    );
    assert!(
        hit.contents.contains("from B.sol"),
        "contents: {}",
        hit.contents
    );
}

#[test]
fn emit_dep_goto_jumps_to_other_file() {
    let b = "contract B {\nevent Transfer(address a);\n}";
    let a = "import \"./B.sol\";\ncontract A {\nfunction f() public {\nemit Transfer(a);\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    let hit = goto_multi(&workspace, "/A.sol", a, "Transfer(a)").expect("dep emit goto");
    assert_eq!(hit.path, PathBuf::from("/B.sol"));
    let want = b.find("Transfer").expect("decl") as u32;
    assert!(hit.span.contains(want));
}

fn forge_body(body: &str) -> String {
    format!("contract T is Test {{\nfunction f() public {{\n{body}\n}}\n}}")
}

fn forge_plain(body: &str) -> String {
    format!("contract C {{\nfunction f() public {{\n{body}\n}}\n}}")
}

fn builtin_hit(text: &str, offset: u32, label: &str) -> CompletionItem {
    let tokens = SolidityLexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    complete_at(&file, text, offset)
        .into_iter()
        .find(|item| item.label == label)
        .unwrap_or_else(|| panic!("missing builtin {label}"))
}

#[test]
fn vm_completion_covers_every_table_entry() {
    let text = forge_body("vm.");
    let at = text.rfind("vm.").expect("dot") as u32 + 3;
    let tokens = SolidityLexer.tokenize(&text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    let items = complete_at(&file, &text, at);
    for builtin in VM_TABLE {
        let hit = items
            .iter()
            .find(|item| item.label == builtin.name)
            .unwrap_or_else(|| panic!("missing {}", builtin.name));
        assert_eq!(completion_item_kind(hit.kind).value(), 3);
        assert!(hit.detail.contains("forge-std"), "detail: {}", hit.detail);
    }
}

#[test]
fn console_completion_covers_every_table_entry() {
    let text = forge_body("console.");
    let at = text.rfind("console.").expect("dot") as u32 + 8;
    let tokens = SolidityLexer.tokenize(&text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    let items = complete_at(&file, &text, at);
    for builtin in CONSOLE_TABLE {
        let hit = items
            .iter()
            .find(|item| item.label == builtin.name)
            .unwrap_or_else(|| panic!("missing {}", builtin.name));
        assert_eq!(completion_item_kind(hit.kind).value(), 3);
        assert!(hit.detail.contains("forge-std"), "detail: {}", hit.detail);
    }
}

#[test]
fn assert_completion_covers_every_table_entry() {
    for builtin in ASSERT_TABLE {
        let text = forge_body(builtin.name);
        let at = text.rfind(builtin.name).expect("prefix") as u32 + builtin.name.len() as u32;
        let hit = builtin_hit(&text, at, builtin.name);
        assert_eq!(completion_item_kind(hit.kind).value(), 3);
        assert!(hit.detail.contains("forge-std"), "detail: {}", hit.detail);
    }
}

#[test]
fn vm_hover_covers_every_table_entry() {
    for builtin in VM_TABLE {
        let text = forge_body(&format!("vm.{}(x);", builtin.name));
        let tokens = SolidityLexer.tokenize(&text);
        let (file, _) = SolidityParser.parse_with_errors(&tokens);
        let at = text.rfind(builtin.name).expect("member") as u32;
        let found = hover_at(&file, &text, at).expect("hover");
        assert!(
            found.contents.contains(builtin.detail),
            "contents: {}",
            found.contents
        );
        assert!(
            found.contents.contains("forge-std"),
            "contents: {}",
            found.contents
        );
    }
}

#[test]
fn assert_hover_covers_every_table_entry() {
    for builtin in ASSERT_TABLE {
        let text = forge_body(&format!("{}(x);", builtin.name));
        let tokens = SolidityLexer.tokenize(&text);
        let (file, _) = SolidityParser.parse_with_errors(&tokens);
        let at = text.rfind(builtin.name).expect("callee") as u32;
        let found = hover_at(&file, &text, at).expect("hover");
        assert!(
            found.contents.contains(builtin.detail),
            "contents: {}",
            found.contents
        );
        assert!(
            found.contents.contains("forge-std"),
            "contents: {}",
            found.contents
        );
    }
}

#[test]
fn builtin_goto_returns_none_by_design() {
    let member_text = forge_body("vm.prank(x);");
    let tokens = SolidityLexer.tokenize(&member_text);
    let (member_file, _) = SolidityParser.parse_with_errors(&tokens);
    let member_at = member_text.rfind("prank").expect("member") as u32;
    assert!(goto_definition(&member_file, &member_text, member_at).is_none());
    let plain_text = forge_body("assertEq(a, b);");
    let plain_tokens = SolidityLexer.tokenize(&plain_text);
    let (plain_file, _) = SolidityParser.parse_with_errors(&plain_tokens);
    let plain_at = plain_text.rfind("assertEq").expect("callee") as u32;
    assert!(goto_definition(&plain_file, &plain_text, plain_at).is_none());
    let workspace = open_workspace(&[("/repo/C.sol", plain_text.as_str())]);
    assert!(goto_multi(&workspace, "/repo/C.sol", &plain_text, "assertEq").is_none());
}

#[test]
fn goto_skips_coincidental_dep_match() {
    let dep =
        "contract B {\nfunction assertEq(uint a) public {}\nfunction prank(address x) public {}\n}";
    let cur = "import \"./B.sol\";\ncontract T is Test {\nfunction f() public {\nassertEq(a, b);\nvm.prank(x);\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", cur), ("/B.sol", dep)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    assert!(goto_multi(&workspace, "/A.sol", cur, "assertEq(a").is_none());
    assert!(goto_multi(&workspace, "/A.sol", cur, "prank(x)").is_none());
}

#[test]
fn user_receiver_shadows_vm_builtin() {
    let text = "contract T is Test {\nuint vm;\nfunction f() public {\nvm.\n}\n}";
    let at = text.rfind("vm.").expect("dot") as u32 + 3;
    let labels = complete_labels(text, at);
    assert!(!labels.contains(&"prank".to_string()), "labels: {labels:?}");
    let tokens = SolidityLexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    let member_at = at - 1;
    assert!(hover_at(&file, text, member_at).is_none());
}

#[test]
fn user_function_shadows_assert_builtin() {
    let text = "contract T is Test {\nfunction assertEq(uint a) public {}\nfunction f() public {\nassertEq;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("assertEq;").expect("use") as u32;
    let found = hover_at(&file, text, at).expect("hover");
    assert!(
        !found.contents.contains("forge-std"),
        "local wins: {}",
        found.contents
    );
    assert_eq!(found.contents, "function assertEq(uint a)");
    let complete_at_use = text.rfind("assertEq;").expect("prefix") as u32 + 6;
    let items = complete_at(&file, text, complete_at_use);
    let hits: Vec<&CompletionItem> = items
        .iter()
        .filter(|item| item.label == "assertEq")
        .collect();
    assert_eq!(hits.len(), 1);
    assert!(
        !hits[0].detail.contains("forge-std"),
        "detail: {}",
        hits[0].detail
    );
}

#[test]
fn asserts_stay_hidden_without_test_heritage() {
    let text = forge_plain("assertEq(a, b);");
    let at = text.rfind("assertEq").expect("callee") as u32;
    let tokens = SolidityLexer.tokenize(&text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert!(hover_at(&file, &text, at).is_none());
    let complete_at_end = at + 8;
    let labels = complete_labels(&text, complete_at_end);
    assert!(
        !labels.contains(&"assertEq".to_string()),
        "labels: {labels:?}"
    );
    let wrong = forge_plain("assertEq(1);");
    let found = diagnostics_of(&wrong);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(found[0].message, "unresolved identifier 'assertEq'");
}

#[test]
fn transitive_test_heritage_enables_asserts() {
    let text = "contract Base is Test {\n}\ncontract T is Base {\nfunction f() public {\nassertEq(a, b);\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("assertEq(a").expect("callee") as u32;
    let found = hover_at(&file, text, at).expect("hover");
    assert!(
        found.contents.contains("forge-std"),
        "contents: {}",
        found.contents
    );
}

#[test]
fn unknown_forge_names_stay_silent() {
    let member_text = forge_body("vm.bogus(1);");
    let tokens = SolidityLexer.tokenize(&member_text);
    let (member_file, _) = SolidityParser.parse_with_errors(&tokens);
    let member_at = member_text.rfind("bogus").expect("member") as u32;
    assert!(hover_at(&member_file, &member_text, member_at).is_none());
    let plain_text = forge_body("assertBogus(1);");
    let found = diagnostics_of(&plain_text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(found[0].message, "unresolved identifier 'assertBogus'");
    let hevm_text = forge_body("hevm.warp(1);");
    let hevm_tokens = SolidityLexer.tokenize(&hevm_text);
    let (hevm_file, _) = SolidityParser.parse_with_errors(&hevm_tokens);
    let hevm_at = hevm_text.rfind("warp").expect("member") as u32;
    assert!(hover_at(&hevm_file, &hevm_text, hevm_at).is_none());
    let hevm_dot = hevm_text.rfind("hevm.").expect("dot") as u32 + 5;
    let labels = complete_labels(&hevm_text, hevm_dot);
    assert!(labels.is_empty(), "labels: {labels:?}");
}

#[test]
fn assert_arity_reports_exact_message() {
    let short = forge_body("assertEq(a);");
    let found = diagnostics_of(&short);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this function 'assertEq' takes 2 arguments but 1 supplied"
    );
    let long = forge_body("assertTrue(a, b);");
    let too_many = diagnostics_of(&long);
    assert_eq!(too_many.len(), 1, "diags: {too_many:?}");
    assert_eq!(
        too_many[0].message,
        "this function 'assertTrue' takes 1 arguments but 2 supplied"
    );
    let ok = forge_body("assertEq(a, b);");
    assert!(diagnostics_of(&ok).is_empty());
}

#[test]
fn builtin_completion_beats_coincidental_dep() {
    let dep = "contract B {\nfunction assertEq(uint a) public {}\n}";
    let cur = "import \"./B.sol\";\ncontract T is Test {\nfunction f() public {\nassertEq\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", cur), ("/B.sol", dep)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    let found = complete_multi(&workspace, "/A.sol", cur, "assertEq");
    let hits: Vec<&(String, String)> = found
        .iter()
        .filter(|(label, _)| label == "assertEq")
        .collect();
    assert_eq!(hits.len(), 1);
    assert!(
        hits[0].1.contains("forge-std"),
        "builtin wins: {}",
        hits[0].1
    );
}

fn signature_multi(
    workspace: &Workspace,
    path: &str,
    text: &str,
    needle: &str,
) -> Option<Vec<forgels_core::SignatureData>> {
    let owned = text.to_string();
    let buf = PathBuf::from(path);
    let at = owned.rfind(needle).expect("needle") as u32 + needle.len() as u32;
    let ctx = ProjectContext::new(buf.as_path(), &owned, at, workspace);
    SolidityAnalyzer.signature_help(&ctx)
}

#[test]
fn signature_plain_first_param_active_zero() {
    let text = "contract C {\nfunction greet(uint a, uint b) public { a; b; }\nfunction f() public {\ngreet(\n}\n}";
    let workspace = open_workspace(&[("/repo/C.sol", text)]);
    let found = signature_multi(&workspace, "/repo/C.sol", text, "greet(").expect("help");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].label, "function greet(uint a, uint b)");
    let labels: Vec<&str> = found[0]
        .params
        .iter()
        .map(|param| param.label.as_str())
        .collect();
    assert_eq!(labels, vec!["uint a", "uint b"]);
    assert_eq!(found[0].active_param, 0);
}

#[test]
fn signature_second_param_skips_string_comma() {
    let text = "contract C {\nfunction greet(string a, uint b) public {}\nfunction f() public {\ngreet(\"x,y\", \n}\n}";
    let workspace = open_workspace(&[("/repo/C.sol", text)]);
    let found = signature_multi(&workspace, "/repo/C.sol", text, "greet(\"x,y\", ").expect("help");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].active_param, 1);
}

#[test]
fn signature_unclosed_paren_still_helps() {
    let text = "contract C {\nfunction greet(uint a, uint b) public { a; b; }\nfunction f() public {\ngreet(1, 2";
    let workspace = open_workspace(&[("/repo/C.sol", text)]);
    let found = signature_multi(&workspace, "/repo/C.sol", text, "greet(1, 2").expect("help");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].label, "function greet(uint a, uint b)");
    assert_eq!(found[0].active_param, 1);
}

#[test]
fn signature_outside_paren_returns_none() {
    let text =
        "contract C {\nfunction greet(uint a) public { a; }\nfunction f() public {\ngreet(1)\n}\n}";
    let workspace = open_workspace(&[("/repo/C.sol", text)]);
    assert!(signature_multi(&workspace, "/repo/C.sol", text, "greet(1)").is_none());
}

#[test]
fn signature_unknown_callee_returns_none() {
    let text = "contract C {\nfunction f() public {\nmissing(1, \n}\n}";
    let workspace = open_workspace(&[("/repo/C.sol", text)]);
    assert!(signature_multi(&workspace, "/repo/C.sol", text, "missing(1, ").is_none());
}

#[test]
fn signature_member_call_shows_receiver_signature() {
    let text = "contract Hello {\nfunction greet(uint a, uint b) public { a; b; }\n}\ncontract C {\nfunction f() public {\nHello h;\nh.greet(\n}\n}";
    let workspace = open_workspace(&[("/repo/C.sol", text)]);
    let found = signature_multi(&workspace, "/repo/C.sol", text, "h.greet(").expect("help");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].label, "function greet(uint a, uint b)");
    assert_eq!(found[0].active_param, 0);
}

#[test]
fn signature_emit_call_shows_event_params() {
    let text = "contract C {\nevent Transfer(address from, uint amount);\nfunction f() public {\nemit Transfer(\n}\n}";
    let workspace = open_workspace(&[("/repo/C.sol", text)]);
    let found = signature_multi(&workspace, "/repo/C.sol", text, "emit Transfer(").expect("help");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].label, "event Transfer(address from, uint amount)");
    let labels: Vec<&str> = found[0]
        .params
        .iter()
        .map(|param| param.label.as_str())
        .collect();
    assert_eq!(labels, vec!["address from", "uint amount"]);
    assert_eq!(found[0].active_param, 0);
}

#[test]
fn signature_assert_builtin_inside_test_contract() {
    let text = "contract T is Test {\nfunction f() public {\nassertEq(\n}\n}";
    let workspace = open_workspace(&[("/repo/C.sol", text)]);
    let found = signature_multi(&workspace, "/repo/C.sol", text, "assertEq(").expect("help");
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].label,
        "function assertEq(uint256 left, uint256 right)"
    );
    let labels: Vec<&str> = found[0]
        .params
        .iter()
        .map(|param| param.label.as_str())
        .collect();
    assert_eq!(labels, vec!["uint256 left", "uint256 right"]);
    assert_eq!(found[0].active_param, 0);
}

#[test]
fn signature_vm_member_builtin_signature() {
    let text = "contract C {\nfunction f() public {\nvm.prank(\n}\n}";
    let workspace = open_workspace(&[("/repo/C.sol", text)]);
    let found = signature_multi(&workspace, "/repo/C.sol", text, "vm.prank(").expect("help");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].label, "function prank(address msgSender)");
    assert_eq!(found[0].active_param, 0);
}

#[test]
fn signature_overloads_return_all_signatures() {
    let text = "contract C {\nfunction greet(uint a) public { a; }\nfunction greet(uint a, uint b) public { a; b; }\nfunction f() public {\ngreet(\n}\n}";
    let workspace = open_workspace(&[("/repo/C.sol", text)]);
    let found = signature_multi(&workspace, "/repo/C.sol", text, "greet(").expect("help");
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].label, "function greet(uint a)");
    assert_eq!(found[1].label, "function greet(uint a, uint b)");
    assert_eq!(found[0].active_param, 0);
    assert_eq!(found[1].active_param, 0);
}

#[test]
fn array_dynamic_completion_lists_length_push_pop() {
    let text = "contract C {\nfunction f() public {\nuint[] memory arr;\narr.\n}\n}";
    let at = text.rfind("arr.").expect("dot") as u32 + 4;
    let labels = complete_labels(text, at);
    assert!(labels.contains(&"length".to_string()), "labels: {labels:?}");
    assert!(labels.contains(&"push".to_string()), "labels: {labels:?}");
    assert!(labels.contains(&"pop".to_string()), "labels: {labels:?}");
    let tokens = SolidityLexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    let items = complete_at(&file, text, at);
    let length = items
        .iter()
        .find(|item| item.label == "length")
        .expect("hit");
    assert_eq!(completion_item_kind(length.kind).value(), 5);
    assert!(
        length.detail.contains("uint256 length"),
        "detail: {}",
        length.detail
    );
    let push = items.iter().find(|item| item.label == "push").expect("hit");
    assert_eq!(completion_item_kind(push.kind).value(), 3);
    let pop = items.iter().find(|item| item.label == "pop").expect("hit");
    assert_eq!(completion_item_kind(pop.kind).value(), 3);
}

#[test]
fn array_fixed_completion_lists_length_only() {
    let text = "contract C {\nfunction f() public {\nuint[3] memory fixedArr;\nfixedArr.\n}\n}";
    let at = text.rfind("fixedArr.").expect("dot") as u32 + 9;
    let labels = complete_labels(text, at);
    assert!(labels.contains(&"length".to_string()), "labels: {labels:?}");
    assert!(!labels.contains(&"push".to_string()), "labels: {labels:?}");
    assert!(!labels.contains(&"pop".to_string()), "labels: {labels:?}");
}

#[test]
fn array_multidim_outer_suffix_decides_dynamic() {
    let dyn_text = "contract C {\nfunction f() public {\nuint[2][] dynMulti;\ndynMulti.\n}\n}";
    let dyn_at = dyn_text.rfind("dynMulti.").expect("dot") as u32 + 9;
    let dyn_labels = complete_labels(dyn_text, dyn_at);
    assert!(
        dyn_labels.contains(&"push".to_string()),
        "labels: {dyn_labels:?}"
    );
    assert!(
        dyn_labels.contains(&"pop".to_string()),
        "labels: {dyn_labels:?}"
    );
    let fixed_text =
        "contract C {\nfunction f() public {\nuint[][2] fixedMulti;\nfixedMulti.\n}\n}";
    let fixed_at = fixed_text.rfind("fixedMulti.").expect("dot") as u32 + 11;
    let fixed_labels = complete_labels(fixed_text, fixed_at);
    assert!(
        fixed_labels.contains(&"length".to_string()),
        "labels: {fixed_labels:?}"
    );
    assert!(
        !fixed_labels.contains(&"push".to_string()),
        "labels: {fixed_labels:?}"
    );
    assert!(
        !fixed_labels.contains(&"pop".to_string()),
        "labels: {fixed_labels:?}"
    );
}

#[test]
fn array_hover_shows_solidity_origin_with_member_span() {
    let text = "contract C {\nfunction f() public {\nuint[] memory arr;\narr.length;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("arr.length").expect("use") as u32 + 4;
    let found = hover_at(&file, text, at).expect("hover");
    assert_eq!(found.contents, "uint256 length - from solidity");
    assert!(found.span.contains(at));
    assert_eq!(found.span.start, at);
    assert_eq!(found.span.end, at + 6);
}

#[test]
fn array_goto_returns_none_and_skips_coincidental_dep() {
    let text = "contract C {\nfunction f() public {\nuint[] memory arr;\narr.length;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("arr.length").expect("use") as u32 + 4;
    assert!(goto_definition(&file, text, at).is_none());
    let dep = "contract B {\nfunction length() public {}\nfunction push() public {}\n}";
    let cur = "import \"./B.sol\";\ncontract C {\nfunction f() public {\nuint[] memory arr;\narr.length;\narr.push;\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", cur), ("/B.sol", dep)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    assert!(goto_multi(&workspace, "/A.sol", cur, "length").is_none());
    assert!(goto_multi(&workspace, "/A.sol", cur, "push").is_none());
}

#[test]
fn array_push_wrong_arity_reports_exact_message() {
    let text = "contract C {\nfunction f() public {\nuint[] memory arr;\narr.push();\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this function 'push' takes 1 arguments but 0 supplied"
    );
    let covered = &text[found[0].span.start as usize..found[0].span.end as usize];
    assert!(
        covered.starts_with("arr.push") && covered.ends_with(')'),
        "span: {covered}"
    );
    let ok = "contract C {\nfunction f() public {\nuint[] memory arr;\narr.push(1);\n}\n}";
    assert!(diagnostics_of(ok).is_empty());
}

#[test]
fn array_pop_wrong_arity_reports_exact_message() {
    let text = "contract C {\nfunction f() public {\nuint[] memory arr;\narr.pop(1);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this function 'pop' takes 0 arguments but 1 supplied"
    );
    let ok = "contract C {\nfunction f() public {\nuint[] memory arr;\narr.pop();\n}\n}";
    assert!(diagnostics_of(ok).is_empty());
}

#[test]
fn array_length_and_fixed_calls_stay_silent() {
    let text = "contract C {\nfunction f() public {\nuint[] memory arr;\nuint[3] memory fixedArr;\narr.length(1);\nfixedArr.push(1);\nfixedArr.pop();\narr.bogus(1, 2);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn array_memory_param_still_resolves() {
    let text = "contract C {\nfunction f(uint[] memory vals) public {\nvals.\n}\n}";
    let at = text.rfind("vals.").expect("dot") as u32 + 5;
    let labels = complete_labels(text, at);
    assert!(labels.contains(&"length".to_string()), "labels: {labels:?}");
    assert!(labels.contains(&"push".to_string()), "labels: {labels:?}");
    assert!(labels.contains(&"pop".to_string()), "labels: {labels:?}");
}

#[test]
fn array_cross_file_members_stay_empty() {
    let dep = "contract Hello {\nfunction greet() public {}\n}";
    let cur = "import \"./H.sol\";\ncontract C {\nfunction f() public {\nuint[] memory arr;\narr.length;\narr.\n}\n}";
    let mut workspace = open_workspace(&[("/H.sol", dep), ("/C.sol", cur)]);
    workspace.set_file_deps(PathBuf::from("/C.sol"), vec![PathBuf::from("/H.sol")]);
    let found = member_complete_multi(&workspace, "/C.sol", cur, "arr.");
    let labels: Vec<&str> = found.iter().map(|item| item.0.as_str()).collect();
    assert!(labels.contains(&"length"), "labels: {labels:?}");
    assert!(!labels.contains(&"greet"), "labels: {labels:?}");
    assert!(hover_multi(&workspace, "/C.sol", cur, "length").is_some());
}

#[test]
fn address_completion_lists_plain_six_members() {
    let text = "contract C {\nfunction f() public {\naddress wallet;\nwallet.\n}\n}";
    let at = text.rfind("wallet.").expect("dot") as u32 + 7;
    let labels = complete_labels(text, at);
    for want in [
        "balance",
        "code",
        "codehash",
        "call",
        "delegatecall",
        "staticcall",
    ] {
        assert!(labels.contains(&want.to_string()), "labels: {labels:?}");
    }
    assert!(
        !labels.contains(&"transfer".to_string()),
        "labels: {labels:?}"
    );
    assert!(!labels.contains(&"send".to_string()), "labels: {labels:?}");
    let tokens = SolidityLexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    let items = complete_at(&file, text, at);
    let balance = items
        .iter()
        .find(|item| item.label == "balance")
        .expect("hit");
    assert_eq!(completion_item_kind(balance.kind).value(), 5);
    assert!(
        balance.detail.contains("uint256 balance"),
        "detail: {}",
        balance.detail
    );
    let call = items.iter().find(|item| item.label == "call").expect("hit");
    assert_eq!(completion_item_kind(call.kind).value(), 3);
}

#[test]
fn address_payable_lists_eight_members() {
    let text = "contract C {\nfunction f() public {\naddress payable wallet;\nwallet.\n}\n}";
    let at = text.rfind("wallet.").expect("dot") as u32 + 7;
    let labels = complete_labels(text, at);
    for want in [
        "balance",
        "code",
        "codehash",
        "transfer",
        "send",
        "call",
        "delegatecall",
        "staticcall",
    ] {
        assert!(labels.contains(&want.to_string()), "labels: {labels:?}");
    }
}

#[test]
fn address_array_stays_array() {
    let text = "contract C {\nfunction f() public {\naddress[] wallets;\nwallets.\n}\n}";
    let at = text.rfind("wallets.").expect("dot") as u32 + 8;
    let labels = complete_labels(text, at);
    assert!(labels.contains(&"length".to_string()), "labels: {labels:?}");
    assert!(labels.contains(&"push".to_string()), "labels: {labels:?}");
    assert!(
        !labels.contains(&"balance".to_string()),
        "labels: {labels:?}"
    );
    assert!(
        !labels.contains(&"transfer".to_string()),
        "labels: {labels:?}"
    );
}

#[test]
fn address_hover_shows_solidity_origin_with_member_span() {
    let text = "contract C {\nfunction f() public {\naddress wallet;\nwallet.balance;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("wallet.balance").expect("use") as u32 + 7;
    let found = hover_at(&file, text, at).expect("hover");
    assert_eq!(found.contents, "uint256 balance - from solidity");
    assert!(found.span.contains(at));
    assert_eq!(found.span.start, at);
    assert_eq!(found.span.end, at + 7);
}

#[test]
fn address_goto_returns_none_and_skips_coincidental_dep() {
    let text = "contract C {\nfunction f() public {\naddress wallet;\nwallet.balance;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("wallet.balance").expect("use") as u32 + 7;
    assert!(goto_definition(&file, text, at).is_none());
    let dep = "contract B {\nfunction balance() public {}\nfunction transfer() public {}\n}";
    let cur = "import \"./B.sol\";\ncontract C {\nfunction f() public {\naddress wallet;\nwallet.balance;\nwallet.transfer;\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", cur), ("/B.sol", dep)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    assert!(goto_multi(&workspace, "/A.sol", cur, "balance").is_none());
    assert!(goto_multi(&workspace, "/A.sol", cur, "transfer").is_none());
}

#[test]
fn address_methods_wrong_arity_reports_exact_message() {
    for name in ["call", "delegatecall", "staticcall"] {
        let text = format!(
            "contract C {{\nfunction f() public {{\naddress wallet;\nwallet.{name}();\n}}\n}}"
        );
        let found = diagnostics_of(&text);
        assert_eq!(found.len(), 1, "{name}: {found:?}");
        assert_eq!(
            found[0].message,
            format!("this function '{name}' takes 1 arguments but 0 supplied")
        );
        let covered = &text[found[0].span.start as usize..found[0].span.end as usize];
        assert!(
            covered.starts_with(&format!("wallet.{name}")) && covered.ends_with(')'),
            "span: {covered}"
        );
        let ok = format!(
            "contract C {{\nfunction f() public {{\naddress wallet;\nwallet.{name}(1);\n}}\n}}"
        );
        assert!(diagnostics_of(&ok).is_empty(), "{name}: ok");
    }
}

#[test]
fn address_payable_methods_wrong_arity_reports_exact_message() {
    for name in ["transfer", "send", "call", "delegatecall", "staticcall"] {
        let text = format!(
            "contract C {{\nfunction f() public {{\naddress payable wallet;\nwallet.{name}();\n}}\n}}"
        );
        let found = diagnostics_of(&text);
        assert_eq!(found.len(), 1, "{name}: {found:?}");
        assert_eq!(
            found[0].message,
            format!("this function '{name}' takes 1 arguments but 0 supplied")
        );
        let ok = format!(
            "contract C {{\nfunction f() public {{\naddress payable wallet;\nwallet.{name}(1);\n}}\n}}"
        );
        assert!(diagnostics_of(&ok).is_empty(), "{name}: ok");
    }
}

#[test]
fn address_plain_transfer_and_send_stay_silent() {
    let text = "contract C {\nfunction f() public {\naddress wallet;\nwallet.transfer(1);\nwallet.send(1);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at_transfer = text.rfind("transfer").expect("use") as u32;
    assert!(hover_at(&file, text, at_transfer).is_none());
    let at_send = text.rfind("send").expect("use") as u32;
    assert!(hover_at(&file, text, at_send).is_none());
}

#[test]
fn address_payable_table_splits_eight_vs_six() {
    let payable = super::address::address_members(true);
    let plain = super::address::address_members(false);
    assert_eq!(payable.len(), 8);
    assert_eq!(plain.len(), 6);
    let payable_names: Vec<&str> = payable.iter().map(|item| item.name.as_str()).collect();
    let plain_names: Vec<&str> = plain.iter().map(|item| item.name.as_str()).collect();
    for want in [
        "balance",
        "code",
        "codehash",
        "transfer",
        "send",
        "call",
        "delegatecall",
        "staticcall",
    ] {
        assert!(payable_names.contains(&want), "payable: {payable_names:?}");
    }
    for want in [
        "balance",
        "code",
        "codehash",
        "call",
        "delegatecall",
        "staticcall",
    ] {
        assert!(plain_names.contains(&want), "plain: {plain_names:?}");
    }
    assert!(!plain_names.contains(&"transfer"));
    assert!(!plain_names.contains(&"send"));
}

#[test]
fn address_payable_transfer_hover_shows_detail() {
    let text =
        "contract C {\nfunction f() public {\naddress payable wallet;\nwallet.transfer;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("transfer").expect("use") as u32;
    let found = hover_at(&file, text, at).expect("hover");
    assert_eq!(
        found.contents,
        "function transfer(uint256 amount) - from solidity"
    );
}

#[test]
fn address_fields_and_unknown_calls_stay_silent() {
    let text = "contract C {\nfunction f() public {\naddress wallet;\nwallet.balance(1);\nwallet.code(1);\nwallet.codehash(1);\nwallet.bogus(1, 2);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn address_cross_file_members_stay_empty() {
    let dep = "contract Hello {\nfunction greet() public {}\n}";
    let cur = "import \"./H.sol\";\ncontract C {\nfunction f() public {\naddress wallet;\nwallet.balance;\nwallet.\n}\n}";
    let mut workspace = open_workspace(&[("/H.sol", dep), ("/C.sol", cur)]);
    workspace.set_file_deps(PathBuf::from("/C.sol"), vec![PathBuf::from("/H.sol")]);
    let found = member_complete_multi(&workspace, "/C.sol", cur, "wallet.");
    let labels: Vec<&str> = found.iter().map(|item| item.0.as_str()).collect();
    assert!(labels.contains(&"balance"), "labels: {labels:?}");
    assert!(!labels.contains(&"greet"), "labels: {labels:?}");
    assert!(hover_multi(&workspace, "/C.sol", cur, "balance").is_some());
}

#[test]
fn contract_instance_completion_lists_state_and_function() {
    let text = "contract Hello {\nuint total;\nfunction greet(uint a) public { a; }\n}\ncontract C {\nfunction f() public {\nHello h;\nh.\n}\n}";
    let at = text.rfind("h.").expect("dot") as u32 + 2;
    let labels = complete_labels(text, at);
    assert!(labels.contains(&"total".to_string()), "labels: {labels:?}");
    assert!(labels.contains(&"greet".to_string()), "labels: {labels:?}");
}

#[test]
fn contract_instance_completion_filters_by_prefix() {
    let text = "contract Hello {\nuint total;\nfunction greet(uint a) public { a; }\n}\ncontract C {\nfunction f() public {\nHello h;\nh.gre\n}\n}";
    let at = text.rfind("h.gre").expect("dot") as u32 + 5;
    let labels = complete_labels(text, at);
    assert!(labels.contains(&"greet".to_string()), "labels: {labels:?}");
    assert!(!labels.contains(&"total".to_string()), "labels: {labels:?}");
}

#[test]
fn contract_instance_hover_shows_function_detail() {
    let text = "contract Hello {\nfunction greet(uint a) public { a; }\n}\ncontract C {\nfunction f() public {\nHello h;\nh.greet;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("h.greet").expect("use") as u32 + 2;
    let found = hover_at(&file, text, at).expect("hover");
    assert_eq!(found.contents, "function greet(uint a)");
}

#[test]
fn contract_instance_goto_jumps_to_decl() {
    let text = "contract Hello {\nfunction greet(uint a) public { a; }\n}\ncontract C {\nfunction f() public {\nHello h;\nh.greet;\n}\n}";
    let at_use = text.rfind("h.greet").expect("use") as u32 + 2;
    let at_decl = text.find("greet").expect("decl") as u32;
    let target = goto_target(text, at_use);
    assert!(target.start <= at_decl && at_decl < target.end);
}

#[test]
fn contract_instance_state_var_receiver_resolves() {
    let text = "contract Hello {\nfunction greet() public {}\n}\ncontract C {\nHello hState;\nfunction f() public {\nhState.\n}\n}";
    let at = text.rfind("hState.").expect("dot") as u32 + 7;
    let labels = complete_labels(text, at);
    assert!(labels.contains(&"greet".to_string()), "labels: {labels:?}");
}

#[test]
fn contract_instance_param_receiver_resolves() {
    let text = "contract Hello {\nfunction greet() public {}\n}\ncontract C {\nfunction f(Hello hParam) public {\nhParam.\n}\n}";
    let at = text.rfind("hParam.").expect("dot") as u32 + 7;
    let labels = complete_labels(text, at);
    assert!(labels.contains(&"greet".to_string()), "labels: {labels:?}");
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let hover_at_pos = text.rfind("hParam.").expect("use") as u32 + 7;
    let items = complete_at(&file, text, hover_at_pos);
    assert!(items.iter().any(|item| item.label == "greet"));
}

#[test]
fn cross_file_instance_hover_includes_origin() {
    let dep = "contract Hello {\nfunction greet(uint a) public { a; }\n}";
    let cur = "import \"./H.sol\";\ncontract C {\nfunction f() public {\nHello h;\nh.greet;\n}\n}";
    let mut workspace = open_workspace(&[("/H.sol", dep), ("/C.sol", cur)]);
    workspace.set_file_deps(PathBuf::from("/C.sol"), vec![PathBuf::from("/H.sol")]);
    let hit = hover_multi(&workspace, "/C.sol", cur, "greet").expect("hover");
    assert!(
        hit.contents.contains("function greet"),
        "contents: {}",
        hit.contents
    );
    assert!(
        hit.contents.contains("from Hello in H.sol"),
        "contents: {}",
        hit.contents
    );
}

#[test]
fn cross_file_instance_goto_jumps_to_dep() {
    let dep = "contract Hello {\nfunction greet(uint a) public { a; }\n}";
    let cur = "import \"./H.sol\";\ncontract C {\nfunction f() public {\nHello h;\nh.greet;\n}\n}";
    let mut workspace = open_workspace(&[("/H.sol", dep), ("/C.sol", cur)]);
    workspace.set_file_deps(PathBuf::from("/C.sol"), vec![PathBuf::from("/H.sol")]);
    let hit = goto_multi(&workspace, "/C.sol", cur, "greet").expect("goto");
    assert_eq!(hit.path, PathBuf::from("/H.sol"));
    let want = dep.find("greet").expect("decl") as u32;
    assert!(hit.span.contains(want));
}

#[test]
fn cross_file_instance_signature_shows_dep() {
    let dep = "contract Hello {\nfunction greet(uint a, uint b) public { a; b; }\n}";
    let cur = "import \"./H.sol\";\ncontract C {\nfunction f() public {\nHello h;\nh.greet(\n}\n}";
    let mut workspace = open_workspace(&[("/H.sol", dep), ("/C.sol", cur)]);
    workspace.set_file_deps(PathBuf::from("/C.sol"), vec![PathBuf::from("/H.sol")]);
    let found = signature_multi(&workspace, "/C.sol", cur, "h.greet(").expect("help");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].label, "function greet(uint a, uint b)");
    assert_eq!(found[0].active_param, 0);
}

#[test]
fn inherited_via_instance_shows_base() {
    let text = "contract B {\nuint baseVar;\nfunction baseFn(uint a) public {}\n}\ncontract D is B {\n}\ncontract C {\nfunction f() public {\nD d;\nd.\n}\n}";
    let at = text.rfind("d.").expect("dot") as u32 + 2;
    let labels = complete_labels(text, at);
    assert!(labels.contains(&"baseFn".to_string()), "labels: {labels:?}");
    assert!(
        labels.contains(&"baseVar".to_string()),
        "labels: {labels:?}"
    );
    let use_text = "contract B {\nuint baseVar;\nfunction baseFn(uint a) public {}\n}\ncontract D is B {\n}\ncontract C {\nfunction f() public {\nD d;\nd.baseFn;\n}\n}";
    let use_tokens = SolidityLexer.tokenize(use_text);
    let (use_file, use_errors) = SolidityParser.parse_with_errors(&use_tokens);
    assert!(use_errors.is_empty(), "errors: {use_errors:?}");
    let use_at = use_text.rfind("d.baseFn").expect("use") as u32 + 2;
    let found = hover_at(&use_file, use_text, use_at).expect("hover");
    assert!(
        found.contents.contains("function baseFn"),
        "contents: {}",
        found.contents
    );
    assert!(
        found.contents.contains("from B"),
        "contents: {}",
        found.contents
    );
}

#[test]
fn contract_instance_enum_receiver_stays_silent() {
    let text = "contract C {\nenum Color { Red, Blue }\nfunction f() public {\nColor c;\nc.\n}\n}";
    let at = text.rfind("c.").expect("dot") as u32 + 2;
    let labels = complete_labels(text, at);
    assert!(labels.is_empty(), "labels: {labels:?}");
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let hover_pos = text.rfind("c.").expect("use") as u32 + 2;
    assert!(hover_at(&file, text, hover_pos).is_none());
    assert!(goto_definition(&file, text, hover_pos).is_none());
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn contract_instance_mapping_receiver_stays_silent() {
    let text =
        "contract C {\nmapping(address => uint) balances;\nfunction f() public {\nbalances.\n}\n}";
    let at = text.rfind("balances.").expect("dot") as u32 + 9;
    let labels = complete_labels(text, at);
    assert!(labels.is_empty(), "labels: {labels:?}");
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert!(hover_at(&file, text, at).is_none());
    assert!(goto_definition(&file, text, at).is_none());
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn instance_hides_events_errors_modifiers() {
    let text = "contract Hello {\nevent Transfer(address a);\nerror Oops();\nmodifier onlyOwner() { _; }\nfunction greet() public {}\nuint total;\n}\ncontract C {\nfunction f() public {\nHello h;\nh.\n}\n}";
    let at = text.rfind("h.").expect("dot") as u32 + 2;
    let labels = complete_labels(text, at);
    assert!(labels.contains(&"greet".to_string()), "labels: {labels:?}");
    assert!(labels.contains(&"total".to_string()), "labels: {labels:?}");
    assert!(
        !labels.contains(&"Transfer".to_string()),
        "labels: {labels:?}"
    );
    assert!(!labels.contains(&"Oops".to_string()), "labels: {labels:?}");
    assert!(
        !labels.contains(&"onlyOwner".to_string()),
        "labels: {labels:?}"
    );
}

#[test]
fn variable_completion_shows_param_and_local_types() {
    let text = "contract C {\nfunction f(uint amount) public {\nuint count = 1;\namo\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at_param = text.rfind("amo").expect("prefix") as u32 + 3;
    let items = complete_at(&file, text, at_param);
    let hit = items
        .iter()
        .find(|item| item.label == "amount")
        .expect("param hit");
    assert_eq!(hit.detail, "uint amount");
    assert_eq!(completion_item_kind(hit.kind).value(), 6);
    let local_text = "contract C {\nfunction f(uint amount) public {\nuint count = 1;\ncou\n}\n}";
    let local_tokens = SolidityLexer.tokenize(local_text);
    let (local_file, local_errors) = SolidityParser.parse_with_errors(&local_tokens);
    assert!(local_errors.is_empty(), "errors: {local_errors:?}");
    let at_local = local_text.rfind("cou").expect("prefix") as u32 + 3;
    let local_items = complete_at(&local_file, local_text, at_local);
    let local_hit = local_items
        .iter()
        .find(|item| item.label == "count")
        .expect("local hit");
    assert_eq!(local_hit.detail, "uint count = 1");
    assert_eq!(completion_item_kind(local_hit.kind).value(), 6);
}

#[test]
fn member_completion_shows_state_and_field_types() {
    let text = "contract Hello {\nuint total;\nfunction greet(uint a) public { a; }\n}\ncontract C {\nfunction f() public {\nHello h;\nh.\n}\n}";
    let at = text.rfind("h.").expect("dot") as u32 + 2;
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let items = complete_at(&file, text, at);
    let total = items
        .iter()
        .find(|item| item.label == "total")
        .expect("total hit");
    assert_eq!(total.detail, "uint total");
    assert_eq!(completion_item_kind(total.kind).value(), 6);
    let struct_text = "struct Person {\nuint age;\nstring name;\n}\ncontract C {\nfunction f() public {\nPerson p;\np.\n}\n}";
    let struct_at = struct_text.rfind("p.").expect("dot") as u32 + 2;
    let struct_tokens = SolidityLexer.tokenize(struct_text);
    let (struct_file, struct_errors) = SolidityParser.parse_with_errors(&struct_tokens);
    assert!(struct_errors.is_empty(), "errors: {struct_errors:?}");
    let struct_items = complete_at(&struct_file, struct_text, struct_at);
    let age = struct_items
        .iter()
        .find(|item| item.label == "age")
        .expect("age hit");
    assert_eq!(age.detail, "uint age");
    let name = struct_items
        .iter()
        .find(|item| item.label == "name")
        .expect("name hit");
    assert_eq!(name.detail, "string name");
}

#[test]
fn this_completion_shows_state_var_type() {
    let text =
        "contract C {\nuint total;\nfunction foo() public {}\nfunction f() public {\nthis.\n}\n}";
    let at = text.rfind("this.").expect("dot") as u32 + 5;
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let items = complete_at(&file, text, at);
    let total = items
        .iter()
        .find(|item| item.label == "total")
        .expect("total hit");
    assert_eq!(total.detail, "uint total");
    let foo = items
        .iter()
        .find(|item| item.label == "foo")
        .expect("foo hit");
    assert!(
        foo.detail.contains("function foo"),
        "detail: {}",
        foo.detail
    );
}

#[test]
fn cross_file_member_state_var_shows_type_and_origin() {
    let dep = "contract Hello {\nuint total;\n}";
    let cur = "import \"./H.sol\";\ncontract C {\nfunction f() public {\nHello h;\nh.\n}\n}";
    let mut workspace = open_workspace(&[("/H.sol", dep), ("/C.sol", cur)]);
    workspace.set_file_deps(PathBuf::from("/C.sol"), vec![PathBuf::from("/H.sol")]);
    let found = member_complete_multi(&workspace, "/C.sol", cur, "h.");
    let hit = found
        .iter()
        .find(|(label, _)| label == "total")
        .expect("total hit");
    assert!(hit.1.starts_with("uint total"), "detail: {}", hit.1);
    assert!(hit.1.contains("from Hello in H.sol"), "detail: {}", hit.1);
}

#[test]
fn hover_shows_param_and_local_types() {
    let text =
        "contract C {\nfunction f(uint amount) public {\nuint count = 1;\namount;\ncount;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at_param = text.rfind("amount;").expect("use") as u32;
    let param = hover_at(&file, text, at_param).expect("hover");
    assert_eq!(param.contents, "uint amount");
    let decl_param = text.find("amount").expect("decl") as u32;
    assert!(param.span.start <= decl_param && decl_param < param.span.end);
    let at_local = text.rfind("count;").expect("use") as u32;
    let local = hover_at(&file, text, at_local).expect("hover");
    assert_eq!(local.contents, "uint count = 1");
    let decl_local = text.find("count").expect("decl") as u32;
    assert!(local.span.start <= decl_local && decl_local < local.span.end);
}

#[test]
fn cross_file_member_hover_shows_var_type_and_origin() {
    let dep = "contract Hello {\nuint total;\n}";
    let cur = "import \"./H.sol\";\ncontract C {\nfunction f() public {\nHello h;\nh.total;\n}\n}";
    let mut workspace = open_workspace(&[("/H.sol", dep), ("/C.sol", cur)]);
    workspace.set_file_deps(PathBuf::from("/C.sol"), vec![PathBuf::from("/H.sol")]);
    let hit = hover_multi(&workspace, "/C.sol", cur, "total").expect("hover");
    assert!(
        hit.contents.contains("uint total"),
        "contents: {}",
        hit.contents
    );
    assert!(
        hit.contents.contains("from Hello in H.sol"),
        "contents: {}",
        hit.contents
    );
}

#[test]
fn non_variable_details_stay_keyword_and_file() {
    let text = "contract C {\nfun";
    let at = text.len() as u32;
    let tokens = SolidityLexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    let items = complete_at(&file, text, at);
    let hit = items
        .iter()
        .find(|item| item.label == "function")
        .expect("keyword hit");
    assert_eq!(hit.detail, "keyword");
    assert_eq!(completion_item_kind(hit.kind).value(), 14);
    let root = fresh_sol_root("forgels-sol-var-nonvar");
    let current = root.join("A.sol");
    let paths = complete_import_paths(&current, std::slice::from_ref(&root), "");
    assert!(!paths.is_empty());
    assert!(paths.iter().all(|item| item.detail == "file"));
    assert!(
        paths
            .iter()
            .all(|item| completion_item_kind(item.kind).value() == 17)
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn bytes_completion_has_length_only() {
    let text = "contract C {\nbytes b;\nfunction f() public {\nb.\n}\n}";
    let at = text.rfind("b.").expect("dot") as u32 + 2;
    let labels = complete_labels(text, at);
    assert_eq!(labels, vec!["length".to_string()], "labels: {labels:?}");
}

#[test]
fn bytes_sized_completion_has_length_only() {
    let text = "contract C {\nbytes32 h;\nfunction f() public {\nh.\n}\n}";
    let at = text.rfind("h.").expect("dot") as u32 + 2;
    let labels = complete_labels(text, at);
    assert_eq!(labels, vec!["length".to_string()], "labels: {labels:?}");
}

#[test]
fn bytes_hover_shows_length_detail() {
    let text = "contract C {\nbytes b;\nfunction f() public {\nb.length;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("b.length").expect("use") as u32 + 2;
    let hover = hover_at(&file, text, at).expect("hover");
    assert_eq!(hover.contents, "uint256 length - from solidity");
}

#[test]
fn bytes_goto_returns_none() {
    let text = "contract C {\nbytes b;\nfunction f() public {\nb.length;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("b.length").expect("use") as u32 + 2;
    assert!(goto_definition(&file, text, at).is_none());
}

#[test]
fn bytes_length_call_stays_silent() {
    let text = "contract C {\nbytes b;\nfunction f() public {\nb.length(1);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn string_completion_is_empty() {
    let text = "contract C {\nstring s;\nfunction f() public {\ns.\n}\n}";
    let at = text.rfind("s.").expect("dot") as u32 + 2;
    let labels = complete_labels(text, at);
    assert!(labels.is_empty(), "labels: {labels:?}");
}

#[test]
fn string_member_hover_returns_none() {
    let text = "contract C {\nstring s;\nfunction f() public {\ns.length;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    let at = text.rfind("s.length").expect("use") as u32 + 2;
    assert!(hover_at(&file, text, at).is_none());
}

#[test]
fn function_member_completion_lists_selector_gas_value() {
    let text =
        "contract C {\nfunction greet(uint a) public { a; }\nfunction f() public {\ngreet.\n}\n}";
    let at = text.rfind("greet.").expect("dot") as u32 + 6;
    let labels = complete_labels(text, at);
    assert!(
        labels.contains(&"selector".to_string()),
        "labels: {labels:?}"
    );
    assert!(labels.contains(&"gas".to_string()), "labels: {labels:?}");
    assert!(labels.contains(&"value".to_string()), "labels: {labels:?}");
    assert_eq!(labels.len(), 3, "labels: {labels:?}");
}

#[test]
fn function_hover_shows_selector_detail() {
    let text = "contract C {\nfunction greet(uint a) public { a; }\nfunction f() public {\ngreet.selector;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("greet.selector").expect("use") as u32 + 6;
    let hover = hover_at(&file, text, at).expect("hover");
    assert_eq!(hover.contents, "bytes4 selector - from solidity");
}

#[test]
fn function_goto_returns_none() {
    let text = "contract C {\nfunction greet(uint a) public { a; }\nfunction f() public {\ngreet.selector;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("greet.selector").expect("use") as u32 + 6;
    assert!(goto_definition(&file, text, at).is_none());
}

#[test]
fn function_gas_correct_count_stays_silent() {
    let text = "contract C {\nfunction greet(uint a) public { a; }\nfunction f() public {\ngreet.gas(100);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn function_gas_wrong_count_reports() {
    let text = "contract C {\nfunction greet(uint a) public { a; }\nfunction f() public {\ngreet.gas();\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this function 'gas' takes 1 arguments but 0 supplied"
    );
}

#[test]
fn function_selector_call_stays_silent() {
    let text = "contract C {\nfunction greet(uint a) public { a; }\nfunction f() public {\ngreet.selector(1);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn type_contract_completion_lists_metadata() {
    let text = "contract C {\nfunction f() public {\ntype(C).\n}\n}";
    let at = text.rfind("type(C).").expect("dot") as u32 + 8;
    let labels = complete_labels(text, at);
    for member in ["name", "creationCode", "runtimeCode", "interfaceId"] {
        assert!(
            labels.contains(&member.to_string()),
            "{member} missing: {labels:?}"
        );
    }
}

#[test]
fn type_integer_completion_lists_min_max() {
    let text = "contract C {\nfunction f() public {\ntype(uint256).\n}\n}";
    let at = text.rfind("type(uint256).").expect("dot") as u32 + 14;
    let labels = complete_labels(text, at);
    assert_eq!(
        labels,
        vec!["max".to_string(), "min".to_string()],
        "labels: {labels:?}"
    );
}

#[test]
fn type_hover_shows_creation_code() {
    let text = "contract C {\nfunction f() public {\ntype(C).creationCode;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    let at = text.rfind("creationCode").expect("use") as u32;
    let hover = hover_at(&file, text, at).expect("hover");
    assert_eq!(hover.contents, "bytes creationCode - from solidity");
}

#[test]
fn type_goto_returns_none() {
    let text = "contract C {\nfunction f() public {\ntype(C).creationCode;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    let at = text.rfind("creationCode").expect("use") as u32;
    assert!(goto_definition(&file, text, at).is_none());
}

#[test]
fn msg_completion_lists_sender_value_data_sig() {
    let text = "contract C {\nfunction f() public {\nmsg.\n}\n}";
    let at = text.rfind("msg.").expect("dot") as u32 + 4;
    let labels = complete_labels(text, at);
    for member in ["sender", "value", "data", "sig"] {
        assert!(
            labels.contains(&member.to_string()),
            "{member} missing: {labels:?}"
        );
    }
    assert_eq!(labels.len(), 4, "labels: {labels:?}");
}

#[test]
fn msg_hover_shows_sender_detail() {
    let text = "contract C {\nfunction f() public {\nmsg.sender;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("msg.sender").expect("use") as u32 + 4;
    let hover = hover_at(&file, text, at).expect("hover");
    assert_eq!(hover.contents, "address sender - from solidity");
}

#[test]
fn msg_goto_returns_none() {
    let text = "contract C {\nfunction f() public {\nmsg.sender;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("msg.sender").expect("use") as u32 + 4;
    assert!(goto_definition(&file, text, at).is_none());
}

#[test]
fn block_completion_lists_chain_fields() {
    let text = "contract C {\nfunction f() public {\nblock.\n}\n}";
    let at = text.rfind("block.").expect("dot") as u32 + 6;
    let labels = complete_labels(text, at);
    for member in [
        "chainid",
        "coinbase",
        "number",
        "timestamp",
        "basefee",
        "blobhash",
    ] {
        assert!(
            labels.contains(&member.to_string()),
            "{member} missing: {labels:?}"
        );
    }
}

#[test]
fn tx_completion_lists_gasprice_origin() {
    let text = "contract C {\nfunction f() public {\ntx.\n}\n}";
    let at = text.rfind("tx.").expect("dot") as u32 + 3;
    let labels = complete_labels(text, at);
    assert_eq!(
        labels,
        vec!["gasprice".to_string(), "origin".to_string()],
        "labels: {labels:?}"
    );
}

#[test]
fn abi_completion_lists_encode_family() {
    let text = "contract C {\nfunction f() public {\nabi.\n}\n}";
    let at = text.rfind("abi.").expect("dot") as u32 + 4;
    let labels = complete_labels(text, at);
    for member in [
        "decode",
        "encode",
        "encodePacked",
        "encodeWithSelector",
        "encodeWithSignature",
        "encodeCall",
    ] {
        assert!(
            labels.contains(&member.to_string()),
            "{member} missing: {labels:?}"
        );
    }
}

#[test]
fn abi_hover_shows_decode_detail() {
    let text = "contract C {\nfunction f() public {\nabi.decode;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("abi.decode").expect("use") as u32 + 4;
    let hover = hover_at(&file, text, at).expect("hover");
    assert!(
        hover.contents.contains("function decode"),
        "contents: {}",
        hover.contents
    );
}

#[test]
fn abi_goto_returns_none() {
    let text = "contract C {\nfunction f() public {\nabi.decode;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("abi.decode").expect("use") as u32 + 4;
    assert!(goto_definition(&file, text, at).is_none());
}

#[test]
fn global_field_call_stays_silent() {
    let text = "contract C {\nfunction f() public {\nmsg.sender(1);\nblock.timestamp(1);\ntx.origin(1);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn blobhash_correct_count_stays_silent() {
    let text = "contract C {\nfunction f() public {\nblock.blobhash(1);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn blobhash_wrong_count_reports() {
    let text = "contract C {\nfunction f() public {\nblock.blobhash();\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this function 'blobhash' takes 1 arguments but 0 supplied"
    );
}

#[test]
fn abi_decode_wrong_count_reports() {
    let text = "contract C {\nbytes b;\nfunction f() public {\nabi.decode(b);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this function 'decode' takes 2 arguments but 1 supplied"
    );
}

#[test]
fn abi_variadic_calls_stay_silent() {
    let text = "contract C {\nfunction f() public {\nabi.encode(a, b, c);\nabi.encodePacked(a);\nabi.encodeWithSelector(s, a, b);\nabi.encodeWithSignature(s);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn abi_encode_call_wrong_count_reports() {
    let text = "contract C {\nbytes t;\nfunction f() public {\nabi.encodeCall(t);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this function 'encodeCall' takes 2 arguments but 1 supplied"
    );
}

#[test]
fn global_receiver_shadowed_by_user_var() {
    let text = "contract C {\nuint msg;\nfunction f() public {\nmsg.\n}\n}";
    let at = text.rfind("msg.").expect("dot") as u32 + 4;
    let labels = complete_labels(text, at);
    assert!(
        !labels.contains(&"sender".to_string()),
        "labels: {labels:?}"
    );
}

#[test]
fn global_member_access_stays_silent_in_diagnostics() {
    let text = "contract C {\nfunction f() public {\nmsg.sender;\nblock.timestamp;\ntx.origin;\nabi.decode;\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn builtin_members_never_jump_to_dep_declarations() {
    let dep = "contract D {\nfunction sender() public {}\nfunction length() public {}\nfunction selector() public {}\n}";
    let cur = "contract C {\nbytes b;\nfunction greet(uint a) public { a; }\nfunction f() public {\nmsg.sender;\nb.length;\ngreet.selector;\n}\n}";
    let mut workspace = open_workspace(&[("/D.sol", dep), ("/C.sol", cur)]);
    workspace.set_file_deps(PathBuf::from("/C.sol"), vec![PathBuf::from("/D.sol")]);
    for needle in ["msg.sender", "b.length", "greet.selector"] {
        let owned = cur.to_string();
        let buf = PathBuf::from("/C.sol");
        let at = owned.rfind(needle).expect("needle") as u32
            + needle.rfind('.').expect("dot") as u32
            + 1;
        let ctx = ProjectContext::new(buf.as_path(), &owned, at, &workspace);
        assert!(
            SolidityAnalyzer.goto_location(&ctx).is_none(),
            "{needle} jumped"
        );
    }
}

#[test]
fn type_member_never_jumps_to_dep_declarations() {
    let dep = "contract D {\nfunction creationCode() public {}\n}";
    let cur = "contract C {\nfunction f() public {\ntype(C).creationCode;\n}\n}";
    let mut workspace = open_workspace(&[("/D.sol", dep), ("/C.sol", cur)]);
    workspace.set_file_deps(PathBuf::from("/C.sol"), vec![PathBuf::from("/D.sol")]);
    let owned = cur.to_string();
    let buf = PathBuf::from("/C.sol");
    let at = owned.rfind("creationCode").expect("needle") as u32;
    let ctx = ProjectContext::new(buf.as_path(), &owned, at, &workspace);
    assert!(SolidityAnalyzer.goto_location(&ctx).is_none());
}

fn outline_of(text: &str) -> Vec<forgels_core::DocumentSymbolData> {
    let tokens = SolidityLexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    super::document_symbols_in(&file)
}

#[test]
fn outline_nests_contract_function_param() {
    let text = "contract C {\nfunction f(uint a) public {}\n}";
    let symbols = outline_of(text);
    assert_eq!(symbols.len(), 1);
    let contract = &symbols[0];
    assert_eq!(contract.name, "C");
    assert_eq!(contract.kind, forgels_protocol::SymbolKind::CLASS);
    assert_eq!(contract.kind.value(), 5);
    assert_eq!(contract.range, Span::new(0, text.len() as u32));
    let name_at = text.find('C').expect("name") as u32;
    assert_eq!(contract.selection, Span::new(name_at, name_at + 1));
    assert!(contract.selection.start >= contract.range.start);
    assert!(contract.selection.end <= contract.range.end);
    assert_eq!(contract.children.len(), 1);
    let function = &contract.children[0];
    assert_eq!(function.name, "f");
    assert_eq!(function.kind, forgels_protocol::SymbolKind::METHOD);
    assert_eq!(function.kind.value(), 6);
    assert!(function.range.start >= contract.range.start);
    assert!(function.range.end <= contract.range.end);
    let fn_at = (text.find("function f").expect("function") + "function ".len()) as u32;
    assert_eq!(function.selection, Span::new(fn_at, fn_at + 1));
    assert_eq!(function.children.len(), 1);
    let param = &function.children[0];
    assert_eq!(param.name, "a");
    assert_eq!(param.kind.value(), 13);
    assert_eq!(param.detail, "uint a");
    assert!(param.children.is_empty());
}

#[test]
fn outline_struct_enum_event_shapes() {
    let text =
        "struct S {\nuint x;\n}\nenum E {\nA,\nB\n}\nevent Transfer(address from, uint amount);";
    let symbols = outline_of(text);
    assert_eq!(symbols.len(), 3);
    let strukt = &symbols[0];
    assert_eq!(strukt.name, "S");
    assert_eq!(strukt.kind.value(), 23);
    assert_eq!(strukt.children.len(), 1);
    assert_eq!(strukt.children[0].name, "x");
    assert_eq!(strukt.children[0].kind.value(), 8);
    let enumeration = &symbols[1];
    assert_eq!(enumeration.name, "E");
    assert_eq!(enumeration.kind.value(), 10);
    let variants: Vec<&str> = enumeration
        .children
        .iter()
        .map(|child| child.name.as_str())
        .collect();
    assert_eq!(variants, vec!["A", "B"]);
    for child in &enumeration.children {
        assert_eq!(child.kind.value(), 22);
        assert_eq!(child.range, child.selection);
    }
    let event = &symbols[2];
    assert_eq!(event.name, "Transfer");
    assert_eq!(event.kind.value(), 24);
    let params: Vec<&str> = event
        .children
        .iter()
        .map(|child| child.name.as_str())
        .collect();
    assert_eq!(params, vec!["from", "amount"]);
}

#[test]
fn outline_modifier_error_constructor_kinds() {
    let text = "contract C {\nconstructor(uint a) { a; }\nmodifier onlyOwner() {\n_\n}\nerror Unauthorized();\n}";
    let symbols = outline_of(text);
    assert_eq!(symbols.len(), 1);
    let kinds: Vec<(String, u8)> = symbols[0]
        .children
        .iter()
        .map(|child| (child.name.clone(), child.kind.value()))
        .collect();
    assert!(kinds.contains(&("constructor".to_string(), 6)));
    assert!(kinds.contains(&("onlyOwner".to_string(), 6)));
    assert!(kinds.contains(&("Unauthorized".to_string(), 13)));
    let constructor = symbols[0]
        .children
        .iter()
        .find(|child| child.name == "constructor")
        .expect("constructor");
    assert_eq!(constructor.children.len(), 1);
    assert_eq!(constructor.children[0].name, "a");
}

#[test]
fn outline_keeps_partial_results_on_broken_input() {
    let text = "contract C {\nfunction ok() public {}\nfunction broken(uint ;\n}";
    let symbols = outline_of(text);
    assert_eq!(symbols.len(), 1);
    let names: Vec<&str> = symbols[0]
        .children
        .iter()
        .map(|child| child.name.as_str())
        .collect();
    assert!(names.contains(&"ok"));
    assert!(!names.contains(&"broken"));
}

#[test]
fn outline_state_var_uses_field_kind() {
    let text = "contract C {\nuint256 total;\n}";
    let symbols = outline_of(text);
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].children.len(), 1);
    assert_eq!(symbols[0].children[0].name, "total");
    assert_eq!(symbols[0].children[0].kind.value(), 8);
}

#[test]
fn visibility_external_plain_call_errors() {
    let text = "contract C {\nfunction ext() external {}\nfunction f() public {\next();\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "external function 'ext' must be called via 'this.ext()' or 'C.ext()'"
    );
}

#[test]
fn visibility_external_via_this_stays_silent() {
    let text = "contract C {\nfunction ext() external {}\nfunction f() public {\nthis.ext();\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn visibility_external_via_contract_stays_silent() {
    let text = "contract C {\nfunction ext() external {}\nfunction f() public {\nC.ext();\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn visibility_public_plain_call_stays_silent() {
    let text = "contract C {\nfunction pubf() public {}\nfunction f() public {\npubf();\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn visibility_inherited_external_errors() {
    let text = "contract B {\nfunction ext() external {}\n}\ncontract D is B {\nfunction f() public {\next();\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "external function 'ext' must be called via 'this.ext()' or 'D.ext()'"
    );
}

#[test]
fn visibility_derived_override_wins() {
    let text = "contract B {\nfunction ext() external {}\n}\ncontract D is B {\nfunction ext() public {}\nfunction f() public {\next();\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn mutability_pure_read_errors() {
    let text = "contract C {\nuint x;\nfunction f() pure {\nx;\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "pure function 'f' cannot read state variable 'x'"
    );
}

#[test]
fn mutability_pure_write_errors() {
    let text = "contract C {\nuint x;\nfunction f() pure {\nx = 1;\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "pure function 'f' cannot write to state variable 'x'"
    );
}

#[test]
fn mutability_view_read_stays_silent() {
    let text = "contract C {\nuint x;\nfunction f() view {\nx;\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn mutability_view_write_errors() {
    let text = "contract C {\nuint x;\nfunction f() view {\nx = 1;\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "view function 'f' cannot write to state variable 'x'"
    );
}

#[test]
fn mutability_plain_function_stays_silent() {
    let text = "contract C {\nuint x;\nfunction f() public {\nx;\nx = 1;\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn mutability_param_shadow_reports_shadow() {
    let text = "contract C {\nuint x;\nfunction f(uint x) pure {\nx;\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(found[0].message, "parameter 'x' shadows outer declaration");
}

#[test]
fn mutability_this_read_in_pure_errors() {
    let text = "contract C {\nuint x;\nfunction f() pure {\nthis.x;\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "pure function 'f' cannot read state variable 'x'"
    );
}

#[test]
fn mutability_this_write_in_view_errors() {
    let text = "contract C {\nuint x;\nfunction f() view {\nthis.x = 1;\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "view function 'f' cannot write to state variable 'x'"
    );
}

#[test]
fn mutability_incr_write_in_view_errors() {
    let text = "contract C {\nuint x;\nfunction f() view {\nx++;\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "view function 'f' cannot write to state variable 'x'"
    );
}

#[test]
fn mutability_delete_write_in_view_errors() {
    let text = "contract C {\nuint x;\nfunction f() view {\ndelete x;\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "view function 'f' cannot write to state variable 'x'"
    );
}

#[test]
fn mutability_constant_write_errors_like_view() {
    let text = "contract C {\nuint x;\nfunction f() constant {\nx = 1;\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "view function 'f' cannot write to state variable 'x'"
    );
}

#[test]
fn mutability_assembly_access_stays_silent() {
    let text = "contract C {\nuint x;\nfunction f() pure {\nassembly {\nlet y := x\n}\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn payable_non_payable_value_errors() {
    let text = "contract C {\nfunction f() public {}\nfunction g() public {\nf{value: 1}();\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "non-payable function 'f' cannot receive 'value'"
    );
}

#[test]
fn payable_payable_value_stays_silent() {
    let text = "contract C {\nfunction f() payable {}\nfunction g() public {\nf{value: 1}();\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn payable_gas_only_stays_silent() {
    let text = "contract C {\nfunction f() public {}\nfunction g() public {\nf{gas: 5000}();\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn payable_no_options_stays_silent() {
    let text = "contract C {\nfunction f() public {}\nfunction g() public {\nf();\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn payable_unknown_target_stays_silent_for_payable() {
    let text = "contract C {\nfunction g() public {\nmissing{value: 1}();\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(found[0].message, "unresolved identifier 'missing'");
}

#[test]
fn payable_member_non_payable_errors() {
    let text = "contract H {\nfunction f() public {}\n}\ncontract C {\nfunction g() public {\nH h;\nh.f{value: 1}();\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "non-payable function 'f' cannot receive 'value'"
    );
}

#[test]
fn payable_member_payable_stays_silent() {
    let text = "contract H {\nfunction f() payable {}\n}\ncontract C {\nfunction g() public {\nH h;\nh.f{value: 1}();\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn payable_builtin_with_value_stays_silent() {
    let text = "contract T is Test {\nfunction g() public {\nvm.prank{value: 1}(1);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn builtin_arity_wrong_count_reports_exact_message() {
    for (name, arity) in [
        ("gasleft", 0),
        ("keccak256", 1),
        ("sha256", 1),
        ("ripemd160", 1),
        ("ecrecover", 4),
        ("addmod", 3),
        ("mulmod", 3),
        ("blockhash", 1),
        ("selfdestruct", 1),
        ("assert", 1),
    ] {
        let args = (0..arity + 1).map(|_| "1").collect::<Vec<_>>().join(", ");
        let text = format!("contract C {{\nfunction f() public {{\n{name}({args});\n}}\n}}");
        let found = diagnostics_of(&text);
        assert_eq!(found.len(), 1, "{name}: {found:?}");
        assert_eq!(
            found[0].message,
            format!(
                "this function '{name}' takes {arity} arguments but {} supplied",
                arity + 1
            )
        );
        let covered = &text[found[0].span.start as usize..found[0].span.end as usize];
        assert!(
            covered.starts_with(name) && covered.ends_with(')'),
            "span: {covered}"
        );
    }
}

#[test]
fn builtin_arity_correct_count_stays_silent() {
    for (name, arity) in [
        ("gasleft", 0),
        ("keccak256", 1),
        ("sha256", 1),
        ("ripemd160", 1),
        ("ecrecover", 4),
        ("addmod", 3),
        ("mulmod", 3),
        ("blockhash", 1),
        ("selfdestruct", 1),
    ] {
        let args = (0..arity).map(|_| "1").collect::<Vec<_>>().join(", ");
        let text = format!("contract C {{\nfunction f() public {{\n{name}({args});\n}}\n}}");
        assert!(diagnostics_of(&text).is_empty(), "{name}: ok");
    }
    let text = "contract C {\nfunction f() public {\nassert(true);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn builtin_arity_user_definition_wins() {
    let text = "contract C {\nfunction gasleft(uint a) public { a; }\nfunction f() public {\ngasleft(1);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn builtin_arity_variadic_stays_silent() {
    let text = "contract C {\nfunction f(bool ok) public {\nrequire(ok);\nrequire(ok, \"no\");\nrevert();\nrevert(\"no\");\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn options_unknown_key_errors() {
    let text = "contract C {\nfunction f() public {}\nfunction g() public {\nf{foo: 1}();\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(found[0].message, "unknown call option 'foo'");
    let covered = &text[found[0].span.start as usize..found[0].span.end as usize];
    assert_eq!(covered, "foo");
}

#[test]
fn options_unknown_key_errors_for_member_and_new() {
    let member = "contract H {\nfunction f() public {}\n}\ncontract C {\nfunction g() public {\nH h;\nh.f{foo: 1}();\n}\n}";
    let found = diagnostics_of(member);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(found[0].message, "unknown call option 'foo'");
    let deploy =
        "contract C {\nconstructor(uint a) { a; }\nfunction f() public {\nnew C{foo: 1}(1);\n}\n}";
    let deploy_found = diagnostics_of(deploy);
    assert_eq!(deploy_found.len(), 1, "diags: {deploy_found:?}");
    assert_eq!(deploy_found[0].message, "unknown call option 'foo'");
}

#[test]
fn options_salt_only_allowed_for_new() {
    let text = "contract C {\nfunction f() public {}\nfunction g() public {\nf{salt: 1}();\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "call option 'salt' only allowed for 'new'"
    );
    let deploy =
        "contract C {\nconstructor(uint a) { a; }\nfunction f() public {\nnew C{salt: 1}(1);\n}\n}";
    assert!(diagnostics_of(deploy).is_empty());
}

#[test]
fn options_known_keys_and_nested_values_stay_silent() {
    let text = "contract C {\nfunction f() payable {}\nfunction h(uint a, uint b) public { a; b; }\nfunction g(uint x) public {\nf{value: 1}();\nf{gas: 5000}();\nf{value: x, gas: 5000}();\nf{value: h(1, 2)}();\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn abi_encode_with_selector_needs_at_least_one() {
    for name in ["encodeWithSelector", "encodeWithSignature"] {
        let text = format!("contract C {{\nfunction f() public {{\nabi.{name}();\n}}\n}}");
        let found = diagnostics_of(&text);
        assert_eq!(found.len(), 1, "{name}: {found:?}");
        assert_eq!(
            found[0].message,
            format!("this function '{name}' takes at least 1 arguments but 0 supplied")
        );
        let one = format!("contract C {{\nfunction f() public {{\nabi.{name}(1);\n}}\n}}");
        assert!(diagnostics_of(&one).is_empty(), "{name}: one");
        let two = format!("contract C {{\nfunction f() public {{\nabi.{name}(1, 2);\n}}\n}}");
        assert!(diagnostics_of(&two).is_empty(), "{name}: two");
    }
    let encode = "contract C {\nfunction f() public {\nabi.encode();\nabi.encodePacked();\n}\n}";
    assert!(diagnostics_of(encode).is_empty());
}

#[test]
fn type_mismatch_uint_string_errors() {
    let text = "contract C {\nuint x = \"hi\";\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "type mismatch: cannot assign string literal to 'uint'"
    );
}

#[test]
fn type_mismatch_bool_number_errors() {
    let text = "contract C {\nbool b = 1;\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "type mismatch: cannot assign number literal to 'bool'"
    );
}

#[test]
fn type_mismatch_string_number_errors() {
    let text = "contract C {\nstring s = 1;\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "type mismatch: cannot assign number literal to 'string'"
    );
}

#[test]
fn type_mismatch_correct_literals_stay_silent() {
    let text =
        "contract C {\nuint a = 1;\nbool b = true;\nstring s = \"hi\";\naddress w = 0xAbC123;\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn type_mismatch_ether_and_ident_stay_silent() {
    let text = "contract C {\nuint y;\nuint a = 1 ether;\nuint b = y;\nuint c = 1 + 2;\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn type_mismatch_local_var_errors() {
    let text = "contract C {\nfunction f() public {\nuint x = \"hi\";\nx;\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "type mismatch: cannot assign string literal to 'uint'"
    );
}

#[test]
fn type_mismatch_array_type_stays_silent() {
    let text = "contract C {\nuint[] a = 1;\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn using_exact_match_completes_library_function() {
    let text = "library SafeMath {\nfunction add(uint self, uint other) public returns (uint) { self; other; }\n}\ncontract C {\nusing SafeMath for uint;\nfunction f() public {\nuint x;\nx.\n}\n}";
    let at = text.rfind("x.").expect("dot") as u32 + 2;
    let labels = complete_labels(text, at);
    assert!(labels.contains(&"add".to_string()), "labels: {labels:?}");
}

#[test]
fn using_wrong_type_stays_silent() {
    let text = "library SafeMath {\nfunction add(uint self, uint other) public returns (uint) { self; other; }\n}\ncontract C {\nusing SafeMath for uint;\nfunction f() public {\nuint256 x;\nx.\n}\n}";
    let at = text.rfind("x.").expect("dot") as u32 + 2;
    let labels = complete_labels(text, at);
    assert!(!labels.contains(&"add".to_string()), "labels: {labels:?}");
}

#[test]
fn using_wildcard_matches_first_param_type() {
    let text = "library SafeMath {\nfunction add(uint self, uint other) public returns (uint) { self; other; }\n}\ncontract C {\nusing SafeMath for *;\nfunction f() public {\nuint x;\nx.\n}\n}";
    let at = text.rfind("x.").expect("dot") as u32 + 2;
    let labels = complete_labels(text, at);
    assert!(labels.contains(&"add".to_string()), "labels: {labels:?}");
}

#[test]
fn using_own_member_wins_over_library() {
    let text = "library L {\nfunction foo(C self) public {}\n}\ncontract C {\nusing L for C;\nfunction foo() public {}\nfunction f() public {\nthis.\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("this.").expect("dot") as u32 + 5;
    let items = complete_at(&file, text, at);
    let matches: Vec<&CompletionItem> = items.iter().filter(|item| item.label == "foo").collect();
    assert_eq!(matches.len(), 1);
    assert!(
        !matches[0].detail.contains("via using"),
        "detail: {}",
        matches[0].detail
    );
}

#[test]
fn using_hover_shows_library_origin() {
    let text = "library SafeMath {\nfunction add(uint self, uint other) public returns (uint) { self; other; }\n}\ncontract C {\nusing SafeMath for uint;\nfunction f() public {\nuint x;\nx.add;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("x.add").expect("use") as u32 + 2;
    let hover = hover_at(&file, text, at).expect("hover");
    assert!(
        hover.contents.contains("function add"),
        "contents: {}",
        hover.contents
    );
    assert!(
        hover.contents.contains("via using"),
        "contents: {}",
        hover.contents
    );
}

#[test]
fn using_goto_jumps_to_library_function() {
    let text = "library SafeMath {\nfunction add(uint self, uint other) public returns (uint) { self; other; }\n}\ncontract C {\nusing SafeMath for uint;\nfunction f() public {\nuint x;\nx.add;\n}\n}";
    let at_use = text.rfind("x.add").expect("use") as u32 + 2;
    let at_decl = text.find("function add").expect("decl") as u32 + 9;
    let target = goto_target(text, at_use);
    assert!(target.start <= at_decl && at_decl < target.end);
}

#[test]
fn using_correct_arity_stays_silent() {
    let text = "library SafeMath {\nfunction add(uint self, uint other) public returns (uint) { self; other; }\n}\ncontract C {\nusing SafeMath for uint;\nfunction f() public {\nuint x;\nx.add(1);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn using_wrong_arity_reports_expected_minus_one() {
    let text = "library SafeMath {\nfunction add(uint self, uint other) public returns (uint) { self; other; }\n}\ncontract C {\nusing SafeMath for uint;\nfunction f() public {\nuint x;\nx.add(1, 2);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this function 'add' takes 1 arguments but 2 supplied"
    );
}

#[test]
fn using_unknown_library_stays_silent() {
    let text = "contract C {\nusing Missing for uint;\nfunction f() public {\nuint x;\nx;\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn override_missing_override_errors_o1() {
    let text = "contract B {\nfunction f() public virtual {}\n}\ncontract D is B {\nfunction f() public {}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "function 'f' overrides virtual base but missing 'override'"
    );
}

#[test]
fn override_without_base_errors_o2() {
    let text = "contract D {\nfunction f() public override {}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "function 'f' has 'override' but no base function"
    );
}

#[test]
fn override_non_virtual_base_errors_o3() {
    let text = "contract B {\nfunction f() public {}\n}\ncontract D is B {\nfunction f() public override {}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(found[0].message, "function 'f' overrides non-virtual base");
}

#[test]
fn override_correct_pair_stays_silent() {
    let text = "contract B {\nfunction f() public virtual {}\n}\ncontract D is B {\nfunction f() public override {}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn override_overload_with_different_params_stays_silent() {
    let text = "contract B {\nfunction f(uint a) public virtual { a; }\n}\ncontract D is B {\nfunction f(uint a, uint b) public { a; b; }\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn override_private_derived_stays_silent() {
    let text = "contract B {\nfunction f() public virtual {}\n}\ncontract D is B {\nfunction f() private {}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn override_constructor_stays_silent() {
    let text =
        "contract B {\nfunction f() public virtual {}\n}\ncontract D is B {\nconstructor() {}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn getter_scalar_completes_and_resolves() {
    let text = "contract C {\nuint public total;\nfunction f() public {\ntot\n}\n}";
    let workspace = open_workspace(&[("/repo/C.sol", text)]);
    let items = complete_multi(&workspace, "/repo/C.sol", text, "tot");
    let labels: Vec<&str> = items.iter().map(|item| item.0.as_str()).collect();
    assert!(labels.contains(&"total"), "labels: {labels:?}");
}

#[test]
fn getter_array_takes_index_param() {
    let text = "contract C {\nuint[] public scores;\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let scope = super::collect_symbols(&file);
    let found = scope.resolve("scores", 0).expect("found");
    assert_eq!(found.detail, "uint[] scores");
    let getters = super::getter::contract_getters(&file.contracts()[0]);
    assert_eq!(getters.len(), 1);
    assert_eq!(getters[0].detail, "function scores(uint256) returns (uint)");
}

#[test]
fn getter_single_mapping_takes_key_param() {
    let text = "contract C {\nmapping(address => uint) public balances;\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let getters = super::getter::contract_getters(&file.contracts()[0]);
    assert_eq!(getters.len(), 1);
    assert_eq!(
        getters[0].detail,
        "function balances(address) returns (uint)"
    );
}

#[test]
fn getter_nested_mapping_stays_silent() {
    let text = "contract C {\nmapping(address => mapping(uint => bool)) public nested;\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let getters = super::getter::contract_getters(&file.contracts()[0]);
    assert!(getters.is_empty());
}

#[test]
fn getter_explicit_function_wins() {
    let text = "contract C {\nuint public total;\nfunction total() public returns (uint) {}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let getters = super::getter::contract_getters(&file.contracts()[0]);
    assert!(getters.is_empty());
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn getter_member_completion_includes_synthetic() {
    let text = "contract C {\nuint public total;\nfunction f() public {\nthis.\n}\n}";
    let at = text.rfind("this.").expect("dot") as u32 + 5;
    let labels = complete_labels(text, at);
    assert!(labels.contains(&"total".to_string()), "labels: {labels:?}");
}

#[test]
fn getter_signature_shows_synthetic_params() {
    let text = "contract C {\nmapping(address => uint) public balances;\nfunction f() public {\nbalances(\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    let at = text.rfind("balances(").expect("call") as u32 + 9;
    let found = super::signature_at(&file, text, at, &[]).expect("signature");
    assert!(
        found
            .iter()
            .any(|item| item.label.contains("balances(address)"))
    );
}

#[test]
fn getter_correct_call_stays_silent() {
    let text = "contract C {\nuint public total;\nfunction f() public {\ntotal();\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn getter_wrong_call_reports_arity() {
    let text = "contract C {\nuint public total;\nfunction f() public {\ntotal(1);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this function 'total' takes 0 arguments but 1 supplied"
    );
}

#[test]
fn nested_block_shadowing_resolves_inner() {
    let text = "contract C {\nuint x;\nfunction f() public {\nuint x = 1;\nif (true) {\nuint x = 2;\nx;\n}\n}\n}";
    let (_, scope) = collect(text);
    let at_use = text.rfind("\nx;\n").expect("use") as u32 + 1;
    let found = scope.resolve("x", at_use).expect("found");
    assert_eq!(found.kind, SymbolKind::LocalVar);
    let at_decl = text.rfind("uint x = 2").expect("inner decl") as u32 + 5;
    assert!(found.span.contains(at_decl));
}

#[test]
fn nested_goto_prefers_inner_block() {
    let text =
        "contract C {\nuint x;\nfunction f() public {\nif (true) {\nuint x = 1;\nx;\n}\n}\n}";
    let at_use = text.rfind("\nx;\n").expect("use") as u32 + 1;
    let at_inner = text.find("x = 1").expect("inner") as u32;
    let target = goto_target(text, at_use);
    assert!(target.start <= at_inner && at_inner < target.end);
}

#[test]
fn for_header_local_visible_in_body() {
    let text = "contract C {\nfunction f() public {\nfor (uint i = 0; i < 10; i++) {\ni;\n}\n}\n}";
    let (_, scope) = collect(text);
    let at_use = text.rfind("\ni;\n").expect("use") as u32 + 1;
    let found = scope.resolve("i", at_use).expect("found");
    assert_eq!(found.kind, SymbolKind::LocalVar);
    assert_eq!(found.detail, "uint i = 0");
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn for_header_local_misses_after_loop() {
    let text = "contract C {\nfunction f() public {\nfor (uint i = 0; i < 10; i++) {\n}\ni;\n}\n}";
    let (_, scope) = collect(text);
    let at_use = text.rfind("\ni;\n").expect("use") as u32 + 1;
    assert!(scope.resolve("i", at_use).is_none());
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(found[0].message, "unresolved identifier 'i'");
}

#[test]
fn tuple_names_resolve_and_stay_silent() {
    let text =
        "contract C {\nfunction f(uint pair) public {\n(uint a, uint b) = pair;\na;\nb;\n}\n}";
    let (_, scope) = collect(text);
    let at_a = text.rfind("\na;\n").expect("use a") as u32 + 1;
    let found = scope.resolve("a", at_a).expect("found");
    assert_eq!(found.kind, SymbolKind::LocalVar);
    assert_eq!(found.detail, "uint a");
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn nested_call_arity_reports() {
    let text = "contract C {\nfunction greet(uint a, uint b) public { a; b; }\nfunction f() public {\nif (true) {\ngreet(1);\n}\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this function 'greet' takes 2 arguments but 1 supplied"
    );
}

#[test]
fn nested_member_arity_reports() {
    let text = "contract Hello {\nfunction greet(uint a, uint b) public { a; b; }\n}\ncontract C {\nfunction f() public {\nHello h;\nwhile (true) {\nh.greet(1);\n}\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this function 'greet' takes 2 arguments but 1 supplied"
    );
}

#[test]
fn nested_missing_identifier_reports() {
    let text = "contract C {\nfunction f() public {\nif (true) {\nmissing;\n}\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(found[0].message, "unresolved identifier 'missing'");
}

#[test]
fn nested_emit_arity_reports() {
    let text = "contract C {\nevent Transfer(address a, uint b);\nfunction f() public {\nif (true) {\nemit Transfer(1);\n}\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this event 'Transfer' takes 2 arguments but 1 supplied"
    );
}

#[test]
fn nested_type_mismatch_reports_once() {
    let text = "contract C {\nfunction f() public {\nif (true) {\nuint x = \"hi\";\nx;\n}\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "type mismatch: cannot assign string literal to 'uint'"
    );
}

#[test]
fn nested_type_mismatch_builtin_reports_once() {
    let text =
        "contract C {\nfunction f() public {\nif (true) {\nuint x = msg.sender;\nx;\n}\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "type mismatch: cannot assign address to 'uint'"
    );
}

#[test]
fn nested_payable_plain_reports_once() {
    let text = "contract C {\nfunction f() public {}\nfunction g() public {\nif (true) {\nf{value: 1}();\n}\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "non-payable function 'f' cannot receive 'value'"
    );
}

#[test]
fn nested_payable_member_reports_once() {
    let text = "contract H {\nfunction f() public {}\n}\ncontract C {\nfunction g() public {\nH h;\nwhile (true) {\nh.f{value: 1}();\n}\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "non-payable function 'f' cannot receive 'value'"
    );
}

#[test]
fn nested_forge_assert_reports_once() {
    let text = "contract T is Test {\nfunction f() public {\nif (true) {\nassertEq(1);\n}\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this function 'assertEq' takes 2 arguments but 1 supplied"
    );
}

#[test]
fn error_too_few_reports_exact_message() {
    let text = "contract C {\nerror Insufficient(uint a, uint b);\nfunction f() public {\nrevert Insufficient(1);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this error 'Insufficient' takes 2 arguments but 1 supplied"
    );
    let covered = &text[found[0].span.start as usize..found[0].span.end as usize];
    assert!(
        covered.starts_with("revert") && covered.ends_with(')'),
        "span: {covered}"
    );
}

#[test]
fn error_too_many_reports_exact_message() {
    let text = "contract C {\nerror Insufficient(uint a);\nfunction f() public {\nrevert Insufficient(1, 2);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this error 'Insufficient' takes 1 arguments but 2 supplied"
    );
}

#[test]
fn error_correct_count_stays_silent() {
    let text = "contract C {\nerror Insufficient(uint a, uint b);\nfunction f() public {\nrevert Insufficient(1, 2);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn error_inherited_arity_uses_base() {
    let text = "contract B {\nerror Insufficient(uint a, uint b);\n}\ncontract D is B {\nfunction f() public {\nrevert Insufficient(1);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this error 'Insufficient' takes 2 arguments but 1 supplied"
    );
}

#[test]
fn error_unknown_reports_name_span_once() {
    let text = "contract C {\nfunction f() public {\nrevert Missing(1);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(found[0].message, "unresolved error 'Missing'");
    let at = text.find("Missing(1)").expect("use") as u32;
    assert_eq!(found[0].span.start, at);
    assert_eq!(found[0].span.end, at + 7);
}

#[test]
fn error_bare_revert_stays_silent() {
    let bare = "contract C {\nfunction f() public {\nrevert;\n}\n}";
    assert!(diagnostics_of(bare).is_empty());
    let reason = "contract C {\nfunction f() public {\nrevert(\"msg\");\n}\n}";
    assert!(diagnostics_of(reason).is_empty());
}

#[test]
fn error_dep_stays_silent() {
    let b = "contract B {\nerror Insufficient(uint a);\n}";
    let a =
        "import \"./B.sol\";\ncontract A {\nfunction f() public {\nrevert Insufficient(1);\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    assert!(diagnostics_multi(&workspace, "/A.sol", a).is_empty());
}

#[test]
fn error_untracked_dep_still_errors() {
    let a = "import \"./Missing.sol\";\ncontract A {\nfunction f() public {\nrevert Insufficient(1);\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/Missing.sol")]);
    let found = diagnostics_multi(&workspace, "/A.sol", a);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(found[0].message, "unresolved error 'Insufficient'");
}

#[test]
fn error_hover_shows_detail() {
    let text = "contract C {\nerror Insufficient(uint a);\nfunction f() public {\nrevert Insufficient(1);\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("Insufficient(1)").expect("use") as u32;
    let hover = hover_at(&file, text, at).expect("hover");
    assert_eq!(hover.contents, "error Insufficient");
}

#[test]
fn error_goto_jumps_to_declaration() {
    let text = "contract C {\nerror Insufficient(uint a);\nfunction f() public {\nrevert Insufficient(1);\n}\n}";
    let at_use = text.rfind("Insufficient(1)").expect("use") as u32;
    let at_decl = text.find("Insufficient").expect("decl") as u32;
    let target = goto_target(text, at_use);
    assert!(target.start <= at_decl && at_decl < target.end);
}

#[test]
fn modifier_plain_completion_suggests_modifier() {
    let text = "contract C {\nmodifier onlyOwner() { _; }\nfunction f() public {\nonly\n}\n}";
    let at = text.rfind("only").expect("prefix") as u32 + 4;
    let labels = complete_labels(text, at);
    assert!(
        labels.contains(&"onlyOwner".to_string()),
        "labels: {labels:?}"
    );
}

#[test]
fn modifier_plain_hover_shows_detail() {
    let text = "contract C {\nmodifier onlyOwner() { _; }\nfunction f() public onlyOwner {}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("onlyOwner {}").expect("use") as u32;
    let hover = hover_at(&file, text, at).expect("hover");
    assert_eq!(hover.contents, "modifier onlyOwner");
}

#[test]
fn modifier_plain_goto_jumps_to_declaration() {
    let text = "contract C {\nmodifier onlyOwner() { _; }\nfunction f() public onlyOwner {}\n}";
    let at_use = text.rfind("onlyOwner {}").expect("use") as u32;
    let at_decl = text.find("onlyOwner").expect("decl") as u32;
    let target = goto_target(text, at_use);
    assert!(target.start <= at_decl && at_decl < target.end);
}

#[test]
fn error_plain_completion_suggests_error() {
    let text = "contract C {\nerror Insufficient(uint a);\nfunction f() public {\nInsuf\n}\n}";
    let at = text.rfind("Insuf").expect("prefix") as u32 + 5;
    let labels = complete_labels(text, at);
    assert!(
        labels.contains(&"Insufficient".to_string()),
        "labels: {labels:?}"
    );
}

#[test]
fn error_plain_hover_shows_detail() {
    let text =
        "contract C {\nerror Insufficient(uint a);\nfunction f() public {\nInsufficient;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("Insufficient;").expect("use") as u32;
    let hover = hover_at(&file, text, at).expect("hover");
    assert_eq!(hover.contents, "error Insufficient");
}

#[test]
fn error_plain_goto_jumps_to_declaration() {
    let text =
        "contract C {\nerror Insufficient(uint a);\nfunction f() public {\nInsufficient;\n}\n}";
    let at_use = text.rfind("Insufficient;").expect("use") as u32;
    let at_decl = text.find("Insufficient").expect("decl") as u32;
    let target = goto_target(text, at_use);
    assert!(target.start <= at_decl && at_decl < target.end);
}

#[test]
fn struct_too_few_reports_exact_message() {
    let text = "contract C {\nstruct Person {\nuint age;\nstring name;\n}\nfunction f() public {\nPerson(1);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this struct 'Person' takes 2 arguments but 1 supplied"
    );
    let covered = &text[found[0].span.start as usize..found[0].span.end as usize];
    assert!(
        covered.starts_with("Person") && covered.ends_with(')'),
        "span: {covered}"
    );
}

#[test]
fn struct_too_many_reports_exact_message() {
    let text =
        "struct Person {\nuint age;\n}\ncontract C {\nfunction f() public {\nPerson(1, 2);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this struct 'Person' takes 1 arguments but 2 supplied"
    );
}

#[test]
fn struct_correct_count_stays_silent() {
    let text =
        "contract C {\nstruct Person {\nuint age;\n}\nfunction f() public {\nPerson(1);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn struct_inherited_first_hit_wins() {
    let text = "contract B {\nstruct Person {\nuint age;\nuint height;\n}\n}\ncontract D is B {\nfunction f() public {\nPerson(1);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this struct 'Person' takes 2 arguments but 1 supplied"
    );
}

#[test]
fn struct_unknown_callee_reports_identifier_once() {
    let text = "contract C {\nfunction f() public {\nMissing(1, 2);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(found[0].message, "unresolved identifier 'Missing'");
}

#[test]
fn deploy_implicit_ctor_too_many_reports() {
    let text = "contract C {\nfunction f() public {\nnew C(1);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this constructor for 'C' takes 0 arguments but 1 supplied"
    );
    let covered = &text[found[0].span.start as usize..found[0].span.end as usize];
    assert!(
        covered.starts_with("new") && covered.ends_with(')'),
        "span: {covered}"
    );
}

#[test]
fn deploy_implicit_ctor_empty_stays_silent() {
    let text = "contract C {\nfunction f() public {\nnew C();\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn deploy_explicit_ctor_wrong_count_reports() {
    let text = "contract C {\nconstructor(uint a, uint b) { a; b; }\nfunction f() public {\nnew C(1);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this constructor for 'C' takes 2 arguments but 1 supplied"
    );
}

#[test]
fn deploy_explicit_ctor_correct_stays_silent() {
    let text = "contract C {\nconstructor(uint a) { a; }\nfunction f() public {\nnew C(1);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn deploy_across_file_uses_target_ctor() {
    let dep = "contract Vault {\nconstructor(uint a, uint b) { a; b; }\n}";
    let cur = "import \"./V.sol\";\ncontract C {\nfunction f() public {\nnew Vault(1);\n}\n}";
    let mut workspace = open_workspace(&[("/V.sol", dep), ("/C.sol", cur)]);
    workspace.set_file_deps(PathBuf::from("/C.sol"), vec![PathBuf::from("/V.sol")]);
    let found = diagnostics_multi(&workspace, "/C.sol", cur);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this constructor for 'Vault' takes 2 arguments but 1 supplied"
    );
}

#[test]
fn deploy_across_implicit_ctor_reports() {
    let dep = "contract Vault {\n}";
    let cur = "import \"./V.sol\";\ncontract C {\nfunction f() public {\nnew Vault(1);\n}\n}";
    let mut workspace = open_workspace(&[("/V.sol", dep), ("/C.sol", cur)]);
    workspace.set_file_deps(PathBuf::from("/C.sol"), vec![PathBuf::from("/V.sol")]);
    let found = diagnostics_multi(&workspace, "/C.sol", cur);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this constructor for 'Vault' takes 0 arguments but 1 supplied"
    );
}

#[test]
fn deploy_unknown_target_stays_silent_without_double() {
    let text = "contract C {\nfunction f() public {\nnew Missing(1);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn deploy_option_and_named_args_stay_silent() {
    let option = "contract C {\nconstructor(uint a) { a; }\nfunction f() public {\nnew C{value: 1}(1);\n}\n}";
    assert!(diagnostics_of(option).is_empty());
    let named =
        "contract C {\nconstructor(uint a) { a; }\nfunction f() public {\nnew C({a: 1});\n}\n}";
    assert!(diagnostics_of(named).is_empty());
}

#[test]
fn shadow_param_shadows_state_reports() {
    let text = "contract C {\nuint x;\nfunction f(uint x) public {\nx;\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(found[0].message, "parameter 'x' shadows outer declaration");
}

#[test]
fn shadow_local_shadows_state_reports() {
    let text = "contract C {\nuint total;\nfunction f() public {\nuint total = 1;\ntotal;\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "variable 'total' shadows outer declaration"
    );
}

#[test]
fn shadow_nested_local_shadows_outer_reports() {
    let text = "contract C {\nfunction f() public {\nuint x = 1;\nx;\n{\nuint x = 2;\nx;\n}\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(found[0].message, "variable 'x' shadows outer declaration");
}

#[test]
fn shadow_sibling_blocks_stay_silent() {
    let text =
        "contract C {\nfunction f() public {\n{\nuint x = 1;\nx;\n}\n{\nuint x = 2;\nx;\n}\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn shadow_function_name_stays_silent() {
    let text = "contract C {\nfunction foo() public {\n}\nfunction f() public {\nuint foo = 1;\nfoo;\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn shadow_inherited_state_stays_silent() {
    let text =
        "contract B {\nuint x;\n}\ncontract D is B {\nfunction f() public {\nuint x = 1;\nx;\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn shadow_builtin_name_stays_silent() {
    let text = "contract C {\nfunction f() public {\nuint msg = 1;\nmsg;\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn unused_param_reports() {
    let text = "contract C {\nfunction f(uint a) public {\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(found[0].message, "unused parameter 'a'");
}

#[test]
fn unused_local_reports() {
    let text = "contract C {\nfunction f() public {\nuint x = 1;\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(found[0].message, "unused variable 'x'");
}

#[test]
fn unused_used_names_stay_silent() {
    let text = "contract C {\nfunction f(uint a) public {\nuint x = 1;\na;\nx;\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn unused_named_import_reports_once() {
    let text = "import {B} from \"./B.sol\";\ncontract A {\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(found[0].message, "unused import './B.sol'");
}

#[test]
fn unused_plain_import_stays_silent() {
    let text = "import \"./A.sol\";\ncontract A {\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn data_location_state_with_memory_errors() {
    let text = "contract C {\nstring memory s;\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "data location 'memory' not allowed for state variable"
    );
}

#[test]
fn data_location_value_with_location_errors() {
    let text = "contract C {\nfunction f(uint memory a) public {\na;\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "data location 'memory' not allowed for 'uint'"
    );
}

#[test]
fn data_location_missing_for_string_param_errors() {
    let text = "contract C {\nfunction f(string a) public {\na;\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "data location must be specified for 'string'"
    );
}

#[test]
fn data_location_correct_param_stays_silent() {
    let text = "contract C {\nfunction f(string memory a) public {\na;\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn data_location_calldata_local_errors() {
    let text = "contract C {\nfunction f() public {\nstring calldata s;\ns;\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "data location 'calldata' not allowed for local variable"
    );
}

#[test]
fn data_location_user_type_stays_silent() {
    let text = "struct S {\nuint a;\n}\ncontract C {\nfunction f(S s) public {\ns;\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn super_completion_lists_base_members_only() {
    let text = "contract B {\nuint total;\nfunction settle() public {}\n}\ncontract D is B {\nfunction own() public {}\nfunction f() public {\nsuper.\n}\n}";
    let at = text.rfind("super.").expect("dot") as u32 + 6;
    let labels = complete_labels(text, at);
    assert!(labels.contains(&"total".to_string()), "labels: {labels:?}");
    assert!(labels.contains(&"settle".to_string()), "labels: {labels:?}");
    assert!(!labels.contains(&"own".to_string()), "labels: {labels:?}");
    assert!(!labels.contains(&"f".to_string()), "labels: {labels:?}");
}

#[test]
fn super_hover_shows_base_origin() {
    let text = "contract B {\nfunction settle() public {}\n}\ncontract D is B {\nfunction f() public {\nsuper.settle;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("settle;").expect("use") as u32;
    let hover = hover_at(&file, text, at).expect("hover");
    assert!(
        hover.contents.contains("function settle"),
        "contents: {}",
        hover.contents
    );
    assert!(
        hover.contents.contains("from B"),
        "contents: {}",
        hover.contents
    );
}

#[test]
fn super_goto_jumps_to_base_declaration() {
    let text = "contract B {\nfunction settle() public {}\n}\ncontract D is B {\nfunction f() public {\nsuper.settle;\n}\n}";
    let at_use = text.rfind("settle;").expect("use") as u32;
    let at_decl = text.find("settle").expect("decl") as u32;
    let target = goto_target(text, at_use);
    assert!(target.start <= at_decl && at_decl < target.end);
}

#[test]
fn super_derived_member_stays_unresolved() {
    let text = "contract B {\nfunction settle() public {}\n}\ncontract D is B {\nfunction own() public {}\nfunction f() public {\nsuper.own;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("own;").expect("use") as u32;
    assert!(hover_at(&file, text, at).is_none());
    assert!(goto_definition(&file, text, at).is_none());
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn super_arity_uses_base_signature() {
    let text = "contract B {\nfunction greet(uint a, uint b) public { a; b; }\n}\ncontract D is B {\nfunction f() public {\nsuper.greet(1);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this function 'greet' takes 2 arguments but 1 supplied"
    );
}

#[test]
fn super_arity_shadow_uses_base_not_derived() {
    let text = "contract B {\nfunction greet(uint a) public { a; }\n}\ncontract D is B {\nfunction greet(uint a, uint b) public { a; b; }\nfunction f() public {\nsuper.greet(1, 2);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this function 'greet' takes 1 arguments but 2 supplied"
    );
}

#[test]
fn super_payable_base_non_payable_errors() {
    let text = "contract B {\nfunction f() public {}\n}\ncontract D is B {\nfunction g() public {\nsuper.f{value: 1}();\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "non-payable function 'f' cannot receive 'value'"
    );
}

#[test]
fn super_payable_base_payable_stays_silent() {
    let text = "contract B {\nfunction f() payable {}\n}\ncontract D is B {\nfunction g() public {\nsuper.f{value: 1}();\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn super_signature_shows_base_params() {
    let text = "contract B {\nfunction greet(uint a) public { a; }\n}\ncontract D is B {\nfunction f() public {\nsuper.greet(\n}\n}";
    let workspace = open_workspace(&[("/repo/C.sol", text)]);
    let found = signature_multi(&workspace, "/repo/C.sol", text, "super.greet(").expect("help");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].label, "function greet(uint a)");
}

#[test]
fn super_dep_completion_and_goto() {
    let dep = "contract B {\nfunction settle() public {}\n}";
    let cur = "import \"./B.sol\";\ncontract D is B {\nfunction f() public {\nsuper.settle;\nsuper.\n}\n}";
    let mut workspace = open_workspace(&[("/B.sol", dep), ("/D.sol", cur)]);
    workspace.set_file_deps(PathBuf::from("/D.sol"), vec![PathBuf::from("/B.sol")]);
    let found = member_complete_multi(&workspace, "/D.sol", cur, "super.");
    let hit = found
        .iter()
        .find(|(label, _)| label == "settle")
        .expect("settle hit");
    assert!(hit.1.contains("from B in B.sol"), "detail: {}", hit.1);
    let goto_hit = goto_multi(&workspace, "/D.sol", cur, "settle").expect("goto");
    assert_eq!(goto_hit.path, PathBuf::from("/B.sol"));
    let want = dep.find("settle").expect("decl") as u32;
    assert!(goto_hit.span.contains(want));
}

#[test]
fn super_dep_arity_and_payable_follow_base() {
    let dep = "contract B {\nfunction greet(uint a, uint b) public { a; b; }\nfunction fund() public {}\n}";
    let cur = "import \"./B.sol\";\ncontract D is B {\nfunction f() public {\nsuper.greet(1);\nsuper.fund{value: 1}();\n}\n}";
    let mut workspace = open_workspace(&[("/B.sol", dep), ("/D.sol", cur)]);
    workspace.set_file_deps(PathBuf::from("/D.sol"), vec![PathBuf::from("/B.sol")]);
    let found = diagnostics_multi(&workspace, "/D.sol", cur);
    assert_eq!(found.len(), 2, "diags: {found:?}");
    assert!(
        found
            .iter()
            .any(|item| item.message == "this function 'greet' takes 2 arguments but 1 supplied"),
        "diags: {found:?}"
    );
    assert!(
        found
            .iter()
            .any(|item| item.message == "non-payable function 'fund' cannot receive 'value'"),
        "diags: {found:?}"
    );
}

#[test]
fn enum_variant_completion_lists_variants() {
    let text =
        "enum Status {\nActive,\nClosed\n}\ncontract C {\nfunction f() public {\nStatus.\n}\n}";
    let at = text.rfind("Status.").expect("dot") as u32 + 7;
    let labels = complete_labels(text, at);
    assert!(labels.contains(&"Active".to_string()), "labels: {labels:?}");
    assert!(labels.contains(&"Closed".to_string()), "labels: {labels:?}");
}

#[test]
fn enum_variant_hover_and_goto() {
    let text = "enum Status {\nActive,\nClosed\n}\ncontract C {\nfunction f() public {\nStatus.Active;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at_use = text.rfind("Active;").expect("use") as u32;
    let hover = hover_at(&file, text, at_use).expect("hover");
    assert_eq!(hover.contents, "Status.Active");
    let at_decl = text.find("Active").expect("decl") as u32;
    let target = goto_target(text, at_use);
    assert!(target.start <= at_decl && at_decl < target.end);
}

#[test]
fn enum_unknown_variant_stays_silent() {
    let text =
        "enum Status {\nActive\n}\ncontract C {\nfunction f() public {\nStatus.Missing;\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn enum_dep_completion_and_goto() {
    let dep = "enum Status {\nActive,\nClosed\n}";
    let cur =
        "import \"./S.sol\";\ncontract C {\nfunction f() public {\nStatus.Active;\nStatus.\n}\n}";
    let mut workspace = open_workspace(&[("/S.sol", dep), ("/C.sol", cur)]);
    workspace.set_file_deps(PathBuf::from("/C.sol"), vec![PathBuf::from("/S.sol")]);
    let found = member_complete_multi(&workspace, "/C.sol", cur, "Status.");
    let hit = found
        .iter()
        .find(|(label, _)| label == "Closed")
        .expect("variant hit");
    assert!(hit.1.contains("from S.sol"), "detail: {}", hit.1);
    let goto_hit = goto_multi(&workspace, "/C.sol", cur, "Active").expect("goto");
    assert_eq!(goto_hit.path, PathBuf::from("/S.sol"));
    let want = dep.find("Active").expect("decl") as u32;
    assert!(goto_hit.span.contains(want));
}

#[test]
fn enum_dep_hover_includes_origin() {
    let dep = "enum Status {\nActive,\nClosed\n}";
    let cur = "import \"./S.sol\";\ncontract C {\nfunction f() public {\nStatus.Active;\n}\n}";
    let mut workspace = open_workspace(&[("/S.sol", dep), ("/C.sol", cur)]);
    workspace.set_file_deps(PathBuf::from("/C.sol"), vec![PathBuf::from("/S.sol")]);
    let hit = hover_multi(&workspace, "/C.sol", cur, "Active").expect("hover");
    assert_eq!(hit.contents, "Status.Active - from S.sol");
}

#[test]
fn dep_modifier_completion_includes_origin() {
    let b = "contract B {\nmodifier onlyOwner() { _; }\n}";
    let a = "import \"./B.sol\";\ncontract A {\nfunction f() public {\nonly\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    let found = complete_multi(&workspace, "/A.sol", a, "only");
    let hit = found
        .iter()
        .find(|(label, _)| label == "onlyOwner")
        .expect("dep modifier hit");
    assert!(hit.1.contains("modifier onlyOwner"), "detail: {}", hit.1);
    assert!(hit.1.contains("from B.sol"), "detail: {}", hit.1);
}

#[test]
fn dep_error_completion_includes_origin() {
    let b = "contract B {\nerror Insufficient(uint a);\n}";
    let a = "import \"./B.sol\";\ncontract A {\nfunction f() public {\nInsuf\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    let found = complete_multi(&workspace, "/A.sol", a, "Insuf");
    let hit = found
        .iter()
        .find(|(label, _)| label == "Insufficient")
        .expect("dep error hit");
    assert!(hit.1.contains("error Insufficient"), "detail: {}", hit.1);
    assert!(hit.1.contains("from B.sol"), "detail: {}", hit.1);
}

#[test]
fn dep_modifier_hover_includes_origin() {
    let b = "contract B {\nmodifier onlyOwner() { _; }\n}";
    let a = "import \"./B.sol\";\ncontract A {\nfunction f() public {\nonlyOwner;\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    let hit = hover_multi(&workspace, "/A.sol", a, "onlyOwner;").expect("dep hover");
    assert!(
        hit.contents.contains("modifier onlyOwner"),
        "contents: {}",
        hit.contents
    );
    assert!(
        hit.contents.contains("from B.sol"),
        "contents: {}",
        hit.contents
    );
}

#[test]
fn dep_error_hover_includes_origin() {
    let b = "contract B {\nerror Insufficient(uint a);\n}";
    let a = "import \"./B.sol\";\ncontract A {\nfunction f() public {\nInsufficient;\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    let hit = hover_multi(&workspace, "/A.sol", a, "Insufficient;").expect("dep hover");
    assert!(
        hit.contents.contains("error Insufficient"),
        "contents: {}",
        hit.contents
    );
    assert!(
        hit.contents.contains("from B.sol"),
        "contents: {}",
        hit.contents
    );
}

#[test]
fn dep_modifier_goto_jumps_to_other_file() {
    let b = "contract B {\nmodifier onlyOwner() { _; }\n}";
    let a = "import \"./B.sol\";\ncontract A {\nfunction f() public {\nonlyOwner;\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    let hit = goto_multi(&workspace, "/A.sol", a, "onlyOwner;").expect("dep goto");
    assert_eq!(hit.path, PathBuf::from("/B.sol"));
    let want = b.find("onlyOwner").expect("decl") as u32;
    assert!(hit.span.contains(want));
}

#[test]
fn dep_error_goto_jumps_to_other_file() {
    let b = "contract B {\nerror Insufficient(uint a);\n}";
    let a = "import \"./B.sol\";\ncontract A {\nfunction f() public {\nInsufficient;\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    let hit = goto_multi(&workspace, "/A.sol", a, "Insufficient;").expect("dep goto");
    assert_eq!(hit.path, PathBuf::from("/B.sol"));
    let want = b.find("Insufficient").expect("decl") as u32;
    assert!(hit.span.contains(want));
}

#[test]
fn dep_struct_field_completion_includes_origin() {
    let b = "struct Person {\nuint age;\nstring name;\n}";
    let a = "import \"./B.sol\";\ncontract A {\nfunction f() public {\nPerson p;\np.\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    let found = member_complete_multi(&workspace, "/A.sol", a, "p.");
    let hit = found
        .iter()
        .find(|(label, _)| label == "age")
        .expect("dep field hit");
    assert!(hit.1.contains("uint age"), "detail: {}", hit.1);
    assert!(hit.1.contains("from Person in B.sol"), "detail: {}", hit.1);
}

#[test]
fn dep_struct_field_hover_includes_origin() {
    let b = "struct Person {\nuint age;\n}";
    let a = "import \"./B.sol\";\ncontract A {\nfunction f() public {\nPerson p;\np.age;\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    let hit = hover_multi(&workspace, "/A.sol", a, "age").expect("dep hover");
    assert!(
        hit.contents.contains("uint age"),
        "contents: {}",
        hit.contents
    );
    assert!(
        hit.contents.contains("from Person in B.sol"),
        "contents: {}",
        hit.contents
    );
}

#[test]
fn dep_struct_field_goto_jumps_to_other_file() {
    let b = "struct Person {\nuint age;\n}";
    let a = "import \"./B.sol\";\ncontract A {\nfunction f() public {\nPerson p;\np.age;\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", a), ("/B.sol", b)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    let hit = goto_multi(&workspace, "/A.sol", a, "age").expect("dep goto");
    assert_eq!(hit.path, PathBuf::from("/B.sol"));
    let want = b.find("age").expect("decl") as u32;
    assert!(hit.span.contains(want));
}

#[test]
fn user_type_wrap_unwrap_completion() {
    let text = "type MyNumber is uint256;\ncontract C {\nfunction f() public {\nMyNumber.\n}\n}";
    let at = text.rfind("MyNumber.").expect("dot") as u32 + 9;
    let labels = complete_labels(text, at);
    assert!(labels.contains(&"wrap".to_string()), "labels: {labels:?}");
    assert!(labels.contains(&"unwrap".to_string()), "labels: {labels:?}");
}

#[test]
fn user_type_wrap_hover_shows_detail() {
    let text =
        "type MyNumber is uint256;\ncontract C {\nfunction f() public {\nMyNumber.wrap;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("wrap;").expect("use") as u32;
    let hover = hover_at(&file, text, at).expect("hover");
    assert_eq!(
        hover.contents,
        "function wrap(uint256 value) returns (MyNumber)"
    );
    assert!(hover.span.contains(at));
}

#[test]
fn user_type_wrap_goto_returns_none() {
    let text =
        "type MyNumber is uint256;\ncontract C {\nfunction f() public {\nMyNumber.wrap;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("wrap;").expect("use") as u32;
    assert!(goto_definition(&file, text, at).is_none());
    let workspace = open_workspace(&[("/A.sol", text)]);
    let buf = PathBuf::from("/A.sol");
    let ctx = ProjectContext::new(buf.as_path(), text, at, &workspace);
    assert!(SolidityAnalyzer.goto_location(&ctx).is_none());
}

#[test]
fn user_type_wrap_arity_checks_one_arg() {
    let text = "type MyNumber is uint256;\ncontract C {\nfunction f() public {\nMyNumber.wrap(1, 2);\nMyNumber.unwrap(1);\nMyNumber.bad(1);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this function 'wrap' takes 1 arguments but 2 supplied"
    );
}

#[test]
fn user_type_signature_shows_wrap() {
    let text =
        "type MyNumber is uint256;\ncontract C {\nfunction f() public {\nMyNumber.wrap(\n}\n}";
    let workspace = open_workspace(&[("/repo/C.sol", text)]);
    let found = signature_multi(&workspace, "/repo/C.sol", text, "MyNumber.wrap(").expect("help");
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].label,
        "function wrap(uint256 value) returns (MyNumber)"
    );
}

#[test]
fn user_type_dep_hover_uses_member_span() {
    let dep = "type MyNumber is uint256;";
    let cur = "import \"./T.sol\";\ncontract C {\nfunction f() public {\nMyNumber.wrap;\n}\n}";
    let mut workspace = open_workspace(&[("/T.sol", dep), ("/C.sol", cur)]);
    workspace.set_file_deps(PathBuf::from("/C.sol"), vec![PathBuf::from("/T.sol")]);
    let hit = hover_multi(&workspace, "/C.sol", cur, "wrap").expect("hover");
    assert!(
        hit.contents.contains("function wrap(uint256 value)"),
        "contents: {}",
        hit.contents
    );
    let at = cur.rfind("wrap").expect("use") as u32;
    assert!(hit.span.contains(at));
}

#[test]
fn inherited_error_resolves_in_derived_scope() {
    let text = "contract B {\nerror Insufficient(uint a);\n}\ncontract D is B {\nfunction f() public {\nInsufficient;\n}\n}";
    let (_, scope) = collect(text);
    let at = text.rfind("Insufficient;").expect("use") as u32;
    let found = scope.resolve("Insufficient", at).expect("found");
    assert_eq!(found.kind, SymbolKind::Error);
    assert!(found.detail.contains("from B"), "detail: {}", found.detail);
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn inherited_modifier_resolves_in_derived_scope() {
    let text = "contract B {\nmodifier onlyOwner() {}\n}\ncontract D is B {\nfunction f() public onlyOwner {}\n}";
    let (_, scope) = collect(text);
    let at = text.rfind("onlyOwner {}").expect("use") as u32;
    let found = scope.resolve("onlyOwner", at).expect("found");
    assert_eq!(found.kind, SymbolKind::Modifier);
    assert!(found.detail.contains("from B"), "detail: {}", found.detail);
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn inherited_user_type_resolves_in_derived_scope() {
    let text = "contract B {\ntype MyNumber is uint256;\n}\ncontract D is B {\nfunction f() public {\nMyNumber m;\nm;\n}\n}";
    let (_, scope) = collect(text);
    let at = text.rfind("MyNumber m;").expect("decl") as u32;
    let found = scope.resolve("MyNumber", at).expect("found");
    assert_eq!(found.kind, SymbolKind::UserType);
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn inherited_members_stay_vars_and_funcs() {
    let text = "contract B {\nerror Insufficient(uint a);\ntype MyNumber is uint256;\nevent Transfer(address a);\nuint total;\nfunction settle() public {}\n}\ncontract D is B {\nfunction f() public {\nthis.\n}\n}";
    let at = text.rfind("this.").expect("dot") as u32 + 5;
    let labels = complete_labels(text, at);
    assert!(labels.contains(&"total".to_string()), "labels: {labels:?}");
    assert!(labels.contains(&"settle".to_string()), "labels: {labels:?}");
    assert!(
        !labels.contains(&"Insufficient".to_string()),
        "labels: {labels:?}"
    );
    assert!(
        !labels.contains(&"MyNumber".to_string()),
        "labels: {labels:?}"
    );
    assert!(
        !labels.contains(&"Transfer".to_string()),
        "labels: {labels:?}"
    );
}

#[test]
fn error_arity_follows_dep_heritage() {
    let base = "contract Base {\nerror Insufficient(uint a, uint b);\n}";
    let mid = "import \"./Base.sol\";\ncontract Mid is Base {\n}";
    let cur = "import \"./Mid.sol\";\ncontract D is Mid {\nfunction f() public {\nrevert Insufficient(1);\n}\n}";
    let mut workspace = open_workspace(&[("/Base.sol", base), ("/Mid.sol", mid), ("/D.sol", cur)]);
    workspace.set_file_deps(PathBuf::from("/Mid.sol"), vec![PathBuf::from("/Base.sol")]);
    workspace.set_file_deps(PathBuf::from("/D.sol"), vec![PathBuf::from("/Mid.sol")]);
    let found = diagnostics_multi(&workspace, "/D.sol", cur);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this error 'Insufficient' takes 2 arguments but 1 supplied"
    );
}

#[test]
fn error_arity_uses_dep_contract_fallback() {
    let dep = "contract B {\nerror Insufficient(uint a);\n}";
    let cur = "import \"./B.sol\";\ncontract A {\nfunction f() public {\nrevert Insufficient(1, 2);\n}\n}";
    let mut workspace = open_workspace(&[("/B.sol", dep), ("/A.sol", cur)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    let found = diagnostics_multi(&workspace, "/A.sol", cur);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this error 'Insufficient' takes 1 arguments but 2 supplied"
    );
}

#[test]
fn hover_shows_state_var_value_for_single_literal() {
    let text = "contract C {\nuint256 total = 42;\nstring name = \"hi\";\nbool on = true;\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    for (needle, want) in [
        (" total", "uint256 total = 42"),
        (" name", "string name = \"hi\""),
        (" on", "bool on = true"),
    ] {
        let at = text.find(needle).expect("decl") as u32 + 1;
        let hover = hover_at(&file, text, at).expect("hover");
        assert_eq!(hover.contents, want);
    }
}

#[test]
fn hover_hides_value_for_complex_initializer() {
    let text = "contract C {\nuint256 fee = 1 ether;\nuint256 calc = 1 + 2;\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at_fee = text.find("fee").expect("decl") as u32;
    let fee = hover_at(&file, text, at_fee).expect("hover");
    assert_eq!(fee.contents, "uint256 fee");
    let at_calc = text.find("calc").expect("decl") as u32;
    let calc = hover_at(&file, text, at_calc).expect("hover");
    assert_eq!(calc.contents, "uint256 calc");
}

#[test]
fn hover_shows_line_natspec_for_function() {
    let text = "contract C {\n/// @notice Adds one.\n/// @param x input.\nfunction add(uint x) public returns (uint) {\nx;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("add").expect("use") as u32;
    let hover = hover_at(&file, text, at).expect("hover");
    assert!(
        hover
            .contents
            .starts_with("function add(uint x) returns (uint)"),
        "contents: {}",
        hover.contents
    );
    assert!(
        hover
            .contents
            .contains("@notice Adds one.\n@param x input."),
        "contents: {}",
        hover.contents
    );
}

#[test]
fn hover_shows_block_natspec_for_state_var() {
    let text =
        "contract C {\n/**\n* @notice Total supply.\n* @dev Capped at 100.\n*/\nuint256 total;\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.find("total").expect("decl") as u32;
    let hover = hover_at(&file, text, at).expect("hover");
    assert_eq!(
        hover.contents,
        "uint256 total\n\n@notice Total supply.\n@dev Capped at 100."
    );
}

#[test]
fn hover_ignores_plain_comment_and_wide_gap() {
    let text = "contract C {\n// plain note\nuint256 a;\n/// @notice Stale.\n\n\n\nuint256 b;\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at_a = text.find("uint256 a").expect("decl") as u32 + 8;
    let hover_a = hover_at(&file, text, at_a).expect("hover");
    assert_eq!(hover_a.contents, "uint256 a");
    let at_b = text.rfind("uint256 b").expect("decl") as u32 + 8;
    let hover_b = hover_at(&file, text, at_b).expect("hover");
    assert_eq!(hover_b.contents, "uint256 b");
}

#[test]
fn hover_skips_natspec_for_params() {
    let text = "contract C {\n/// @notice Stray.\nfunction f(uint amount) public {\namount;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("amount;").expect("use") as u32;
    let hover = hover_at(&file, text, at).expect("hover");
    assert_eq!(hover.contents, "uint amount");
}

#[test]
fn dep_hover_places_natspec_before_origin() {
    let dep = "/// @notice Vault.\ncontract Vault {\n/// @notice Withdraw.\nfunction withdraw(uint a) public {}\n}";
    let cur = "import \"./B.sol\";\ncontract A {\nfunction f() public {\nwithdraw;\n}\n}";
    let mut workspace = open_workspace(&[("/A.sol", cur), ("/B.sol", dep)]);
    workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
    let hit = hover_multi(&workspace, "/A.sol", cur, "withdraw").expect("dep hover");
    assert!(
        hit.contents.starts_with("function withdraw(uint a)"),
        "contents: {}",
        hit.contents
    );
    assert!(
        hit.contents.contains("@notice Withdraw."),
        "contents: {}",
        hit.contents
    );
    let docs_at = hit.contents.find("@notice").expect("docs");
    let from_at = hit.contents.find("from B.sol").expect("origin");
    assert!(docs_at < from_at, "contents: {}", hit.contents);
}

#[test]
fn function_type_inner_names_stay_silent() {
    let text = "contract C {\nfunction (uint amount) external cb;\nuint good;\n}";
    let found = diagnostics_of(text);
    assert!(found.is_empty(), "diags: {found:?}");
}

#[test]
fn function_type_outer_uses_resolve() {
    let text = "contract C {\nfunction (uint) external cb;\nfunction f() public {\ncb;\n}\n}";
    let found = diagnostics_of(text);
    assert!(found.is_empty(), "diags: {found:?}");
}

#[test]
fn function_type_hover_shows_canon_text() {
    let text = "contract C {\nfunction (uint) external cb;\nfunction f() public {\ncb;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("\ncb;\n").expect("use") as u32 + 1;
    let hover = hover_at(&file, text, at).expect("hover");
    assert_eq!(hover.contents, "function (uint) external cb");
}

#[test]
fn function_type_external_member_resolves() {
    let text =
        "contract C {\nfunction (uint) external cb;\nfunction f() public {\ncb.selector;\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("cb.selector").expect("use") as u32 + 3;
    let hover = hover_at(&file, text, at).expect("hover");
    assert_eq!(hover.contents, "bytes4 selector - from solidity");
}

#[test]
fn function_type_internal_stays_unknown() {
    let text =
        "contract C {\nfunction (uint) internal cb;\nfunction f() public {\ncb.selector;\n}\n}";
    let found = diagnostics_of(text);
    assert!(found.is_empty(), "diags: {found:?}");
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let at = text.rfind("cb.selector").expect("use") as u32 + 3;
    assert!(hover_at(&file, text, at).is_none());
}

#[test]
fn type_mismatch_cast_stays_silent() {
    let text = "contract C {\nuint x = uint8(1);\naddress a = address(0);\n}";
    assert!(
        diagnostics_of(text).is_empty(),
        "diags: {:?}",
        diagnostics_of(text)
    );
}

#[test]
fn type_mismatch_builtin_match_stays_silent() {
    let text = "contract C {\naddress a = msg.sender;\nuint b = block.timestamp;\nbytes c = abi.encode(1);\naddress addr;\nuint d = addr.balance;\nuint[] arr;\nuint e = arr.length;\n}";
    assert!(
        diagnostics_of(text).is_empty(),
        "diags: {:?}",
        diagnostics_of(text)
    );
}

#[test]
fn type_mismatch_msg_sender_to_uint_errors() {
    let text = "contract C {\nuint x = msg.sender;\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "type mismatch: cannot assign address to 'uint'"
    );
}

#[test]
fn type_mismatch_block_to_address_errors() {
    let text = "contract C {\naddress a = block.timestamp;\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "type mismatch: cannot assign uint256 to 'address'"
    );
}

#[test]
fn type_mismatch_coinbase_to_address_stays_silent() {
    let text = "contract C {\naddress a = block.coinbase;\n}";
    assert!(
        diagnostics_of(text).is_empty(),
        "diags: {:?}",
        diagnostics_of(text)
    );
}

#[test]
fn type_mismatch_coinbase_to_uint_errors() {
    let text = "contract C {\nuint x = block.coinbase;\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "type mismatch: cannot assign address to 'uint'"
    );
}

#[test]
fn type_mismatch_encode_to_uint_errors() {
    let text = "contract C {\nuint x = abi.encode(1);\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "type mismatch: cannot assign bytes to 'uint'"
    );
}

#[test]
fn type_mismatch_balance_to_address_errors() {
    let text = "contract C {\naddress addr;\naddress a = addr.balance;\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "type mismatch: cannot assign uint256 to 'address'"
    );
}

#[test]
fn type_mismatch_length_to_address_errors() {
    let text = "contract C {\nuint[] arr;\naddress a = arr.length;\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "type mismatch: cannot assign uint256 to 'address'"
    );
}

#[test]
fn type_mismatch_binop_and_call_stay_silent() {
    let text = "contract C {\nfunction foo() internal returns (uint) {}\nuint a = 1 + 2;\nuint b = foo();\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn mutability_push_in_view_errors() {
    let text = "contract C {\nuint[] arr;\nfunction f() view {\narr.push(1);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "view function 'f' cannot write to state variable 'arr'"
    );
}

#[test]
fn mutability_pop_in_pure_errors() {
    let text = "contract C {\nuint[] arr;\nfunction f() pure {\narr.pop();\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "pure function 'f' cannot write to state variable 'arr'"
    );
}

#[test]
fn mutability_push_in_public_stays_silent() {
    let text = "contract C {\nuint[] arr;\nfunction f() public {\narr.push(1);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn mutability_struct_field_assign_in_view_errors() {
    let text = "struct S {\nuint x;\n}\ncontract C {\nS s;\nfunction f() view {\ns.x = 1;\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "view function 'f' cannot write to state variable 's'"
    );
}

#[test]
fn mutability_struct_field_read_in_view_stays_silent() {
    let text = "struct S {\nuint x;\n}\ncontract C {\nS s;\nfunction f() view {\ns.x;\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn mutability_struct_field_assign_in_public_stays_silent() {
    let text = "struct S {\nuint x;\n}\ncontract C {\nS s;\nfunction f() public {\ns.x = 1;\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn visibility_instance_internal_errors() {
    let text = "contract C {\nfunction foo() internal {}\n}\ncontract D {\nfunction f() public {\nC c;\nc.foo();\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "internal function 'foo' cannot be called via instance 'c'"
    );
}

#[test]
fn visibility_instance_private_errors() {
    let text = "contract C {\nfunction bar() private {}\n}\ncontract D {\nfunction f() public {\nC c;\nc.bar();\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "private function 'bar' cannot be called via instance 'c'"
    );
}

#[test]
fn visibility_instance_public_stays_silent() {
    let text = "contract C {\nfunction foo() public {}\n}\ncontract D {\nfunction f() public {\nC c;\nc.foo();\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn visibility_private_in_derived_errors() {
    let text = "contract B {\nfunction foo() private {}\n}\ncontract D is B {\nfunction f() public {\nfoo();\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "private function 'foo' cannot be called from derived contract 'D'"
    );
}

#[test]
fn visibility_private_own_call_stays_silent() {
    let text = "contract C {\nfunction foo() private {}\nfunction f() public {\nfoo();\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn visibility_internal_in_derived_stays_silent() {
    let text = "contract B {\nfunction foo() internal {}\n}\ncontract D is B {\nfunction f() public {\nfoo();\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn visibility_overload_mixed_stays_silent() {
    let text = "contract C {\nfunction foo() external {}\nfunction foo(uint a) public { a; }\nfunction f() public {\nfoo();\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn visibility_this_internal_stays_silent() {
    let text = "contract C {\nfunction foo() internal {}\nfunction f() public {\nthis.foo();\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn visibility_private_across_files_in_derived_errors() {
    let base = "contract B {\nfunction foo() private {}\n}";
    let cur = "import \"./B.sol\";\ncontract D is B {\nfunction f() public {\nfoo();\n}\n}";
    let mut workspace = open_workspace(&[("/B.sol", base), ("/D.sol", cur)]);
    workspace.set_file_deps(PathBuf::from("/D.sol"), vec![PathBuf::from("/B.sol")]);
    let found = diagnostics_multi(&workspace, "/D.sol", cur);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "private function 'foo' cannot be called from derived contract 'D'"
    );
}

#[test]
fn unknown_underscore_placeholder_stays_silent() {
    let text = "contract C {\nmodifier onlyOwner() {\n_;\n}\n}";
    assert!(
        diagnostics_of(text).is_empty(),
        "diags: {:?}",
        diagnostics_of(text)
    );
}

#[test]
fn unknown_bare_underscore_stays_silent() {
    let text = "contract C {\nfunction f() public {\n_;\n}\n}";
    assert!(
        diagnostics_of(text).is_empty(),
        "diags: {:?}",
        diagnostics_of(text)
    );
}

#[test]
fn interface_state_var_errors() {
    let text = "interface I {\nuint256 total;\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "interface 'I' cannot have state variable 'total'"
    );
}

#[test]
fn interface_constructor_errors() {
    let text = "interface I {\nconstructor() {}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(found[0].message, "interface 'I' cannot have constructor");
}

#[test]
fn interface_clean_stays_silent() {
    let text = "interface I {\nfunction total() external returns (uint256);\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn contract_state_var_and_constructor_stay_silent() {
    let text = "contract C {\nuint256 total;\nconstructor() {}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn library_state_var_stays_silent_deferred() {
    let text = "library L {\nuint256 total;\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn event_indexed_params_are_stored() {
    let text = "contract C {\nevent Transfer(address indexed from, uint256 value);\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let events = file.contracts()[0].events();
    assert_eq!(events.len(), 1);
    let params = events[0].params();
    assert_eq!(params.len(), 2);
    assert!(params[0].is_indexed());
    assert!(params[0].indexed_span().is_some());
    assert!(!params[1].is_indexed());
    assert_eq!(params[0].name().as_deref(), Some("from"));
}

#[test]
fn event_anonymous_flag_is_stored() {
    let text = "contract C {\nevent Transfer(address a) anonymous;\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let events = file.contracts()[0].events();
    assert_eq!(events.len(), 1);
    assert!(events[0].is_anonymous());
    assert!(events[0].anonymous_span().is_some());
}

#[test]
fn event_four_indexed_errors() {
    let text = "contract C {\nevent E(address indexed a, uint indexed b, uint indexed c, uint indexed d);\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "event 'E' has 4 indexed params but max is 3"
    );
}

#[test]
fn event_three_indexed_stays_silent() {
    let text = "contract C {\nevent E(address indexed a, uint indexed b, uint indexed c);\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn event_anonymous_four_indexed_stays_silent() {
    let text = "contract C {\nevent E(address indexed a, uint indexed b, uint indexed c, uint indexed d) anonymous;\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn event_no_indexed_stays_silent() {
    let text = "contract C {\nevent Transfer(address from, uint256 value);\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn yul_let_use_stays_silent() {
    let text = "contract C {\nfunction f() public {\nassembly {\nlet x := 1\nlet y := x\n}\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn yul_outer_use_stays_silent() {
    let text = "contract C {\nuint x;\nfunction f() public {\nassembly {\nlet y := x\n}\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn yul_builtin_stays_silent() {
    let text = "contract C {\nfunction f() public {\nassembly {\nlet y := add(1, 2)\nsstore(0, y)\n}\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn yul_unknown_opcode_stays_silent() {
    let text = "contract C {\nfunction f() public {\nassembly {\nlet x := bogus\n}\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn yul_completion_lists_decl_and_builtin() {
    let text = "contract C {\nfunction f() public {\nassembly {\nlet myval := 1\nmy\n}\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    let at = text.rfind("my\n").expect("prefix") as u32 + 2;
    let labels: Vec<String> = complete_at(&file, text, at)
        .into_iter()
        .map(|item| item.label)
        .collect();
    assert!(labels.contains(&"myval".to_string()), "labels: {labels:?}");
}

#[test]
fn yul_hover_shows_decl_detail() {
    let text =
        "contract C {\nfunction f() public {\nassembly {\nlet myval := 1\nlet y := myval\n}\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    let at = text.rfind("myval\n").expect("use") as u32 + 2;
    let found = hover_at(&file, text, at).expect("hover");
    assert!(
        found.contents.contains("myval"),
        "hover: {:?}",
        found.contents
    );
    assert!(
        found.contents.contains("yul"),
        "hover: {:?}",
        found.contents
    );
}

#[test]
fn yul_hover_builtin_shows_yul_origin() {
    let text = "contract C {\nfunction f() public {\nassembly {\nlet y := add(1, 2)\n}\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    let at = text.rfind("add").expect("builtin") as u32 + 1;
    let found = hover_at(&file, text, at).expect("hover");
    assert!(
        found.contents.contains("add"),
        "hover: {:?}",
        found.contents
    );
    assert!(
        found.contents.contains("yul"),
        "hover: {:?}",
        found.contents
    );
}

#[test]
fn yul_goto_decl_jumps_to_let() {
    let text =
        "contract C {\nfunction f() public {\nassembly {\nlet myval := 1\nlet y := myval\n}\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    let decl_at = text.find("myval :=").expect("decl") as u32 + 2;
    let use_at = text.rfind("myval\n").expect("use") as u32 + 2;
    let target = goto_definition(&file, text, use_at).expect("goto");
    assert!(target.contains(decl_at), "target: {target:?}");
}

#[test]
fn yul_goto_builtin_returns_none() {
    let text = "contract C {\nfunction f() public {\nassembly {\nlet y := add(1, 2)\n}\n}\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    let at = text.rfind("add").expect("builtin") as u32 + 1;
    assert!(goto_definition(&file, text, at).is_none());
}
