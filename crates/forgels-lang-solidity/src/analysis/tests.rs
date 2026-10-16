use super::{
    CompletionItem, CompletionKind, Scope, SolidityAnalyzer, SymbolKind, collect_symbols,
    complete_at, complete_bare_imports, complete_import_paths, completion_item_kind, exports_of,
    goto_definition, hover_at, import_inner_span, import_path_at, imports_in,
};
use crate::{SolidityLexer, SolidityParser};
use forgels_core::{Analyzer, ProjectContext, Workspace, routing::Language};
use forgels_syntax::{Lexer, Span};
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
fn exports_of_lists_contracts_and_functions_only() {
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
        (CompletionKind::Keyword, 14),
        (CompletionKind::Path, 17),
        (CompletionKind::Package, 19),
    ];
    for (kind, want) in cases {
        assert_eq!(completion_item_kind(kind).value(), want);
    }
    // Follow-up: wire EnumMember to 20 once SymbolKind gains it.
    assert_eq!(
        forgels_protocol::CompletionItemKind::ENUM_MEMBER.value(),
        20
    );
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
    let text = "contract C {\nfunction greet(uint a, uint b) public {}\nfunction f() public {\ngreet(1);\n}\n}";
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
    let text =
        "contract C {\nfunction greet(uint a) public {}\nfunction f() public {\ngreet(1, 2);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].message,
        "this function 'greet' takes 1 arguments but 2 supplied"
    );
}

#[test]
fn correct_arg_count_has_no_diagnostic() {
    let text = "contract C {\nfunction greet(uint a, uint b) public {}\nfunction f() public {\ngreet(1, 2);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn unknown_callee_stays_silent() {
    let text = "contract C {\nfunction f() public {\nmissing(1, 2, 3);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn member_call_stays_silent() {
    let text = "contract C {\nfunction greet(uint a, uint b) public {}\nfunction f() public {\nobj.greet(1);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn super_and_new_calls_stay_silent() {
    let text = "contract C {\nfunction greet(uint a, uint b) public {}\nfunction f() public {\nsuper.greet(1);\nnew C(1, 2, 3);\nsuper(1);\nconstructor(1);\n}\n}";
    assert!(diagnostics_of(text).is_empty());
}

#[test]
fn overload_uses_first_definition() {
    let text = "contract C {\nfunction greet(uint a) public {}\nfunction greet(uint a, uint b) public {}\nfunction f() public {\ngreet(1, 2);\n}\n}";
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
fn inherited_plain_completion_shows_base_function() {
    let text = "contract B {\nfunction greet(uint a) public {}\n}\ncontract D is B {\nfunction f() public {\ngre\n}\n}";
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
    let text = "contract B {\nfunction greet(uint a) public {}\n}\ncontract D is B {\nfunction f() public {\ngreet;\n}\n}";
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
    let text = "contract B {\nfunction greet(uint a) public {}\n}\ncontract D is B {\nfunction f() public {\ngreet;\n}\n}";
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
fn inherited_shadow_derived_wins_without_suffix() {
    let text = "contract B {\nfunction greet(uint a) public {}\n}\ncontract D is B {\nfunction greet() public {}\nfunction f() public {\ngreet;\n}\n}";
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
    let text = "contract B {\nfunction greet(uint a, uint b) public {}\n}\ncontract D is B {\nfunction f() public {\ngreet(1);\n}\n}";
    let found = diagnostics_of(text);
    assert_eq!(found.len(), 1, "diags: {found:?}");
    assert_eq!(
        found[0].message,
        "this function 'greet' takes 2 arguments but 1 supplied"
    );
}

#[test]
fn inherited_arity_shadow_uses_derived_signature() {
    let text = "contract B {\nfunction greet(uint a, uint b) public {}\n}\ncontract D is B {\nfunction greet(uint a) public {}\nfunction f() public {\ngreet(1, 2);\n}\n}";
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
    let dep = "contract Hello {\nfunction greet(uint a) public {}\n}";
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
