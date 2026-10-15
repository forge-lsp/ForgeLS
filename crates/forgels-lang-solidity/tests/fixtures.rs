use forgels_lang_solidity::{SolidityLexer, SolidityParser, TypeNode};
use forgels_syntax::{Lexer, Parser};

fn load(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).expect("fixture exists")
}

fn parse_fixture(name: &str) -> forgels_lang_solidity::SolidityFile {
    let text = load(name);
    let tokens = SolidityLexer.tokenize(&text);
    let (file, errors) = SolidityParser.parse_with_errors(&tokens);
    assert!(errors.is_empty(), "{name} errors: {errors:?}");
    file
}

#[test]
fn skeleton_parses_directives_and_heritage() {
    let file = parse_fixture("skeleton.sol");
    assert_eq!(file.pragmas().len(), 2);
    assert_eq!(file.pragmas()[1].name().as_deref(), Some("experimental"));
    assert_eq!(file.imports().len(), 4);
    assert_eq!(
        file.imports()[3].symbols(),
        vec!["A".to_string(), "B".to_string(), "C".to_string()]
    );
    let contracts = file.contracts();
    assert_eq!(contracts.len(), 1);
    assert_eq!(contracts[0].name().as_deref(), Some("Token"));
    assert!(contracts[0].is_abstract());
    assert_eq!(contracts[0].heritage(), vec!["A", "B"]);
}

#[test]
fn members_cover_state_vars_and_functions() {
    let file = parse_fixture("members.sol");
    let contracts = file.contracts();
    assert_eq!(contracts.len(), 1);
    assert_eq!(contracts[0].heritage(), vec!["Ownable"]);
    let names: Vec<String> = contracts[0]
        .state_vars()
        .iter()
        .filter_map(|var| var.name())
        .collect();
    assert_eq!(
        names,
        vec![
            "total",
            "owner",
            "paused",
            "label",
            "root",
            "balances",
            "approvals",
            "scores",
            "matrix",
            "rate",
        ]
    );
    let types: Vec<String> = contracts[0]
        .state_vars()
        .iter()
        .filter_map(|var| var.type_node().map(|node| TypeNode::text(&node)))
        .collect();
    assert_eq!(
        types,
        vec![
            "uint256",
            "address",
            "bool",
            "string",
            "bytes32",
            "mapping(address => uint)",
            "mapping(address => mapping(uint => bool))",
            "uint[]",
            "uint[2][]",
            "fixed128x18",
        ]
    );
    let functions = contracts[0].functions();
    assert_eq!(functions.len(), 3);
    assert_eq!(functions[1].name().as_deref(), Some("deposit"));
    assert_eq!(functions[2].return_types().len(), 1);
}

#[test]
fn body_keeps_locals_past_assembly() {
    let file = parse_fixture("body.sol");
    let body = file.contracts()[0].functions()[0].body().expect("body");
    let names: Vec<String> = body
        .locals()
        .iter()
        .filter_map(|local| local.name())
        .collect();
    assert_eq!(names, vec!["sum"]);
}
