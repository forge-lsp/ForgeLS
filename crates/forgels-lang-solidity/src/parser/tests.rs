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
fn chained_member_call_keeps_tails_with_final_call() {
    let text = "contract C {\nfunction f() public {\na.b.c(1);\n}\n}";
    let lexer = SolidityLexer;
    let tokens = lexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let body = file.contracts()[0].functions()[0].body().expect("body");
    assert_eq!(body.members().len(), 1);
    assert_eq!(body.members()[0].receiver().as_deref(), Some("a"));
    assert_eq!(body.members()[0].member().as_deref(), Some("b"));
    assert_eq!(
        body.members()[0].chain(),
        vec!["a".to_string(), "b".to_string(), "c".to_string()]
    );
    assert_eq!(body.members()[0].tails(), vec!["c".to_string()]);
    assert_eq!(body.members()[0].final_member().as_deref(), Some("c"));
    assert!(body.members()[0].is_call());
    assert_eq!(body.members()[0].arg_count(), 1);
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
fn using_directive_parses_library_and_target() {
    let text = "contract C {\nusing SafeMath for uint;\n}";
    let file = parse_ok(text);
    let usings = file.contracts()[0].usings();
    assert_eq!(usings.len(), 1);
    assert_eq!(usings[0].library().as_deref(), Some("SafeMath"));
    assert_eq!(usings[0].target_text().as_deref(), Some("uint"));
    assert!(!usings[0].is_wildcard());
    assert!(!usings[0].is_global());
}

#[test]
fn using_wildcard_and_global_flags() {
    let text = "contract C {\nusing SafeMath for *;\n}";
    let file = parse_ok(text);
    let usings = file.contracts()[0].usings();
    assert_eq!(usings.len(), 1);
    assert!(usings[0].is_wildcard());
    assert_eq!(usings[0].target_text().as_deref(), Some("*"));
    let global = "using SafeMath for uint global;\ncontract C {}";
    let global_file = parse_ok(global);
    assert_eq!(global_file.usings().len(), 1);
    assert!(global_file.usings()[0].is_global());
}

#[test]
fn using_operator_form_keeps_span_without_library() {
    let text = "contract C {\nusing {add as +} for MyNumber global;\n}";
    let tokens = SolidityLexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let usings = file.contracts()[0].usings();
    assert_eq!(usings.len(), 1);
    assert!(usings[0].library().is_none());
}

#[test]
fn function_virtual_override_and_bases_are_stored() {
    let text = "contract C {\nfunction f() public virtual {}\nfunction g() public override {}\nfunction h() public override(A, B) {}\n}";
    let file = parse_ok(text);
    let functions = file.contracts()[0].functions();
    assert_eq!(functions.len(), 3);
    assert!(functions[0].is_virtual());
    assert!(!functions[0].is_override());
    assert!(!functions[1].is_virtual());
    assert!(functions[1].is_override());
    assert!(functions[1].override_bases().is_empty());
    assert!(functions[2].is_override());
    assert_eq!(functions[2].override_bases(), vec!["A", "B"]);
}

#[test]
fn state_var_public_is_stored_and_name_survives() {
    let text = "contract C {\nuint public total;\nuint private hidden;\n}";
    let file = parse_ok(text);
    let vars = file.contracts()[0].state_vars();
    assert_eq!(vars.len(), 2);
    assert_eq!(vars[0].name().as_deref(), Some("total"));
    assert!(vars[0].is_public());
    assert!(!vars[1].is_public());
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

#[test]
fn sized_edges_keep_text() {
    use crate::TypeNode;
    let text = "contract C {\nuint8 a;\nint8 b;\nuint128 c;\nbytes1 d;\n}";
    let file = parse_ok(text);
    let types: Vec<String> = file.contracts()[0]
        .state_vars()
        .iter()
        .filter_map(|var| var.type_node().map(|type_node| TypeNode::text(&type_node)))
        .collect();
    assert_eq!(types, vec!["uint8", "int8", "uint128", "bytes1"]);
}

#[test]
fn payable_type_keeps_payable_with_arrays() {
    use crate::TypeNode;
    let text = "contract C {\naddress payable public owner;\naddress payable[] wallets;\naddress payable[2] fixed;\n}";
    let file = parse_ok(text);
    let vars = file.contracts()[0].state_vars();
    let names: Vec<String> = vars.iter().filter_map(|var| var.name()).collect();
    assert_eq!(names, vec!["owner", "wallets", "fixed"]);
    let types: Vec<String> = vars
        .iter()
        .filter_map(|var| var.type_node().map(|type_node| TypeNode::text(&type_node)))
        .collect();
    assert_eq!(
        types,
        vec!["address payable", "address payable[]", "address payable[2]"]
    );
}

#[test]
fn outer_fixed_multidim_keeps_text() {
    use crate::TypeNode;
    let text = "contract C {\nuint[][2] matrix;\n}";
    let file = parse_ok(text);
    let types: Vec<String> = file.contracts()[0]
        .state_vars()
        .iter()
        .filter_map(|var| var.type_node().map(|type_node| TypeNode::text(&type_node)))
        .collect();
    assert_eq!(types, vec!["uint[][2]"]);
}

#[test]
fn mapping_keys_keep_text_with_named_key() {
    use crate::TypeNode;
    let text = "contract C {\nmapping(bool => uint) a;\nmapping(string => uint) b;\nmapping(bytes => uint) c;\nmapping(MyContract => uint) d;\nmapping(Color => uint) e;\nmapping(address account => uint) f;\n}";
    let file = parse_ok(text);
    let types: Vec<String> = file.contracts()[0]
        .state_vars()
        .iter()
        .filter_map(|var| var.type_node().map(|type_node| TypeNode::text(&type_node)))
        .collect();
    assert_eq!(
        types,
        vec![
            "mapping(bool => uint)",
            "mapping(string => uint)",
            "mapping(bytes => uint)",
            "mapping(MyContract => uint)",
            "mapping(Color => uint)",
            "mapping(address account => uint)",
        ]
    );
}

#[test]
fn mapping_values_keep_array_struct_forms() {
    use crate::TypeNode;
    let text = "struct Person {\nuint age;\n}\ncontract C {\nmapping(address => uint[]) a;\nmapping(uint => Person) b;\nmapping(uint => Person[]) c;\n}\nstruct Vault {\nmapping(address => uint) balances;\n}";
    let file = parse_ok(text);
    let types: Vec<String> = file.contracts()[0]
        .state_vars()
        .iter()
        .filter_map(|var| var.type_node().map(|type_node| TypeNode::text(&type_node)))
        .collect();
    assert_eq!(
        types,
        vec![
            "mapping(address => uint[])",
            "mapping(uint => Person)",
            "mapping(uint => Person[])",
        ]
    );
    let vault = file
        .structs()
        .into_iter()
        .find(|item| item.name().as_deref() == Some("Vault"))
        .expect("vault");
    let fields: Vec<String> = vault
        .fields()
        .iter()
        .filter_map(|field| field.type_node().map(|node| node.text()))
        .collect();
    assert_eq!(fields, vec!["mapping(address => uint)"]);
}

#[test]
fn fixed_ufixed_and_udvt_sized_keep_text() {
    use crate::TypeNode;
    let text = "type MyNumber is uint128;\ntype Small is uint8;\ncontract C {\nfixed a;\nufixed b;\nfixed128x18 c;\nufixed128x18 d;\n}";
    let file = parse_ok(text);
    let underlying: Vec<String> = file
        .user_types()
        .iter()
        .filter_map(|item| item.underlying().map(|node| node.text()))
        .collect();
    assert_eq!(underlying, vec!["uint128", "uint8"]);
    let types: Vec<String> = file.contracts()[0]
        .state_vars()
        .iter()
        .filter_map(|var| var.type_node().map(|type_node| TypeNode::text(&type_node)))
        .collect();
    assert_eq!(
        types,
        vec!["fixed", "ufixed", "fixed128x18", "ufixed128x18"]
    );
}

#[test]
fn function_visibility_and_mutability_are_stored() {
    use crate::{Mutability, Visibility};
    let text = "contract C {\nfunction a() public pure {}\nfunction b() external view {}\nfunction c() internal {}\nfunction d() private payable {}\n}";
    let file = parse_ok(text);
    let functions = file.contracts()[0].functions();
    assert_eq!(functions.len(), 4);
    assert_eq!(functions[0].visibility(), Some(Visibility::Public));
    assert_eq!(functions[0].mutability(), Some(Mutability::Pure));
    assert_eq!(functions[1].visibility(), Some(Visibility::External));
    assert_eq!(functions[1].mutability(), Some(Mutability::View));
    assert_eq!(functions[2].visibility(), Some(Visibility::Internal));
    assert_eq!(functions[2].mutability(), None);
    assert_eq!(functions[3].visibility(), Some(Visibility::Private));
    assert_eq!(functions[3].mutability(), Some(Mutability::Payable));
}

#[test]
fn function_constant_normalizes_to_view() {
    use crate::Mutability;
    let text = "contract C {\nfunction f() public constant {}\n}";
    let file = parse_ok(text);
    let functions = file.contracts()[0].functions();
    assert_eq!(functions.len(), 1);
    assert_eq!(functions[0].mutability(), Some(Mutability::View));
    assert!(functions[0].mutability_span().is_some());
}

#[test]
fn function_without_modifiers_has_no_visibility() {
    let text = "contract C {\nfunction f(uint a) {}\n}";
    let file = parse_ok(text);
    let functions = file.contracts()[0].functions();
    assert_eq!(functions.len(), 1);
    assert_eq!(functions[0].visibility(), None);
    assert_eq!(functions[0].mutability(), None);
    assert!(functions[0].visibility_span().is_none());
}

#[test]
fn state_var_single_literal_initializer_is_stored() {
    let text = "contract C {\nuint x = 1;\nstring s = \"hi\";\nbool b = true;\n}";
    let file = parse_ok(text);
    let vars = file.contracts()[0].state_vars();
    assert_eq!(vars.len(), 3);
    assert_eq!(
        vars[0]
            .initializer()
            .as_ref()
            .map(|token| token.text.as_str()),
        Some("1")
    );
    assert!(vars[0].initializer_span().is_some());
    assert_eq!(
        vars[1]
            .initializer()
            .as_ref()
            .map(|token| token.text.as_str()),
        Some("\"hi\"")
    );
    assert_eq!(
        vars[2]
            .initializer()
            .as_ref()
            .map(|token| token.text.as_str()),
        Some("true")
    );
}

#[test]
fn state_var_complex_initializer_stores_nothing() {
    let text = "contract C {\nuint a = 1 ether;\nuint b = y;\nuint c = 1 + 2;\nuint d;\n}";
    let file = parse_ok(text);
    let vars = file.contracts()[0].state_vars();
    assert_eq!(vars.len(), 4);
    assert!(vars.iter().all(|var| var.initializer().is_none()));
}

#[test]
fn local_var_single_literal_is_stored() {
    let text = "contract C {\nfunction f() public {\nuint x = 42;\nuint y = x;\n}\n}";
    let file = parse_ok(text);
    let body = file.contracts()[0].functions()[0].body().expect("body");
    let locals = body.locals();
    assert_eq!(locals.len(), 2);
    assert_eq!(
        locals[0]
            .initializer()
            .as_ref()
            .map(|token| token.text.as_str()),
        Some("42")
    );
    assert!(locals[1].initializer().is_none());
}

#[test]
fn call_options_store_markers_and_span() {
    let text = "contract C {\nfunction f() public {\ngreet{value: 1}(1, 2);\n}\n}";
    let file = parse_ok(text);
    let body = file.contracts()[0].functions()[0].body().expect("body");
    let calls = body.calls();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].has_call_options());
    assert!(calls[0].options_span().is_some());
    assert_eq!(calls[0].arg_count(), 2);
    let covered = &text[calls[0].options_span().expect("span").start as usize
        ..calls[0].options_span().expect("span").end as usize];
    assert!(covered.starts_with('{') && covered.ends_with('}'));
}

#[test]
fn call_without_options_has_no_span() {
    let text = "contract C {\nfunction f() public {\ngreet(1);\n}\n}";
    let file = parse_ok(text);
    let body = file.contracts()[0].functions()[0].body().expect("body");
    let calls = body.calls();
    assert_eq!(calls.len(), 1);
    assert!(!calls[0].has_call_options());
    assert!(calls[0].options_span().is_none());
    assert_eq!(calls[0].arg_count(), 1);
}

#[test]
fn member_options_store_markers_and_span() {
    let text = "contract C {\nfunction f() public {\nobj.greet{value: 1}(1);\n}\n}";
    let file = parse_ok(text);
    let body = file.contracts()[0].functions()[0].body().expect("body");
    let members = body.members();
    assert_eq!(members.len(), 1);
    assert!(members[0].has_call_options());
    assert!(members[0].options_span().is_some());
    assert!(members[0].is_call());
    assert_eq!(members[0].arg_count(), 1);
}

#[test]
fn member_options_without_parens_falls_back_to_plain() {
    let text = "contract C {\nfunction f() public {\nobj.greet{value: 1};\n}\n}";
    let lexer = SolidityLexer;
    let tokens = lexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let body = file.contracts()[0].functions()[0].body().expect("body");
    assert_eq!(body.members().len(), 1);
    assert!(!body.members()[0].has_call_options());
    assert!(!body.members()[0].is_call());
}

#[test]
fn nested_blocks_recurse_via_all() {
    use crate::LocalVar;
    let text = "contract C {\nfunction f() public {\nuint a = 1;\nif (true) {\nuint b = 2;\nwhile (a > 0) {\nuint c = 3;\n}\n}\n}\n}";
    let file = parse_ok(text);
    let body = file.contracts()[0].functions()[0].body().expect("body");
    let direct: Vec<String> = body.locals().iter().filter_map(LocalVar::name).collect();
    assert_eq!(direct, vec!["a"]);
    assert_eq!(body.child_blocks().len(), 1);
    let all: Vec<String> = body
        .all_locals()
        .iter()
        .filter_map(LocalVar::name)
        .collect();
    assert_eq!(all, vec!["a", "b", "c"]);
}

#[test]
fn for_header_decl_is_collected() {
    use crate::LocalVar;
    let text = "contract C {\nfunction f() public {\nfor (uint i = 0; i < 10; i++) {\nuint x = 1;\n}\n}\n}";
    let file = parse_ok(text);
    let body = file.contracts()[0].functions()[0].body().expect("body");
    let all: Vec<String> = body
        .all_locals()
        .iter()
        .filter_map(LocalVar::name)
        .collect();
    assert_eq!(all, vec!["i", "x"]);
}

#[test]
fn for_without_decl_collects_nothing() {
    let text = "contract C {\nfunction f() public {\nuint i = 0;\nfor (; i < 10; i++) {\nuint x = 1;\n}\nfor (i = 0; i < 5; i++) {\n}\n}\n}";
    let lexer = SolidityLexer;
    let tokens = lexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let body = file.contracts()[0].functions()[0].body().expect("body");
    let all: Vec<String> = body
        .all_locals()
        .iter()
        .filter_map(|local| local.name())
        .collect();
    assert_eq!(all, vec!["i", "x"]);
}

#[test]
fn tuple_decl_expands_per_name() {
    use crate::LocalVar;
    let text = "contract C {\nfunction f() public {\n(uint a, uint b) = pair();\n}\n}";
    let file = parse_ok(text);
    let body = file.contracts()[0].functions()[0].body().expect("body");
    let names: Vec<String> = body.locals().iter().filter_map(LocalVar::name).collect();
    assert_eq!(names, vec!["a", "b"]);
}

#[test]
fn tuple_assignment_holds_no_symbols() {
    let text =
        "contract C {\nfunction f() public {\nuint a = 1;\nuint b = 2;\n(a, b) = pair();\n}\n}";
    let file = parse_ok(text);
    let body = file.contracts()[0].functions()[0].body().expect("body");
    let names: Vec<String> = body
        .all_locals()
        .iter()
        .filter_map(|local| local.name())
        .collect();
    assert_eq!(names, vec!["a", "b"]);
}

#[test]
fn try_catch_and_unchecked_bodies_parse() {
    use crate::LocalVar;
    let text = "contract C {\nfunction f() public {\nunchecked {\nuint a = 1;\n}\ntry this.greet() {\nuint b = 2;\n} catch (bytes memory e) {\nuint c = 3;\n} catch {\nuint d = 4;\n}\n}\n}";
    let lexer = SolidityLexer;
    let tokens = lexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let body = file.contracts()[0].functions()[0].body().expect("body");
    let all: Vec<String> = body
        .all_locals()
        .iter()
        .filter_map(LocalVar::name)
        .collect();
    assert_eq!(all, vec!["a", "b", "c", "d"]);
}

#[test]
fn try_returns_and_catch_params_become_wrapper_params() {
    use crate::Param;
    let text = "contract C {\nfunction f() public {\ntry this.greet() returns (uint amount) {\nuint b = amount;\n} catch (bytes memory e) {\nuint c = 1;\n}\n}\n}";
    let lexer = SolidityLexer;
    let tokens = lexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let body = file.contracts()[0].functions()[0].body().expect("body");
    assert_eq!(body.child_blocks().len(), 1);
    let wrapper = body.child_blocks()[0].clone();
    let names: Vec<String> = wrapper.params().iter().filter_map(Param::name).collect();
    assert_eq!(names, vec!["amount", "e"]);
    let all_params: Vec<String> = body.all_params().iter().filter_map(Param::name).collect();
    assert_eq!(all_params, vec!["amount", "e"]);
}

#[test]
fn try_new_parses_deploy_for_arity() {
    let text = "contract Vault {\nconstructor(uint a, uint b) {}\n}\ncontract C {\nfunction f() public {\ntry new Vault(1) {\n} catch {\n}\n}\n}";
    let lexer = SolidityLexer;
    let tokens = lexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let body = file.contracts()[1].functions()[0].body().expect("body");
    assert_eq!(body.all_news().len(), 1);
    assert_eq!(body.all_news()[0].contract_name().as_deref(), Some("Vault"));
    assert_eq!(body.all_news()[0].arg_count(), 1);
}

#[test]
fn chained_tails_cap_at_four() {
    let text = "contract C {\nfunction f() public {\na.b.c.d.e.f.g;\n}\n}";
    let lexer = SolidityLexer;
    let tokens = lexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let body = file.contracts()[0].functions()[0].body().expect("body");
    assert_eq!(body.members().len(), 1);
    let chain = body.members()[0].chain();
    assert_eq!(chain.len(), 6);
    assert_eq!(body.members()[0].tails().len(), 4);
    assert_eq!(body.members()[0].final_member().as_deref(), Some("f"));
}

#[test]
fn chained_intermediate_call_is_skipped() {
    let text = "contract C {\nfunction f() public {\na.b().c;\n}\n}";
    let lexer = SolidityLexer;
    let tokens = lexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let body = file.contracts()[0].functions()[0].body().expect("body");
    assert_eq!(body.members().len(), 1);
    assert_eq!(
        body.members()[0].chain(),
        vec!["a".to_string(), "b".to_string(), "c".to_string()]
    );
    assert!(!body.members()[0].is_call());
}

#[test]
fn assembly_inside_control_stays_skipped() {
    use crate::LocalVar;
    let text = "contract C {\nfunction f() public {\nif (true) {\nassembly {\nlet y := 1\n}\nuint z = 2;\n}\n}\n}";
    let lexer = SolidityLexer;
    let tokens = lexer.tokenize(text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "errors: {errors:?}");
    let body = file.contracts()[0].functions()[0].body().expect("body");
    let all: Vec<String> = body
        .all_locals()
        .iter()
        .filter_map(LocalVar::name)
        .collect();
    assert_eq!(all, vec!["z"]);
}

#[test]
fn non_brace_body_falls_back_without_node() {
    let text = "contract C {\nfunction f() public {\nif (true)\nrevert();\nuint x = 1;\n}\n}";
    let file = parse_ok(text);
    let body = file.contracts()[0].functions()[0].body().expect("body");
    assert!(body.child_blocks().is_empty());
    let names: Vec<String> = body
        .all_locals()
        .iter()
        .filter_map(|local| local.name())
        .collect();
    assert_eq!(names, vec!["x"]);
}

#[test]
fn error_params_are_stored() {
    let text = "contract C {\nerror Insufficient(uint a, uint b);\n}";
    let file = parse_ok(text);
    let errors = file.contracts()[0].errors();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].name().as_deref(), Some("Insufficient"));
    assert_eq!(errors[0].params().len(), 2);
}

#[test]
fn constructors_are_listed_with_params() {
    let text = "contract C {\nconstructor(uint a) {}\nfunction f() public {}\n}";
    let file = parse_ok(text);
    let constructors = file.contracts()[0].constructors();
    assert_eq!(constructors.len(), 1);
    assert_eq!(constructors[0].params().len(), 1);
    let empty = "contract C {\nfunction f() public {}\n}";
    let plain = parse_ok(empty);
    assert!(plain.contracts()[0].constructors().is_empty());
}

#[test]
fn revert_parses_with_name_and_args() {
    let text = "contract C {\nfunction f() public {\nrevert Insufficient(1, 2);\n}\n}";
    let file = parse_ok(text);
    let body = file.contracts()[0].functions()[0].body().expect("body");
    let reverts = body.reverts();
    assert_eq!(reverts.len(), 1);
    assert_eq!(reverts[0].name().as_deref(), Some("Insufficient"));
    assert_eq!(reverts[0].arg_count(), 2);
    assert_eq!(body.all_reverts().len(), 1);
}

#[test]
fn new_parses_with_contract_and_args() {
    let text = "contract C {\nfunction f() public {\nnew Vault(1, 2);\n}\n}";
    let file = parse_ok(text);
    let body = file.contracts()[0].functions()[0].body().expect("body");
    let news = body.news();
    assert_eq!(news.len(), 1);
    assert_eq!(news[0].contract_name().as_deref(), Some("Vault"));
    assert_eq!(news[0].arg_count(), 2);
    assert_eq!(body.all_news().len(), 1);
}

#[test]
fn data_location_stored_for_param_state_and_local() {
    use crate::DataLocation;
    let text = "contract C {\nstring memory label;\nfunction f(string calldata a, uint b) public {\nstring memory s;\nuint x;\n}\n}";
    let file = parse_ok(text);
    let contract = &file.contracts()[0];
    let state = &contract.state_vars()[0];
    assert_eq!(state.name().as_deref(), Some("label"));
    assert_eq!(state.data_location(), Some(DataLocation::Memory));
    assert!(state.data_location_span().is_some());
    let params = contract.functions()[0].params();
    assert_eq!(params.len(), 2);
    assert_eq!(params[0].name().as_deref(), Some("a"));
    assert_eq!(params[0].data_location(), Some(DataLocation::Calldata));
    assert_eq!(params[1].name().as_deref(), Some("b"));
    assert_eq!(params[1].data_location(), None);
    let body = contract.functions()[0].body().expect("body");
    let locals = body.all_locals();
    assert_eq!(locals.len(), 2);
    assert_eq!(locals[0].name().as_deref(), Some("s"));
    assert_eq!(locals[0].data_location(), Some(DataLocation::Memory));
    assert_eq!(locals[1].name().as_deref(), Some("x"));
    assert_eq!(locals[1].data_location(), None);
}

#[test]
fn data_location_name_skips_location_token() {
    let text = "contract C {\nstring memory label;\n}";
    let file = parse_ok(text);
    let var = &file.contracts()[0].state_vars()[0];
    assert_eq!(var.name().as_deref(), Some("label"));
    assert!(var.name_span().is_some());
    let covered = &text[var.name_span().expect("span").start as usize
        ..var.name_span().expect("span").end as usize];
    assert_eq!(covered, "label");
}
