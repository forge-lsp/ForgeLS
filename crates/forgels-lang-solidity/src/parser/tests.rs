use super::SolidityParser;
use crate::{SolidityFile, SolidityLexer};
use forgels_syntax::{Lexer, Parser};

fn parse_ok(text: &str) -> SolidityFile {
    SolidityParser
        .parse_text(text)
        .expect("valid skeleton parses")
}

#[test]
fn valid_skeleton_parses_without_error() {
    let text = "pragma solidity ^0.8.0;\nimport {A as B, C} from \"./A.sol\";\ncontract Token {}\n";
    let file = parse_ok(text);
    assert_eq!(file.pragmas().len(), 1);
    assert_eq!(file.imports().len(), 1);
    assert_eq!(file.contracts().len(), 1);
    assert_eq!(file.pragmas()[0].name().as_deref(), Some("solidity"));
    assert_eq!(file.imports()[0].path().as_deref(), Some("./A.sol"));
    assert_eq!(
        file.imports()[0].symbols(),
        vec!["A".to_string(), "B".to_string(), "C".to_string()]
    );
    assert_eq!(file.contracts()[0].name().as_deref(), Some("Token"));
}

#[test]
fn all_import_forms_parse() {
    let text =
        "import \"./A.sol\";\nimport \"./B.sol\" as NS;\nimport * as All from \"./C.sol\";\n";
    let file = parse_ok(text);
    assert_eq!(file.imports().len(), 3);
    assert_eq!(file.imports()[0].path().as_deref(), Some("./A.sol"));
    assert_eq!(file.imports()[1].symbols(), vec!["NS".to_string()]);
    assert_eq!(file.imports()[2].symbols(), vec!["All".to_string()]);
}

#[test]
fn empty_file_parses_to_empty_skeleton() {
    let file = parse_ok("");
    assert!(file.pragmas().is_empty());
    assert!(file.imports().is_empty());
    assert!(file.contracts().is_empty());
}

#[test]
fn missing_semicolon_errors_with_span() {
    let text = "pragma solidity ^0.8.0\ncontract C {}";
    let error = SolidityParser.parse_text(text).expect_err("must fail");
    assert!(!error.message.is_empty());
    assert!(error.span.start < text.len() as u32);
}

#[test]
fn contract_without_body_errors() {
    let text = "contract C";
    let error = SolidityParser.parse_text(text).expect_err("must fail");
    assert_eq!(error.message, "expected `{`");
}

#[test]
fn two_broken_contracts_yield_two_errors() {
    let text = "contract ;\ncontract ;\n";
    let lexer = SolidityLexer;
    let tokens = lexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert_eq!(errors.len(), 2);
    assert!(file.contracts().is_empty());
    for error in &errors {
        assert_eq!(error.message, "expected contract name");
    }
}

#[test]
fn one_broken_item_keeps_valid_siblings() {
    let text = "contract ;\ncontract Good {}\n";
    let lexer = SolidityLexer;
    let tokens = lexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert_eq!(errors.len(), 1);
    assert_eq!(file.contracts().len(), 1);
    assert_eq!(file.contracts()[0].name().as_deref(), Some("Good"));
}

#[test]
fn state_variables_cover_beta_types() {
    use crate::{StateVar, TypeNode};
    let text = "contract C {\nuint256 total;\naddress owner;\nbool flag;\nmapping(address => uint) balances;\nuint[] scores;\nstring memory label;\n}";
    let file = parse_ok(text);
    let contracts = file.contracts();
    assert_eq!(contracts.len(), 1);
    let names: Vec<String> = contracts[0]
        .state_vars()
        .iter()
        .filter_map(StateVar::name)
        .collect();
    assert_eq!(
        names,
        vec!["total", "owner", "flag", "balances", "scores", "label"]
    );
    let types: Vec<String> = contracts[0]
        .state_vars()
        .iter()
        .filter_map(|var| var.type_node().map(|type_node| TypeNode::text(&type_node)))
        .collect();
    assert_eq!(
        types,
        vec![
            "uint256",
            "address",
            "bool",
            "mapping(address => uint)",
            "uint[]",
            "string",
        ]
    );
}

#[test]
fn function_with_params_returns_and_modifiers() {
    let text =
        "contract C {\nfunction add(uint a, uint b) public pure returns (uint) {\nreturn a;\n}\n}";
    let file = parse_ok(text);
    let functions = file.contracts()[0].functions();
    assert_eq!(functions.len(), 1);
    assert_eq!(functions[0].name().as_deref(), Some("add"));
    let params: Vec<String> = functions[0]
        .params()
        .iter()
        .filter_map(|param| param.name())
        .collect();
    assert_eq!(params, vec!["a", "b"]);
    assert_eq!(functions[0].return_types().len(), 1);
    assert!(functions[0].body().is_some());
}

#[test]
fn locals_are_collected_and_statements_skipped() {
    use crate::LocalVar;
    let text =
        "contract C {\nfunction f() public {\nuint x = 1;\nx = x + 1;\nstring memory s;\n}\n}";
    let file = parse_ok(text);
    let body = file.contracts()[0].functions()[0].body().expect("body");
    let names: Vec<String> = body.locals().iter().filter_map(LocalVar::name).collect();
    assert_eq!(names, vec!["x", "s"]);
}

#[test]
fn broken_member_keeps_valid_siblings() {
    let text = "contract C {\nuint256 good;\nuint ;\nbool fine;\n}";
    let lexer = SolidityLexer;
    let tokens = lexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert_eq!(errors.len(), 1);
    let names: Vec<String> = file.contracts()[0]
        .state_vars()
        .iter()
        .filter_map(|var| var.name())
        .collect();
    assert_eq!(names, vec!["good", "fine"]);
}

#[test]
fn nested_mapping_text_has_no_stray_spaces() {
    use crate::TypeNode;
    let text = "contract C {\nmapping(address => mapping(uint => bool)) nested;\nuint[] scores;\n}";
    let file = parse_ok(text);
    let types: Vec<String> = file.contracts()[0]
        .state_vars()
        .iter()
        .filter_map(|var| var.type_node().map(|type_node| TypeNode::text(&type_node)))
        .collect();
    assert_eq!(
        types,
        vec!["mapping(address => mapping(uint => bool))", "uint[]",]
    );
}

#[test]
fn multidim_array_text_has_no_stray_spaces() {
    use crate::TypeNode;
    let text = "contract C {\nuint[2][] matrix;\nbytes32[4] fixedArr;\n}";
    let file = parse_ok(text);
    let types: Vec<String> = file.contracts()[0]
        .state_vars()
        .iter()
        .filter_map(|var| var.type_node().map(|type_node| TypeNode::text(&type_node)))
        .collect();
    assert_eq!(types, vec!["uint[2][]", "bytes32[4]"]);
}

#[test]
fn contract_with_heritage_lists_bases() {
    let text = "contract C is A, B {\n}";
    let file = parse_ok(text);
    assert_eq!(file.contracts()[0].name().as_deref(), Some("C"));
    assert_eq!(file.contracts()[0].heritage(), vec!["A", "B"]);
    assert!(!file.contracts()[0].is_abstract());
}

#[test]
fn abstract_contract_with_base_args() {
    let text = "abstract contract C is A(1), B {\nuint x;\n}";
    let file = parse_ok(text);
    assert!(file.contracts()[0].is_abstract());
    assert_eq!(file.contracts()[0].heritage(), vec!["A", "B"]);
    assert_eq!(file.contracts()[0].state_vars().len(), 1);
}

#[test]
fn contract_without_heritage_has_empty_list() {
    let text = "contract C {\n}";
    let file = parse_ok(text);
    assert!(file.contracts()[0].heritage().is_empty());
    assert!(!file.contracts()[0].is_abstract());
}

#[test]
fn heritage_without_base_name_errors() {
    let text = "contract C is {\n}";
    let lexer = SolidityLexer;
    let tokens = lexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "expected base name");
    assert!(file.contracts().is_empty());
}

#[test]
fn assembly_block_skips_balanced_without_error() {
    let text = "contract C {\nfunction f() public {\nuint x = 1;\nassembly {\nlet y := add(x, 1)\n}\nuint z = 2;\n}\n}";
    let lexer = SolidityLexer;
    let tokens = lexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let body = file.contracts()[0].functions()[0].body().expect("body");
    let names: Vec<String> = body
        .locals()
        .iter()
        .filter_map(|local| local.name())
        .collect();
    assert_eq!(names, vec!["x", "z"]);
}

#[test]
fn assembly_with_dialect_string_skips() {
    let text = "contract C {\nfunction f() public {\nassembly \"evmasm\" {\nlet y := 1\n}\n}\n}";
    let lexer = SolidityLexer;
    let tokens = lexer.tokenize(text);
    let (_, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
}

#[test]
fn experimental_pragma_parses_to_semicolon() {
    let text = "pragma experimental ABIEncoderV2;\ncontract C {\n}";
    let file = parse_ok(text);
    assert_eq!(file.pragmas().len(), 1);
    assert_eq!(file.pragmas()[0].name().as_deref(), Some("experimental"));
}

#[test]
fn call_with_nested_args_counts_top_level_only() {
    use forgels_syntax::AstNode;
    let text = "contract C {\nfunction f() public {\ngreet(bar(1, 2), [3, 4]);\n}\nfunction greet(uint a, uint b) public {}\n}";
    let file = parse_ok(text);
    let body = file.contracts()[0].functions()[0].body().expect("body");
    let calls = body.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].callee().as_deref(), Some("greet"));
    assert_eq!(calls[0].arg_count(), 2);
    let span_text =
        &text[calls[0].syntax().span.start as usize..calls[0].syntax().span.end as usize];
    assert!(span_text.ends_with(')'), "span: {span_text}");
}

#[test]
fn empty_call_has_zero_args() {
    let text = "contract C {\nfunction f() public {\ngreet();\n}\nfunction greet() public {}\n}";
    let file = parse_ok(text);
    let body = file.contracts()[0].functions()[0].body().expect("body");
    let calls = body.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].arg_count(), 0);
}

#[test]
fn member_and_named_calls_produce_no_node() {
    let text = "contract C {\nfunction f() public {\nobj.greet(1);\ngreet({value: 1});\n}\n}";
    let lexer = SolidityLexer;
    let tokens = lexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let body = file.contracts()[0].functions()[0].body().expect("body");
    assert!(body.calls().is_empty());
}

#[test]
fn calls_do_not_change_locals() {
    use crate::LocalVar;
    let text =
        "contract C {\nfunction f() public {\nuint x = 1;\ngreet(x);\nstring memory s;\n}\n}";
    let file = parse_ok(text);
    let body = file.contracts()[0].functions()[0].body().expect("body");
    let names: Vec<String> = body.locals().iter().filter_map(LocalVar::name).collect();
    assert_eq!(names, vec!["x", "s"]);
    assert_eq!(body.calls().len(), 1);
}
