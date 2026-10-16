use super::{
    CompletionItem, Scope, SolidityAnalyzer, SymbolKind, collect_symbols, complete_at,
    complete_import_paths, exports_of, goto_definition, hover_at, import_path_at,
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
    assert!(found.iter().any(|item| item.label == "./B.sol"));
    let _ = std::fs::remove_dir_all(&root);
}
