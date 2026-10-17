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

#[test]
fn file_struct_and_enum_parse() {
    let text = "struct Person {\nuint age;\n}\nenum Color {\nRed,\nBlue\n}\ncontract C {\n}";
    let file = parse_ok(text);
    let structs: Vec<String> = file
        .structs()
        .iter()
        .filter_map(|item| item.name())
        .collect();
    assert_eq!(structs, vec!["Person"]);
    let enums: Vec<String> = file.enums().iter().filter_map(|item| item.name()).collect();
    assert_eq!(enums, vec!["Color"]);
}

#[test]
fn contract_struct_and_enum_are_members() {
    let text = "contract C is A {\nstruct Person {\nuint age;\n}\nenum Color {\nRed\n}\nuint x;\n}";
    let file = parse_ok(text);
    let contract = &file.contracts()[0];
    assert_eq!(contract.heritage(), vec!["A"]);
    let structs: Vec<String> = contract
        .structs()
        .iter()
        .filter_map(|item| item.name())
        .collect();
    assert_eq!(structs, vec!["Person"]);
    let enums: Vec<String> = contract
        .enums()
        .iter()
        .filter_map(|item| item.name())
        .collect();
    assert_eq!(enums, vec!["Color"]);
    assert_eq!(contract.state_vars().len(), 1);
}

#[test]
fn unclosed_struct_still_yields_name() {
    let text = "contract C {\nstruct Person {\nuint age;\n";
    let lexer = SolidityLexer;
    let tokens = lexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    let names: Vec<String> = file.contracts()[0]
        .structs()
        .iter()
        .filter_map(|item| item.name())
        .collect();
    assert_eq!(names, vec!["Person"]);
}

#[test]
fn unclosed_file_struct_still_yields_name() {
    let text = "struct Person {\nuint age;\n";
    let lexer = SolidityLexer;
    let tokens = lexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    let names: Vec<String> = file
        .structs()
        .iter()
        .filter_map(|item| item.name())
        .collect();
    assert_eq!(names, vec!["Person"]);
}

#[test]
fn unclosed_enum_still_yields_name() {
    let text = "contract C {\nenum Color {\nRed,\n";
    let lexer = SolidityLexer;
    let tokens = lexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    let names: Vec<String> = file.contracts()[0]
        .enums()
        .iter()
        .filter_map(|item| item.name())
        .collect();
    assert_eq!(names, vec!["Color"]);
}

#[test]
fn struct_fields_keep_type_and_name() {
    let text = "struct Person {\nuint age;\nstring memory name;\n}";
    let file = parse_ok(text);
    let fields = file.structs()[0].fields();
    let names: Vec<String> = fields.iter().filter_map(|field| field.name()).collect();
    assert_eq!(names, vec!["age", "name"]);
    let types: Vec<String> = fields
        .iter()
        .filter_map(|field| field.type_node().map(|node| node.text()))
        .collect();
    assert_eq!(types, vec!["uint", "string"]);
}

#[test]
fn enum_variants_are_stored() {
    let text = "enum Color {\nRed,\nBlue\n}";
    let file = parse_ok(text);
    assert_eq!(file.enums()[0].variants(), vec!["Red", "Blue"]);
}

#[test]
fn block_members_collect_single_dot() {
    let text = "contract C {\nfunction f() public {\nPerson p;\np.age;\nthis.total;\n}\n}";
    let file = parse_ok(text);
    let body = file.contracts()[0].functions()[0].body().expect("body");
    let pairs: Vec<(Option<String>, Option<String>)> = body
        .members()
        .iter()
        .map(|member| (member.receiver(), member.member()))
        .collect();
    assert!(pairs.contains(&(Some("p".to_string()), Some("age".to_string()))));
    assert!(pairs.contains(&(Some("this".to_string()), Some("total".to_string()))));
    assert!(body.members().iter().all(|member| !member.is_call()));
}

#[test]
fn member_call_markers_cover_args() {
    use forgels_syntax::AstNode;
    let text = "contract C {\nfunction f() public {\nobj.greet(1, 2);\n}\n}";
    let file = parse_ok(text);
    let body = file.contracts()[0].functions()[0].body().expect("body");
    assert!(body.calls().is_empty());
    let members = body.members();
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].receiver().as_deref(), Some("obj"));
    assert_eq!(members[0].member().as_deref(), Some("greet"));
    assert!(members[0].is_call());
    assert_eq!(members[0].arg_count(), 2);
    let span_text =
        &text[members[0].syntax().span.start as usize..members[0].syntax().span.end as usize];
    assert!(span_text.starts_with("obj.greet") && span_text.ends_with(')'));
}

#[test]
fn empty_member_call_marks_call_with_zero_args() {
    let text = "contract C {\nfunction f() public {\nobj.greet();\n}\n}";
    let file = parse_ok(text);
    let body = file.contracts()[0].functions()[0].body().expect("body");
    assert_eq!(body.members().len(), 1);
    assert!(body.members()[0].is_call());
    assert_eq!(body.members()[0].arg_count(), 0);
}

#[test]
fn plain_member_has_no_call_marker() {
    let text = "contract C {\nfunction f() public {\nPerson p;\np.age;\n}\n}";
    let file = parse_ok(text);
    let body = file.contracts()[0].functions()[0].body().expect("body");
    assert_eq!(body.members().len(), 1);
    assert!(!body.members()[0].is_call());
    assert_eq!(body.members()[0].arg_count(), 0);
}

#[test]
fn member_call_with_nested_args_counts_top_level_only() {
    let text = "contract C {\nfunction f() public {\nobj.greet(bar(1, 2), [3, 4]);\n}\n}";
    let file = parse_ok(text);
    let body = file.contracts()[0].functions()[0].body().expect("body");
    assert_eq!(body.members().len(), 1);
    assert!(body.members()[0].is_call());
    assert_eq!(body.members()[0].arg_count(), 2);
}

#[test]
fn member_call_with_brace_falls_back_to_plain() {
    let text = "contract C {\nfunction f() public {\nobj.greet({value: 1});\n}\n}";
    let lexer = SolidityLexer;
    let tokens = lexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let body = file.contracts()[0].functions()[0].body().expect("body");
    assert_eq!(body.members().len(), 1);
    assert_eq!(body.members()[0].receiver().as_deref(), Some("obj"));
    assert_eq!(body.members()[0].member().as_deref(), Some("greet"));
    assert!(!body.members()[0].is_call());
}

#[test]
fn member_call_unclosed_falls_back_to_plain() {
    let text = "contract C {\nfunction f() public {\nobj.greet(1, 2\n}\n}";
    let lexer = SolidityLexer;
    let tokens = lexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    let body = file.contracts()[0].functions()[0].body().expect("body");
    assert_eq!(body.members().len(), 1);
    assert!(!body.members()[0].is_call());
    assert_eq!(body.members()[0].arg_count(), 0);
}

#[test]
fn chained_member_call_keeps_first_pair_only() {
    let text = "contract C {\nfunction f() public {\na.b.c(1);\n}\n}";
    let lexer = SolidityLexer;
    let tokens = lexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let body = file.contracts()[0].functions()[0].body().expect("body");
    assert_eq!(body.members().len(), 1);
    assert_eq!(body.members()[0].receiver().as_deref(), Some("a"));
    assert_eq!(body.members()[0].member().as_deref(), Some("b"));
    assert!(!body.members()[0].is_call());
}

#[test]
fn file_level_using_skips_without_error() {
    let text = "using SafeMath for uint;\nusing {add as +} for MyNumber global;\ncontract C {}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    assert_eq!(file.contracts().len(), 1);
}

#[test]
fn emit_parses_with_name_and_args() {
    let text = "contract C {\nfunction f() public {\nemit Transfer(a, b);\n}\n}";
    let file = parse_ok(text);
    let body = file.contracts()[0].functions()[0].body().expect("body");
    let emits = body.emits();
    assert_eq!(emits.len(), 1);
    assert_eq!(emits[0].name().as_deref(), Some("Transfer"));
    assert_eq!(emits[0].arg_count(), 2);
}

#[test]
fn bare_emit_keeps_partial_node() {
    let text = "contract C {\nfunction f() public {\nemit\n}\n}";
    let lexer = SolidityLexer;
    let tokens = lexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    let body = file.contracts()[0].functions()[0].body().expect("body");
    assert_eq!(body.emits().len(), 1);
    assert!(body.emits()[0].name().is_none());
}

#[test]
fn emit_name_only_keeps_partial_node() {
    let text = "contract C {\nfunction f() public {\nemit Transfer\n}\n}";
    let lexer = SolidityLexer;
    let tokens = lexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    let body = file.contracts()[0].functions()[0].body().expect("body");
    let emits = body.emits();
    assert_eq!(emits.len(), 1);
    assert_eq!(emits[0].name().as_deref(), Some("Transfer"));
    assert_eq!(emits[0].arg_count(), 0);
}

#[test]
fn emit_unclosed_keeps_partial_node() {
    let text = "contract C {\nfunction f() public {\nemit Transfer(a, b\n}\n}";
    let lexer = SolidityLexer;
    let tokens = lexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    let body = file.contracts()[0].functions()[0].body().expect("body");
    let emits = body.emits();
    assert_eq!(emits.len(), 1);
    assert_eq!(emits[0].name().as_deref(), Some("Transfer"));
    assert_eq!(emits[0].arg_count(), 2);
}
