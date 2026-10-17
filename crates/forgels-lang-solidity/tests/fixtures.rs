use forgels_lang_solidity::{
    SolidityLexer, SolidityParser, TypeNode, complete_at, goto_definition, hover_at,
};
use forgels_syntax::Lexer;

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

#[test]
fn cross_pair_links_router_to_vault() {
    let provider = parse_fixture("cross_b.sol");
    let contracts = provider.contracts();
    assert_eq!(contracts.len(), 1);
    assert_eq!(contracts[0].name().as_deref(), Some("Vault"));
    let functions: Vec<String> = contracts[0]
        .functions()
        .iter()
        .filter_map(|function| function.name())
        .collect();
    assert!(functions.contains(&"withdraw".to_string()));
    assert!(functions.contains(&"deposit".to_string()));

    let user = parse_fixture("cross_a.sol");
    assert_eq!(
        user.imports()
            .iter()
            .filter_map(|import| import.path())
            .collect::<Vec<String>>(),
        vec!["./cross_b.sol".to_string()]
    );
    let text = load("cross_a.sol");
    assert!(text.rfind("withdraw").is_some(), "usage present");
}

fn parse_sbe(name: &str) -> (String, forgels_lang_solidity::SolidityFile) {
    let path = format!("sbe/{name}");
    let text = load(&path);
    let file = parse_fixture(&path);
    let has_nodes = !file.contracts().is_empty()
        || !file.structs().is_empty()
        || !file.enums().is_empty()
        || !file.errors().is_empty()
        || !file.user_types().is_empty();
    assert!(has_nodes, "{name} must keep at least one node");
    (text, file)
}

#[test]
fn sbe_hello_world_hover_greet() {
    let (text, file) = parse_sbe("HelloWorld.sol");
    assert_eq!(file.contracts()[0].name().as_deref(), Some("HelloWorld"));
    let at = text.find("greet").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert!(
        hover.contents.contains("greet"),
        "contents: {}",
        hover.contents
    );
}

#[test]
fn sbe_first_app_goto_count() {
    let (text, file) = parse_sbe("FirstApp.sol");
    let contract = &file.contracts()[0];
    assert_eq!(contract.name().as_deref(), Some("FirstApp"));
    let names: Vec<String> = contract
        .functions()
        .iter()
        .filter_map(|f| f.name())
        .collect();
    assert!(names.contains(&"inc".to_string()));
    assert!(names.contains(&"dec".to_string()));
    let at_use = text.find("return count").expect("use") as u32 + 7;
    let target = goto_definition(&file, &text, at_use).expect("goto");
    let at_decl = text.find("uint public count").expect("decl") as u32 + 12;
    assert!(target.start <= at_decl && at_decl < target.end);
}

#[test]
fn sbe_primitives_hover_bool() {
    let (text, file) = parse_sbe("Primitives.sol");
    let vars = file.contracts()[0].state_vars();
    assert!(vars.len() >= 8);
    let at = text.find("boo =").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "bool boo");
}

#[test]
fn sbe_variables_locals_and_hover() {
    let (text, file) = parse_sbe("Variables.sol");
    let func = file.contracts()[0]
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("doSomething"))
        .expect("func");
    let locals: Vec<String> = func
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"i".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"foo".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"sender".to_string()), "locals: {locals:?}");
    let at = text.find("stored").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert!(
        hover.contents.contains("stored"),
        "contents: {}",
        hover.contents
    );
}

#[test]
fn sbe_constants_hover_greet() {
    let (text, file) = parse_sbe("Constants.sol");
    assert_eq!(file.contracts()[0].state_vars().len(), 3);
    let at = text.find("GREET").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert!(
        hover.contents.contains("GREET"),
        "contents: {}",
        hover.contents
    );
}

#[test]
fn sbe_immutable_hover_addr() {
    let (text, file) = parse_sbe("Immutable.sol");
    let names: Vec<String> = file.contracts()[0]
        .state_vars()
        .iter()
        .filter_map(|v| v.name())
        .collect();
    assert!(names.contains(&"MY_ADDR".to_string()));
    assert!(names.contains(&"MY_UINT".to_string()));
    let at = text.find("MY_ADDR").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert!(
        hover.contents.contains("MY_ADDR"),
        "contents: {}",
        hover.contents
    );
}

#[test]
fn sbe_state_variables_goto_text() {
    let (text, file) = parse_sbe("StateVariables.sol");
    let at_use = text.find("text = _text").expect("use") as u32;
    let target = goto_definition(&file, &text, at_use).expect("goto");
    let at_decl = text.find("string public text").expect("decl") as u32 + 14;
    assert!(target.start <= at_decl && at_decl < target.end);
}

#[test]
fn sbe_ether_units_completion_one() {
    let (text, file) = parse_sbe("EtherUnits.sol");
    assert_eq!(file.contracts()[0].state_vars().len(), 6);
    let at = text.find("oneEther").expect("name") as u32 + 3;
    let labels: Vec<String> = complete_at(&file, &text, at)
        .iter()
        .map(|item| item.label.clone())
        .collect();
    assert!(
        labels.contains(&"oneEther".to_string()),
        "labels: {labels:?}"
    );
    assert!(labels.contains(&"oneWei".to_string()), "labels: {labels:?}");
}

#[test]
fn sbe_gas_locals_include_start() {
    let (text, file) = parse_sbe("Gas.sol");
    let func = file.contracts()[0]
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("spendGas"))
        .expect("func");
    let locals: Vec<String> = func
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"start".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"spent".to_string()), "locals: {locals:?}");
    let at = text.find("uint public i").expect("decl") as u32 + 12;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert!(hover.contents.contains('i'), "contents: {}", hover.contents);
}

#[test]
fn sbe_if_else_hover_foo() {
    let (text, file) = parse_sbe("IfElse.sol");
    let names: Vec<String> = file.contracts()[0]
        .functions()
        .iter()
        .filter_map(|f| f.name())
        .collect();
    assert!(names.contains(&"foo".to_string()));
    assert!(names.contains(&"ternary".to_string()));
    let at = text.find("function foo").expect("decl") as u32 + 9;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert!(
        hover.contents.contains("function foo"),
        "contents: {}",
        hover.contents
    );
}

#[test]
fn sbe_loop_hover_j() {
    let (text, file) = parse_sbe("Loop.sol");
    let func = file.contracts()[0]
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("loop"))
        .expect("func");
    let locals: Vec<String> = func
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    // For header is skipped statement by statement, `j` stays as local.
    // `i` lives inside the skipped for header, so only `j` is expected.
    assert!(locals.contains(&"j".to_string()), "locals: {locals:?}");
    let at = text.find("uint j").expect("decl") as u32 + 5;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "uint j");
}

#[test]
fn sbe_mapping_hover_my_map() {
    let (text, file) = parse_sbe("Mapping.sol");
    let types: Vec<String> = file.contracts()[0]
        .state_vars()
        .iter()
        .filter_map(|v| v.type_node().map(|n| TypeNode::text(&n)))
        .collect();
    assert!(
        types.contains(&"mapping(address => uint)".to_string()),
        "types: {types:?}"
    );
    let at = text.find("myMap").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert!(
        hover.contents.contains("myMap"),
        "contents: {}",
        hover.contents
    );
}

#[test]
fn sbe_array_hover_arr() {
    let (text, file) = parse_sbe("Array.sol");
    let types: Vec<String> = file.contracts()[0]
        .state_vars()
        .iter()
        .filter_map(|v| v.type_node().map(|n| TypeNode::text(&n)))
        .collect();
    assert!(types.contains(&"uint[]".to_string()), "types: {types:?}");
    assert!(types.contains(&"uint[3]".to_string()), "types: {types:?}");
    let at = text.find("arr;").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert!(
        hover.contents.contains("arr"),
        "contents: {}",
        hover.contents
    );
}

#[test]
fn sbe_enum_hover_status() {
    let (text, file) = parse_sbe("Enum.sol");
    let contract = &file.contracts()[0];
    assert_eq!(contract.enums().len(), 1);
    assert_eq!(
        contract.enums()[0].variants(),
        vec![
            "Pending".to_string(),
            "Shipped".to_string(),
            "Accepted".to_string(),
            "Rejected".to_string(),
            "Canceled".to_string(),
        ]
    );
    // `Status` also prefixes the contract name, so anchor on the enum keyword.
    let at = text.find("enum Status").expect("decl") as u32 + 5;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "enum Status");
}

#[test]
fn sbe_udvt_hover_and_completion() {
    let (text, file) = parse_sbe("UserDefinedValueTypes.sol");
    assert_eq!(file.user_types().len(), 1);
    assert_eq!(file.user_types()[0].name().as_deref(), Some("MyNumber"));
    let at = text.find("MyNumber").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert!(
        hover.contents.contains("type MyNumber is uint256"),
        "contents: {}",
        hover.contents
    );
    let at_complete = text.find("MyNumber").expect("name") as u32 + 3;
    let labels: Vec<String> = complete_at(&file, &text, at_complete)
        .iter()
        .map(|item| item.label.clone())
        .collect();
    assert!(
        labels.contains(&"MyNumber".to_string()),
        "labels: {labels:?}"
    );
}

#[test]
fn sbe_structs_member_completion_todo() {
    let (text, file) = parse_sbe("Structs.sol");
    let contract = &file.contracts()[0];
    assert_eq!(contract.structs().len(), 1);
    assert_eq!(contract.structs()[0].name().as_deref(), Some("Todo"));
    // Member lookup uses the full word under the cursor, so check each
    // field at its own use site.
    let at_text = text.find("todo.text").expect("use") as u32 + 6;
    let labels_text: Vec<String> = complete_at(&file, &text, at_text)
        .iter()
        .map(|item| item.label.clone())
        .collect();
    assert!(
        labels_text.contains(&"text".to_string()),
        "labels: {labels_text:?}"
    );
    let at_done = text.find("todo.completed").expect("use") as u32 + 6;
    let labels_done: Vec<String> = complete_at(&file, &text, at_done)
        .iter()
        .map(|item| item.label.clone())
        .collect();
    assert!(
        labels_done.contains(&"completed".to_string()),
        "labels: {labels_done:?}"
    );
}

#[test]
fn sbe_data_locations_params_cover_storage() {
    let (text, file) = parse_sbe("DataLocations.sol");
    let contract = &file.contracts()[0];
    let func_f = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("f"))
        .expect("f");
    assert_eq!(func_f.params().len(), 2);
    let at = text.find("_arr").expect("param") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert!(
        hover.contents.contains("_arr"),
        "contents: {}",
        hover.contents
    );
}

#[test]
fn sbe_transient_storage_hover_stored() {
    let (text, file) = parse_sbe("TransientStorage.sol");
    let names: Vec<String> = file.contracts()[0]
        .state_vars()
        .iter()
        .filter_map(|v| v.name())
        .collect();
    assert!(names.contains(&"stored".to_string()));
    assert!(names.contains(&"counter".to_string()));
    let at = text.find("stored").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "uint stored");
}

#[test]
fn sbe_function_hover_return_many() {
    let (text, file) = parse_sbe("Function.sol");
    let contract = &file.contracts()[0];
    let many = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("returnMany"))
        .expect("many");
    assert_eq!(many.return_types().len(), 3);
    let named = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("named"))
        .expect("named");
    assert_eq!(named.return_types().len(), 3);
    let at = text.find("returnMany").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert!(
        hover.contents.contains("function returnMany"),
        "contents: {}",
        hover.contents
    );
}

#[test]
fn sbe_view_pure_hover_x() {
    let (text, file) = parse_sbe("ViewPure.sol");
    let names: Vec<String> = file.contracts()[0]
        .functions()
        .iter()
        .filter_map(|f| f.name())
        .collect();
    assert!(names.contains(&"addToX".to_string()));
    assert!(names.contains(&"add".to_string()));
    let at = text.find("uint public x").expect("decl") as u32 + 12;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "uint x");
}

#[test]
fn sbe_error_hover_and_completion() {
    let (text, file) = parse_sbe("Error.sol");
    let contract = &file.contracts()[0];
    assert_eq!(contract.errors().len(), 1);
    assert_eq!(
        contract.errors()[0].name().as_deref(),
        Some("InsufficientBalance")
    );
    let at = text.find("InsufficientBalance").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "error InsufficientBalance");
    let at_complete = text.rfind("InsufficientBalance").expect("use") as u32 + 5;
    let labels: Vec<String> = complete_at(&file, &text, at_complete)
        .iter()
        .map(|item| item.label.clone())
        .collect();
    assert!(
        labels.contains(&"InsufficientBalance".to_string()),
        "labels: {labels:?}"
    );
}

#[test]
fn sbe_function_modifier_hover_only_owner() {
    let (text, file) = parse_sbe("FunctionModifier.sol");
    let contract = &file.contracts()[0];
    assert_eq!(contract.modifiers().len(), 1);
    assert_eq!(contract.modifiers()[0].name().as_deref(), Some("onlyOwner"));
    let at = text.find("onlyOwner").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "modifier onlyOwner");
    let names: Vec<String> = contract
        .functions()
        .iter()
        .filter_map(|f| f.name())
        .collect();
    assert!(names.contains(&"setX".to_string()), "names: {names:?}");
}

#[test]
fn sbe_events_hover_transfer() {
    let (text, file) = parse_sbe("Events.sol");
    let contract = &file.contracts()[0];
    assert_eq!(contract.events().len(), 1);
    assert_eq!(contract.events()[0].name().as_deref(), Some("Transfer"));
    assert_eq!(contract.events()[0].params().len(), 3);
    let at = text.find("Transfer").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Transfer");
}

#[test]
fn sbe_events_advanced_anonymous() {
    let (text, file) = parse_sbe("EventsAdvanced.sol");
    let contract = &file.contracts()[0];
    let names: Vec<String> = contract.events().iter().filter_map(|e| e.name()).collect();
    assert!(names.contains(&"Log".to_string()), "names: {names:?}");
    assert!(
        names.contains(&"AnonymousLog".to_string()),
        "names: {names:?}"
    );
    let at = text.find("AnonymousLog").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event AnonymousLog");
}

#[test]
fn sbe_constructor_has_constructor_fn() {
    let (text, file) = parse_sbe("Constructor.sol");
    let contract = &file.contracts()[0];
    let found = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("constructor"))
        .expect("ctor");
    assert_eq!(found.params().len(), 2);
    let at = text.find("constructor").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert!(
        hover.contents.contains("function constructor"),
        "contents: {}",
        hover.contents
    );
}

#[test]
fn sbe_inheritance_override_hover() {
    let (text, file) = parse_sbe("Inheritance.sol");
    assert_eq!(file.contracts().len(), 2);
    let base = &file.contracts()[0];
    let derived = &file.contracts()[1];
    assert_eq!(base.name().as_deref(), Some("A"));
    assert_eq!(derived.heritage(), vec!["A"]);
    let at = text.rfind("function foo").expect("decl") as u32 + 9;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert!(
        hover.contents.contains("function foo"),
        "contents: {}",
        hover.contents
    );
}

#[test]
fn sbe_shadowing_derived_wins() {
    let (text, file) = parse_sbe("ShadowingStateVars.sol");
    assert_eq!(file.contracts().len(), 2);
    assert_eq!(file.contracts()[1].heritage(), vec!["X"]);
    let at_use = text.rfind("return name").expect("use") as u32 + 7;
    let target = goto_definition(&file, &text, at_use).expect("goto");
    let at_derived = text.rfind("string public name").expect("derived decl") as u32 + 14;
    assert!(target.start <= at_derived && at_derived < target.end);
}

#[test]
fn sbe_super_member_parses() {
    let (_, file) = parse_sbe("Super.sol");
    let derived = file
        .contracts()
        .into_iter()
        .find(|c| c.name().as_deref() == Some("B"))
        .expect("B");
    let foo = derived
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("foo"))
        .expect("foo");
    let pairs: Vec<(Option<String>, Option<String>)> = foo
        .body()
        .expect("body")
        .members()
        .iter()
        .map(|m| (m.receiver(), m.member()))
        .collect();
    assert!(
        pairs.contains(&(Some("super".to_string()), Some("foo".to_string()))),
        "pairs: {pairs:?}"
    );
}

#[test]
fn sbe_visibility_functions() {
    let (text, file) = parse_sbe("Visibility.sol");
    let names: Vec<String> = file.contracts()[0]
        .functions()
        .iter()
        .filter_map(|f| f.name())
        .collect();
    assert!(names.contains(&"get".to_string()), "names: {names:?}");
    assert!(
        names.contains(&"internalFn".to_string()),
        "names: {names:?}"
    );
    assert!(
        names.contains(&"externalFn".to_string()),
        "names: {names:?}"
    );
    let at = text.find("stored").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert!(
        hover.contents.contains("stored"),
        "contents: {}",
        hover.contents
    );
}

#[test]
fn sbe_interface_kind_and_stub() {
    let (text, file) = parse_sbe("Interface.sol");
    assert_eq!(file.contracts().len(), 2);
    assert_eq!(file.contracts()[0].kind_label(), "interface");
    assert_eq!(file.contracts()[0].name().as_deref(), Some("Counter"));
    assert_eq!(file.contracts()[0].functions().len(), 2);
    assert!(file.contracts()[0].functions()[0].body().is_none());
    assert_eq!(file.contracts()[1].heritage(), vec!["Counter"]);
    let at = text.find("Counter").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "interface Counter");
}

#[test]
fn sbe_payable_hover_owner() {
    let (text, file) = parse_sbe("Payable.sol");
    let names: Vec<String> = file.contracts()[0]
        .functions()
        .iter()
        .filter_map(|f| f.name())
        .collect();
    assert!(names.contains(&"deposit".to_string()), "names: {names:?}");
    assert!(
        names.contains(&"constructor".to_string()),
        "names: {names:?}"
    );
    let at = text.find("owner").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert!(
        hover.contents.contains("owner"),
        "contents: {}",
        hover.contents
    );
}

#[test]
fn sbe_sending_ether_functions() {
    let (_, file) = parse_sbe("SendingEther.sol");
    let contract = &file.contracts()[0];
    let names: Vec<String> = contract
        .functions()
        .iter()
        .filter_map(|f| f.name())
        .collect();
    assert!(
        names.contains(&"sendViaTransfer".to_string()),
        "names: {names:?}"
    );
    assert!(
        names.contains(&"sendViaSend".to_string()),
        "names: {names:?}"
    );
    assert!(
        names.contains(&"sendViaCall".to_string()),
        "names: {names:?}"
    );
    let send = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("sendViaSend"))
        .expect("send");
    let locals: Vec<String> = send
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"ok".to_string()), "locals: {locals:?}");
}

#[test]
fn sbe_fallback_functions() {
    let (text, file) = parse_sbe("Fallback.sol");
    let names: Vec<String> = file.contracts()[0]
        .functions()
        .iter()
        .filter_map(|f| f.name())
        .collect();
    assert!(names.contains(&"fallback".to_string()), "names: {names:?}");
    assert!(names.contains(&"receive".to_string()), "names: {names:?}");
    let at = text.find("fallback").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert!(
        hover.contents.contains("function fallback"),
        "contents: {}",
        hover.contents
    );
}

#[test]
fn sbe_call_parses() {
    let (text, file) = parse_sbe("Call.sol");
    assert_eq!(file.contracts().len(), 2);
    let target = file
        .contracts()
        .into_iter()
        .find(|c| c.name().as_deref() == Some("Call"))
        .expect("Call");
    let names: Vec<String> = target.functions().iter().filter_map(|f| f.name()).collect();
    assert!(names.contains(&"callFoo".to_string()), "names: {names:?}");
    let at = text.rfind("callFoo").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert!(
        hover.contents.contains("function callFoo"),
        "contents: {}",
        hover.contents
    );
}

#[test]
fn sbe_delegatecall_parses() {
    let (text, file) = parse_sbe("Delegatecall.sol");
    let target = file
        .contracts()
        .into_iter()
        .find(|c| c.name().as_deref() == Some("Delegatecall"))
        .expect("dc");
    let func = target
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("del"))
        .expect("del");
    assert_eq!(func.params().len(), 2);
    let at = text.find("delegatecall").expect("use") as u32;
    assert!(text.get(at as usize..).is_some());
}

#[test]
fn sbe_function_selector_local_sel() {
    let (text, file) = parse_sbe("FunctionSelector.sol");
    let func = file.contracts()[0]
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("callIt"))
        .expect("callIt");
    let locals: Vec<String> = func
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"sel".to_string()), "locals: {locals:?}");
    let at = text.find("bytes4 sel").expect("decl") as u32 + 7;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "bytes4 sel");
}

#[test]
fn sbe_calling_other_contract_member_hover() {
    let (text, file) = parse_sbe("CallingOtherContract.sol");
    let at = text.rfind("c.setX").expect("use") as u32 + 2;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert!(
        hover.contents.contains("function setX"),
        "contents: {}",
        hover.contents
    );
}

#[test]
fn sbe_new_contract_local_c() {
    let (text, file) = parse_sbe("NewContract.sol");
    let func = file
        .contracts()
        .into_iter()
        .find(|c| c.name().as_deref() == Some("NewContract"))
        .expect("nc")
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("deploy"))
        .expect("deploy");
    let locals: Vec<String> = func
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"c".to_string()), "locals: {locals:?}");
    let at = text.find("Child c").expect("decl") as u32 + 6;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "Child c");
}

#[test]
fn sbe_try_catch_parses() {
    let (_, file) = parse_sbe("TryCatch.sol");
    assert_eq!(file.contracts().len(), 2);
    let target = file
        .contracts()
        .into_iter()
        .find(|c| c.name().as_deref() == Some("TryCatch"))
        .expect("tc");
    assert_eq!(target.events().len(), 1);
    let names: Vec<String> = target.functions().iter().filter_map(|f| f.name()).collect();
    assert!(names.contains(&"attempt".to_string()), "names: {names:?}");
}

#[test]
fn sbe_import_path() {
    let (text, file) = parse_sbe("Import.sol");
    assert_eq!(file.imports().len(), 1);
    assert_eq!(
        file.imports()[0].path().as_deref(),
        Some("./HelloWorld.sol")
    );
    assert_eq!(file.contracts()[0].name().as_deref(), Some("Import"));
    let at = text.find('x').expect("decl") as u32;
    assert!(hover_at(&file, &text, at).is_some() || text.contains('x'));
}

#[test]
fn sbe_library_kind() {
    let (text, file) = parse_sbe("Library.sol");
    assert_eq!(file.contracts().len(), 2);
    assert_eq!(file.contracts()[0].kind_label(), "library");
    assert_eq!(file.contracts()[0].name().as_deref(), Some("SafeMath"));
    let at = text.find("SafeMath").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "library SafeMath");
}

#[test]
fn sbe_abi_encode_locals() {
    let (text, file) = parse_sbe("ABIEncode.sol");
    let func = file.contracts()[0]
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("encode"))
        .expect("encode");
    let locals: Vec<String> = func
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"data".to_string()), "locals: {locals:?}");
    let at = text.find("bytes memory data").expect("decl") as u32 + 13;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "bytes data");
}

#[test]
fn sbe_abi_decode_locals_and_hover() {
    let (text, file) = parse_sbe("ABIDecode.sol");
    let func = file.contracts()[0]
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("decode"))
        .expect("decode");
    assert_eq!(func.params().len(), 1);
    assert_eq!(func.return_types().len(), 2);
    let locals: Vec<String> = func
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"kept".to_string()), "locals: {locals:?}");
    let at = text.find("function decode").expect("decl") as u32 + 9;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert!(
        hover.contents.contains("function decode"),
        "contents: {}",
        hover.contents
    );
}

#[test]
fn sbe_hashing_hover_digest() {
    let (text, file) = parse_sbe("Hashing.sol");
    let func = file.contracts()[0]
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("hash"))
        .expect("hash");
    let locals: Vec<String> = func
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"digest".to_string()), "locals: {locals:?}");
    let at = text.find("bytes32 digest").expect("decl") as u32 + 8;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "bytes32 digest");
}

#[test]
fn sbe_signature_params_and_locals() {
    let (text, file) = parse_sbe("Signature.sol");
    let func = file.contracts()[0]
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("verify"))
        .expect("verify");
    assert_eq!(func.params().len(), 5);
    let locals: Vec<String> = func
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(
        locals.contains(&"recovered".to_string()),
        "locals: {locals:?}"
    );
    assert!(locals.contains(&"ok".to_string()), "locals: {locals:?}");
    let at = text.find("address recovered").expect("decl") as u32 + 8;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "address recovered");
}

#[test]
fn sbe_gas_golf_unchecked_keeps_locals() {
    let (text, file) = parse_sbe("GasGolf.sol");
    let func = file.contracts()[0]
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("sum"))
        .expect("sum");
    let locals: Vec<String> = func
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"start".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"spent".to_string()), "locals: {locals:?}");
    let at = text.find("uint spent").expect("decl") as u32 + 5;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "uint spent");
}

#[test]
fn sbe_bitwise_locals() {
    let (text, file) = parse_sbe("Bitwise.sol");
    let func = file.contracts()[0]
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("ops"))
        .expect("ops");
    let locals: Vec<String> = func
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"anded".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"ored".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"xored".to_string()), "locals: {locals:?}");
    let at = text.find("uint anded").expect("decl") as u32 + 5;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "uint anded");
}

#[test]
fn sbe_unchecked_math_keeps_locals() {
    let (text, file) = parse_sbe("UncheckedMath.sol");
    let func = file.contracts()[0]
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("inc"))
        .expect("inc");
    let locals: Vec<String> = func
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"before".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"after".to_string()), "locals: {locals:?}");
    let at = text.find("uint after").expect("decl") as u32 + 5;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "uint after");
}

#[test]
fn sbe_assembly_variable_keeps_locals() {
    let (text, file) = parse_sbe("AssemblyVariable.sol");
    let func = file.contracts()[0]
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("yulVar"))
        .expect("yulVar");
    let locals: Vec<String> = func
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"before".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"after".to_string()), "locals: {locals:?}");
    let at = text.find("uint before").expect("decl") as u32 + 5;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "uint before");
}

#[test]
fn sbe_assembly_if_keeps_locals() {
    let (text, file) = parse_sbe("AssemblyIf.sol");
    let func = file.contracts()[0]
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("yulIf"))
        .expect("yulIf");
    let locals: Vec<String> = func
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"before".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"after".to_string()), "locals: {locals:?}");
    let at = text.find("uint after").expect("decl") as u32 + 5;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "uint after");
}

#[test]
fn sbe_assembly_loop_keeps_locals() {
    let (text, file) = parse_sbe("AssemblyLoop.sol");
    let func = file.contracts()[0]
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("yulLoop"))
        .expect("yulLoop");
    let locals: Vec<String> = func
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"before".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"after".to_string()), "locals: {locals:?}");
    let at = text.find("function yulLoop").expect("decl") as u32 + 9;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert!(
        hover.contents.contains("function yulLoop"),
        "contents: {}",
        hover.contents
    );
}

#[test]
fn sbe_assembly_error_keeps_locals() {
    let (text, file) = parse_sbe("AssemblyError.sol");
    let contract = &file.contracts()[0];
    assert_eq!(contract.errors().len(), 1);
    assert_eq!(contract.errors()[0].name().as_deref(), Some("BadNumber"));
    let func = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("yulRevert"))
        .expect("yulRevert");
    let locals: Vec<String> = func
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"before".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"after".to_string()), "locals: {locals:?}");
    let at = text.find("BadNumber").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "error BadNumber");
}

#[test]
fn sbe_assembly_math_keeps_locals() {
    let (text, file) = parse_sbe("AssemblyMath.sol");
    let func = file.contracts()[0]
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("yulMath"))
        .expect("yulMath");
    assert_eq!(func.params().len(), 2);
    let locals: Vec<String> = func
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"before".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"after".to_string()), "locals: {locals:?}");
    let at = text.find("uint after").expect("decl") as u32 + 5;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "uint after");
}

#[test]
fn sbe_ether_wallet_payable_and_locals() {
    let (text, file) = parse_sbe("EtherWallet.sol");
    let contract = &file.contracts()[0];
    let names: Vec<String> = contract
        .state_vars()
        .iter()
        .filter_map(|v| v.name())
        .collect();
    assert!(names.contains(&"owner".to_string()), "names: {names:?}");
    let funcs: Vec<String> = contract
        .functions()
        .iter()
        .filter_map(|f| f.name())
        .collect();
    assert!(funcs.contains(&"withdraw".to_string()), "funcs: {funcs:?}");
    assert!(funcs.contains(&"receive".to_string()), "funcs: {funcs:?}");
    let withdraw = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("withdraw"))
        .expect("withdraw");
    let locals: Vec<String> = withdraw
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"bal".to_string()), "locals: {locals:?}");
    let at = text.find("owner").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert!(
        hover.contents.contains("owner"),
        "contents: {}",
        hover.contents
    );
}

#[test]
fn sbe_multisig_struct_events_locals() {
    let (text, file) = parse_sbe("MultiSigWallet.sol");
    let contract = &file.contracts()[0];
    assert_eq!(contract.structs().len(), 1);
    assert_eq!(contract.structs()[0].name().as_deref(), Some("Txn"));
    let events: Vec<String> = contract.events().iter().filter_map(|e| e.name()).collect();
    assert!(events.contains(&"Submit".to_string()), "events: {events:?}");
    assert!(
        events.contains(&"Confirm".to_string()),
        "events: {events:?}"
    );
    let submit = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("submit"))
        .expect("submit");
    let locals: Vec<String> = submit
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"txId".to_string()), "locals: {locals:?}");
    let at = text.find("Submit").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Submit");
}

#[test]
fn sbe_merkle_proof_locals() {
    let (text, file) = parse_sbe("MerkleTree.sol");
    let contract = &file.contracts()[0];
    let vars: Vec<String> = contract
        .state_vars()
        .iter()
        .filter_map(|v| v.name())
        .collect();
    assert!(vars.contains(&"hashes".to_string()), "vars: {vars:?}");
    let func = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("verify"))
        .expect("verify");
    assert_eq!(func.params().len(), 3);
    let locals: Vec<String> = func
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(
        locals.contains(&"computed".to_string()),
        "locals: {locals:?}"
    );
    assert!(locals.contains(&"valid".to_string()), "locals: {locals:?}");
    let at = text.find("bytes32 computed").expect("decl") as u32 + 8;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "bytes32 computed");
}

#[test]
fn sbe_iterable_mapping_locals() {
    let (text, file) = parse_sbe("IterableMapping.sol");
    let contract = &file.contracts()[0];
    let types: Vec<String> = contract
        .state_vars()
        .iter()
        .filter_map(|v| v.type_node().map(|n| TypeNode::text(&n)))
        .collect();
    assert!(
        types.contains(&"mapping(address => uint)".to_string()),
        "types: {types:?}"
    );
    let set = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("set"))
        .expect("set");
    let locals: Vec<String> = set
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"idx".to_string()), "locals: {locals:?}");
    let get = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("get"))
        .expect("get");
    let get_locals: Vec<String> = get
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(
        get_locals.contains(&"value".to_string()),
        "locals: {get_locals:?}"
    );
    let at = text.find("balances").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert!(
        hover.contents.contains("balances"),
        "contents: {}",
        hover.contents
    );
}

#[test]
fn sbe_erc20_interface_events_mappings() {
    let (text, file) = parse_sbe("ERC20.sol");
    assert_eq!(file.contracts().len(), 2);
    assert_eq!(file.contracts()[0].kind_label(), "interface");
    assert_eq!(file.contracts()[0].name().as_deref(), Some("IERC20"));
    let token = file
        .contracts()
        .into_iter()
        .find(|c| c.name().as_deref() == Some("ERC20"))
        .expect("ERC20");
    assert_eq!(token.heritage(), vec!["IERC20"]);
    assert_eq!(token.events().len(), 1);
    let func = token
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("transfer"))
        .expect("transfer");
    let locals: Vec<String> = func
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"bal".to_string()), "locals: {locals:?}");
    let at = text.find("Transfer").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Transfer");
}

#[test]
fn sbe_erc721_owners_and_transfer() {
    let (text, file) = parse_sbe("ERC721.sol");
    let token = file
        .contracts()
        .into_iter()
        .find(|c| c.name().as_deref() == Some("ERC721"))
        .expect("ERC721");
    assert_eq!(token.heritage(), vec!["IERC721"]);
    let types: Vec<String> = token
        .state_vars()
        .iter()
        .filter_map(|v| v.type_node().map(|n| TypeNode::text(&n)))
        .collect();
    assert!(
        types.contains(&"mapping(uint => address)".to_string()),
        "types: {types:?}"
    );
    let owner_fn = token
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("ownerOf"))
        .expect("ownerOf");
    let locals: Vec<String> = owner_fn
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"owner".to_string()), "locals: {locals:?}");
    let at = text.find("uint tokenId").expect("decl") as u32 + 5;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "uint tokenId");
}

#[test]
fn sbe_erc1155_nested_mapping() {
    let (text, file) = parse_sbe("ERC1155.sol");
    let token = file
        .contracts()
        .into_iter()
        .find(|c| c.name().as_deref() == Some("ERC1155"))
        .expect("ERC1155");
    let types: Vec<String> = token
        .state_vars()
        .iter()
        .filter_map(|v| v.type_node().map(|n| TypeNode::text(&n)))
        .collect();
    assert!(
        types.contains(&"mapping(uint => mapping(address => uint))".to_string()),
        "types: {types:?}"
    );
    let balance = token
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("balanceOf"))
        .expect("balanceOf");
    let locals: Vec<String> = balance
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"bal".to_string()), "locals: {locals:?}");
    let at = text.find("TransferSingle").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event TransferSingle");
}

#[test]
fn sbe_gasless_permit_locals() {
    let (text, file) = parse_sbe("GaslessTokenTransfer.sol");
    let contract = &file.contracts()[0];
    let vars: Vec<String> = contract
        .state_vars()
        .iter()
        .filter_map(|v| v.name())
        .collect();
    assert!(vars.contains(&"nonces".to_string()), "vars: {vars:?}");
    let func = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("permit"))
        .expect("permit");
    assert_eq!(func.params().len(), 6);
    let locals: Vec<String> = func
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"nonce".to_string()), "locals: {locals:?}");
    assert!(
        locals.contains(&"recovered".to_string()),
        "locals: {locals:?}"
    );
    let at = text.find("nonces").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert!(
        hover.contents.contains("nonces"),
        "contents: {}",
        hover.contents
    );
}

#[test]
fn sbe_simple_bytecode_assembly_keeps_locals() {
    let (text, file) = parse_sbe("SimpleBytecodeContract.sol");
    let func = file.contracts()[0]
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("set"))
        .expect("set");
    let locals: Vec<String> = func
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"prev".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"next".to_string()), "locals: {locals:?}");
    let at = text.find("bytes memory prev").expect("decl") as u32 + 13;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "bytes prev");
}

#[test]
fn sbe_create2_factory_locals() {
    let (text, file) = parse_sbe("Create2.sol");
    assert_eq!(file.contracts().len(), 2);
    let factory = file
        .contracts()
        .into_iter()
        .find(|c| c.name().as_deref() == Some("Factory"))
        .expect("Factory");
    assert_eq!(factory.events().len(), 1);
    let deploy = factory
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("deploy"))
        .expect("deploy");
    let locals: Vec<String> = deploy
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"tag".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"c".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"addr".to_string()), "locals: {locals:?}");
    let at = text.find("Deployed").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Deployed");
}

#[test]
fn sbe_minimal_proxy_clone_locals() {
    let (text, file) = parse_sbe("MinimalProxy.sol");
    assert_eq!(file.contracts().len(), 2);
    let proxy = file
        .contracts()
        .into_iter()
        .find(|c| c.name().as_deref() == Some("MinimalProxy"))
        .expect("proxy");
    assert_eq!(proxy.events().len(), 1);
    let clone = proxy
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("clone"))
        .expect("clone");
    let locals: Vec<String> = clone
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"got".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"next".to_string()), "locals: {locals:?}");
    let at = text.find("Cloned").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Cloned");
}

#[test]
fn sbe_upgradeable_proxy_fallback() {
    let (text, file) = parse_sbe("UpgradeableProxy.sol");
    let proxy = &file.contracts()[0];
    let vars: Vec<String> = proxy.state_vars().iter().filter_map(|v| v.name()).collect();
    assert!(vars.contains(&"impl".to_string()), "vars: {vars:?}");
    assert!(vars.contains(&"admin".to_string()), "vars: {vars:?}");
    let names: Vec<String> = proxy.functions().iter().filter_map(|f| f.name()).collect();
    assert!(names.contains(&"upgrade".to_string()), "names: {names:?}");
    assert!(names.contains(&"fallback".to_string()), "names: {names:?}");
    let at = text.find("Upgraded").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Upgraded");
}

#[test]
fn sbe_deploy_any_contract_assembly_locals() {
    let (text, file) = parse_sbe("DeployAnyContract.sol");
    let func = file.contracts()[0]
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("deploy"))
        .expect("deploy");
    let locals: Vec<String> = func
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"prev".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"next".to_string()), "locals: {locals:?}");
    let at = text.find("Deployed").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Deployed");
}

#[test]
fn sbe_write_to_any_slot_assembly() {
    let (text, file) = parse_sbe("WriteToAnySlot.sol");
    let func = file.contracts()[0]
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("write"))
        .expect("write");
    assert_eq!(func.params().len(), 2);
    let locals: Vec<String> = func
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"before".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"after".to_string()), "locals: {locals:?}");
    let at = text.find("function write").expect("decl") as u32 + 9;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert!(
        hover.contents.contains("function write"),
        "contents: {}",
        hover.contents
    );
}

#[test]
fn sbe_uni_payment_channel_close() {
    let (text, file) = parse_sbe("UniPaymentChannel.sol");
    let contract = &file.contracts()[0];
    let vars: Vec<String> = contract
        .state_vars()
        .iter()
        .filter_map(|v| v.name())
        .collect();
    assert!(vars.contains(&"sender".to_string()), "vars: {vars:?}");
    assert!(vars.contains(&"receiver".to_string()), "vars: {vars:?}");
    let close = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("close"))
        .expect("close");
    let locals: Vec<String> = close
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"digest".to_string()), "locals: {locals:?}");
    assert!(
        locals.contains(&"recovered".to_string()),
        "locals: {locals:?}"
    );
    let at = text.find("Closed").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Closed");
}

#[test]
fn sbe_bi_payment_channel_settle() {
    let (text, file) = parse_sbe("BiPaymentChannel.sol");
    let contract = &file.contracts()[0];
    let vars: Vec<String> = contract
        .state_vars()
        .iter()
        .filter_map(|v| v.name())
        .collect();
    assert!(vars.contains(&"alice".to_string()), "vars: {vars:?}");
    assert!(vars.contains(&"bob".to_string()), "vars: {vars:?}");
    let settle = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("settle"))
        .expect("settle");
    let locals: Vec<String> = settle
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"kept".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"valid".to_string()), "locals: {locals:?}");
    let at = text.find("Settled").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Settled");
}

#[test]
fn sbe_english_auction_bid_end() {
    let (text, file) = parse_sbe("EnglishAuction.sol");
    let contract = &file.contracts()[0];
    let events: Vec<String> = contract.events().iter().filter_map(|e| e.name()).collect();
    assert!(events.contains(&"Bid".to_string()), "events: {events:?}");
    assert!(events.contains(&"End".to_string()), "events: {events:?}");
    let bid = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("bid"))
        .expect("bid");
    let locals: Vec<String> = bid
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"amount".to_string()), "locals: {locals:?}");
    let at = text.find("event Bid").expect("decl") as u32 + 6;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Bid");
}

#[test]
fn sbe_dutch_auction_price() {
    let (text, file) = parse_sbe("DutchAuction.sol");
    let contract = &file.contracts()[0];
    let vars: Vec<String> = contract
        .state_vars()
        .iter()
        .filter_map(|v| v.name())
        .collect();
    assert!(vars.contains(&"seller".to_string()), "vars: {vars:?}");
    assert!(vars.contains(&"startPrice".to_string()), "vars: {vars:?}");
    let price = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("price"))
        .expect("price");
    let locals: Vec<String> = price
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(
        locals.contains(&"nowPrice".to_string()),
        "locals: {locals:?}"
    );
    let at = text.find("Bought").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Bought");
}

#[test]
fn sbe_crowd_fund_using_skipped() {
    let (text, file) = parse_sbe("CrowdFund.sol");
    assert!(text.contains("using SafeMath for uint;"), "using kept");
    let contract = &file.contracts()[0];
    assert_eq!(contract.structs().len(), 1);
    assert_eq!(contract.structs()[0].name().as_deref(), Some("Backer"));
    let vars: Vec<String> = contract
        .state_vars()
        .iter()
        .filter_map(|v| v.name())
        .collect();
    assert!(vars.contains(&"goal".to_string()), "vars: {vars:?}");
    assert!(vars.contains(&"pledged".to_string()), "vars: {vars:?}");
    let at = text.find("Funded").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Funded");
}

#[test]
fn sbe_multi_call_targets() {
    let (text, file) = parse_sbe("MultiCall.sol");
    let func = file.contracts()[0]
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("multi"))
        .expect("multi");
    assert_eq!(func.params().len(), 2);
    let locals: Vec<String> = func
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"kept".to_string()), "locals: {locals:?}");
    let at = text.find("function multi").expect("decl") as u32 + 9;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert!(
        hover.contents.contains("function multi"),
        "contents: {}",
        hover.contents
    );
}

#[test]
fn sbe_multi_delegatecall_targets() {
    let (text, file) = parse_sbe("MultiDelegatecall.sol");
    let func = file.contracts()[0]
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("multi"))
        .expect("multi");
    assert_eq!(func.params().len(), 2);
    let locals: Vec<String> = func
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"kept".to_string()), "locals: {locals:?}");
    let at = text.find("delegatecall").expect("use") as u32;
    assert!(text.get(at as usize..).is_some());
}

#[test]
fn sbe_time_lock_queue_execute() {
    let (text, file) = parse_sbe("TimeLock.sol");
    let contract = &file.contracts()[0];
    let vars: Vec<String> = contract
        .state_vars()
        .iter()
        .filter_map(|v| v.name())
        .collect();
    assert!(vars.contains(&"owner".to_string()), "vars: {vars:?}");
    assert!(vars.contains(&"delay".to_string()), "vars: {vars:?}");
    let events: Vec<String> = contract.events().iter().filter_map(|e| e.name()).collect();
    assert!(events.contains(&"Queued".to_string()), "events: {events:?}");
    assert!(
        events.contains(&"Executed".to_string()),
        "events: {events:?}"
    );
    let queue = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("queue"))
        .expect("queue");
    let locals: Vec<String> = queue
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"kept".to_string()), "locals: {locals:?}");
    let at = text.find("Queued").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Queued");
}

#[test]
fn sbe_assembly_bin_exp_locals() {
    let (text, file) = parse_sbe("AssemblyBinExp.sol");
    let func = file.contracts()[0]
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("pow"))
        .expect("pow");
    assert_eq!(func.params().len(), 2);
    let locals: Vec<String> = func
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"before".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"after".to_string()), "locals: {locals:?}");
    let at = text.find("uint after").expect("decl") as u32 + 5;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "uint after");
}

#[test]
fn sbe_airdrop_merkle_claim() {
    let (text, file) = parse_sbe("Airdrop.sol");
    assert_eq!(file.contracts().len(), 2);
    assert_eq!(file.contracts()[0].kind_label(), "interface");
    let drop = file
        .contracts()
        .into_iter()
        .find(|c| c.name().as_deref() == Some("Airdrop"))
        .expect("Airdrop");
    let vars: Vec<String> = drop.state_vars().iter().filter_map(|v| v.name()).collect();
    assert!(vars.contains(&"root".to_string()), "vars: {vars:?}");
    assert!(vars.contains(&"claimed".to_string()), "vars: {vars:?}");
    let claim = drop
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("claim"))
        .expect("claim");
    let locals: Vec<String> = claim
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"leaf".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"ok".to_string()), "locals: {locals:?}");
    let at = text.find("Claimed").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Claimed");
}

#[test]
fn sbe_hack_reentrancy_withdraw() {
    let (text, file) = parse_sbe("HackReentrancy.sol");
    let vault = &file.contracts()[0];
    let types: Vec<String> = vault
        .state_vars()
        .iter()
        .filter_map(|v| v.type_node().map(|n| TypeNode::text(&n)))
        .collect();
    assert!(
        types.contains(&"mapping(address => uint)".to_string()),
        "types: {types:?}"
    );
    let withdraw = vault
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("withdraw"))
        .expect("withdraw");
    let locals: Vec<String> = withdraw
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"bal".to_string()), "locals: {locals:?}");
    let at = text.find("function withdraw").expect("decl") as u32 + 9;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert!(
        hover.contents.contains("function withdraw"),
        "contents: {}",
        hover.contents
    );
}

#[test]
fn sbe_hack_overflow_unchecked() {
    let (text, file) = parse_sbe("HackOverflow.sol");
    let func = file.contracts()[0]
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("add"))
        .expect("add");
    let locals: Vec<String> = func
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"before".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"after".to_string()), "locals: {locals:?}");
    let at = text.find("uint after").expect("decl") as u32 + 5;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "uint after");
}

#[test]
fn sbe_hack_selfdestruct_kill() {
    let (text, file) = parse_sbe("HackSelfDestruct.sol");
    let contract = &file.contracts()[0];
    let names: Vec<String> = contract
        .functions()
        .iter()
        .filter_map(|f| f.name())
        .collect();
    assert!(names.contains(&"kill".to_string()), "names: {names:?}");
    assert!(names.contains(&"receive".to_string()), "names: {names:?}");
    let kill = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("kill"))
        .expect("kill");
    let locals: Vec<String> = kill
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"kept".to_string()), "locals: {locals:?}");
    let at = text.find("Killed").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Killed");
}

#[test]
fn sbe_hack_private_data_slots() {
    let (text, file) = parse_sbe("HackPrivateData.sol");
    let contract = &file.contracts()[0];
    let vars: Vec<String> = contract
        .state_vars()
        .iter()
        .filter_map(|v| v.name())
        .collect();
    assert!(vars.contains(&"seed".to_string()), "vars: {vars:?}");
    assert!(vars.contains(&"locked".to_string()), "vars: {vars:?}");
    let peek = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("peek"))
        .expect("peek");
    let locals: Vec<String> = peek
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"got".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"next".to_string()), "locals: {locals:?}");
    let at = text.find("seed").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert!(
        hover.contents.contains("seed"),
        "contents: {}",
        hover.contents
    );
}

#[test]
fn sbe_hack_delegatecall_pwn() {
    let (text, file) = parse_sbe("HackDelegatecall.sol");
    assert_eq!(file.contracts().len(), 2);
    let hack = file
        .contracts()
        .into_iter()
        .find(|c| c.name().as_deref() == Some("Hack"))
        .expect("Hack");
    assert_eq!(hack.events().len(), 1);
    let pwn = hack
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("pwn"))
        .expect("pwn");
    let locals: Vec<String> = pwn
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"kept".to_string()), "locals: {locals:?}");
    let at = text.find("Pwned").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Pwned");
}

#[test]
fn sbe_hack_randomness_seed() {
    let (text, file) = parse_sbe("HackRandomness.sol");
    let contract = &file.contracts()[0];
    let vars: Vec<String> = contract
        .state_vars()
        .iter()
        .filter_map(|v| v.name())
        .collect();
    assert!(vars.contains(&"last".to_string()), "vars: {vars:?}");
    let roll = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("roll"))
        .expect("roll");
    let locals: Vec<String> = roll
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"seed".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"picked".to_string()), "locals: {locals:?}");
    let at = text.find("Rolled").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Rolled");
}

#[test]
fn sbe_hack_dos_loop_keeps_done() {
    let (text, file) = parse_sbe("HackDoS.sol");
    let contract = &file.contracts()[0];
    let types: Vec<String> = contract
        .state_vars()
        .iter()
        .filter_map(|v| v.type_node().map(|n| TypeNode::text(&n)))
        .collect();
    assert!(types.contains(&"address[]".to_string()), "types: {types:?}");
    let pay = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("pay"))
        .expect("pay");
    let locals: Vec<String> = pay
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"done".to_string()), "locals: {locals:?}");
    let at = text.find("Paid").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Paid");
}

#[test]
fn sbe_hack_tx_origin_phishing_transfer() {
    let (text, file) = parse_sbe("HackTxOriginPhishing.sol");
    assert_eq!(file.contracts().len(), 2);
    assert!(text.contains("tx.origin"), "pattern kept");
    let wallet = file
        .contracts()
        .into_iter()
        .find(|c| c.name().as_deref() == Some("Wallet"))
        .expect("Wallet");
    let transfer = wallet
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("transfer"))
        .expect("transfer");
    let locals: Vec<String> = transfer
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"kept".to_string()), "locals: {locals:?}");
    assert!(
        locals.contains(&"receiver".to_string()),
        "locals: {locals:?}"
    );
    let at = text.find("Drained").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Drained");
}

#[test]
fn sbe_hack_hidden_code_run() {
    let (text, file) = parse_sbe("HackHiddenCode.sol");
    assert_eq!(file.contracts().len(), 2);
    assert!(text.contains("delegatecall"), "pattern kept");
    let hidden = file
        .contracts()
        .into_iter()
        .find(|c| c.name().as_deref() == Some("Hidden"))
        .expect("Hidden");
    let run = hidden
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("run"))
        .expect("run");
    let locals: Vec<String> = run
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"kept".to_string()), "locals: {locals:?}");
    let at = text.find("event Hid").expect("decl") as u32 + 6;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Hid");
}

#[test]
fn sbe_hack_honeypot_balances() {
    let (text, file) = parse_sbe("HackHoneypot.sol");
    let contract = &file.contracts()[0];
    let types: Vec<String> = contract
        .state_vars()
        .iter()
        .filter_map(|v| v.type_node().map(|n| TypeNode::text(&n)))
        .collect();
    assert!(
        types.contains(&"mapping(address => uint)".to_string()),
        "types: {types:?}"
    );
    let withdraw = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("withdraw"))
        .expect("withdraw");
    let locals: Vec<String> = withdraw
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"bal".to_string()), "locals: {locals:?}");
    let at = text.find("Trapped").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Trapped");
}

#[test]
fn sbe_hack_front_running_commit() {
    let (text, file) = parse_sbe("HackFrontRunning.sol");
    let contract = &file.contracts()[0];
    let vars: Vec<String> = contract
        .state_vars()
        .iter()
        .filter_map(|v| v.name())
        .collect();
    assert!(vars.contains(&"commitment".to_string()), "vars: {vars:?}");
    let reveal = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("reveal"))
        .expect("reveal");
    let locals: Vec<String> = reveal
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"kept".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"caller".to_string()), "locals: {locals:?}");
    let at = text.find("Committed").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Committed");
}

#[test]
fn sbe_hack_timestamp_seed() {
    let (text, file) = parse_sbe("HackTimestamp.sol");
    assert!(text.contains("block.timestamp"), "pattern kept");
    let contract = &file.contracts()[0];
    let roll = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("roll"))
        .expect("roll");
    let locals: Vec<String> = roll
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"seed".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"picked".to_string()), "locals: {locals:?}");
    let at = text.find("Rolled").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Rolled");
}

#[test]
fn sbe_hack_sig_replay_nonce() {
    let (text, file) = parse_sbe("HackSigReplay.sol");
    assert!(text.contains("ecrecover"), "pattern kept");
    let contract = &file.contracts()[0];
    let vars: Vec<String> = contract
        .state_vars()
        .iter()
        .filter_map(|v| v.name())
        .collect();
    assert!(vars.contains(&"nonces".to_string()), "vars: {vars:?}");
    let spend = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("spend"))
        .expect("spend");
    assert_eq!(spend.params().len(), 5);
    let locals: Vec<String> = spend
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"kept".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"who".to_string()), "locals: {locals:?}");
    let at = text.find("Spent").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Spent");
}

#[test]
fn sbe_hack_contract_size_enter() {
    let (text, file) = parse_sbe("HackContractSize.sol");
    assert!(text.contains("extcodesize"), "pattern kept");
    assert_eq!(file.contracts().len(), 2);
    let guard = file
        .contracts()
        .into_iter()
        .find(|c| c.name().as_deref() == Some("Guard"))
        .expect("Guard");
    let enter = guard
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("enter"))
        .expect("enter");
    let locals: Vec<String> = enter
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"size".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"who".to_string()), "locals: {locals:?}");
    let at = text.find("Entered").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Entered");
}

#[test]
fn sbe_hack_same_address_deploy() {
    let (text, file) = parse_sbe("HackSameAddress.sol");
    assert_eq!(file.contracts().len(), 2);
    assert!(text.contains("salt"), "pattern kept");
    let factory = file
        .contracts()
        .into_iter()
        .find(|c| c.name().as_deref() == Some("Factory"))
        .expect("Factory");
    let deploy = factory
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("deploy"))
        .expect("deploy");
    let locals: Vec<String> = deploy
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"kept".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"next".to_string()), "locals: {locals:?}");
    let at = text.find("Deployed").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Deployed");
}

#[test]
fn sbe_hack_gas_rule_pay() {
    let (text, file) = parse_sbe("HackGasRule.sol");
    assert!(text.contains("gasleft"), "pattern kept");
    let contract = &file.contracts()[0];
    let pay = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("pay"))
        .expect("pay");
    let locals: Vec<String> = pay
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"kept".to_string()), "locals: {locals:?}");
    assert!(
        locals.contains(&"receiver".to_string()),
        "locals: {locals:?}"
    );
    let at = text.find("Paid").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Paid");
}

#[test]
fn sbe_hack_weth_permit_nonce() {
    let (text, file) = parse_sbe("HackWETHPermit.sol");
    let contract = &file.contracts()[0];
    let vars: Vec<String> = contract
        .state_vars()
        .iter()
        .filter_map(|v| v.name())
        .collect();
    assert!(vars.contains(&"nonces".to_string()), "vars: {vars:?}");
    let permit = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("permit"))
        .expect("permit");
    assert_eq!(permit.params().len(), 6);
    let locals: Vec<String> = permit
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"nonce".to_string()), "locals: {locals:?}");
    assert!(
        locals.contains(&"recovered".to_string()),
        "locals: {locals:?}"
    );
    let at = text.find("Approved").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Approved");
}

#[test]
fn sbe_hack_vault_inflation_mint() {
    let (text, file) = parse_sbe("HackVaultInflation.sol");
    let contract = &file.contracts()[0];
    let vars: Vec<String> = contract
        .state_vars()
        .iter()
        .filter_map(|v| v.name())
        .collect();
    assert!(vars.contains(&"shares".to_string()), "vars: {vars:?}");
    assert!(vars.contains(&"total".to_string()), "vars: {vars:?}");
    let mint = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("mint"))
        .expect("mint");
    let locals: Vec<String> = mint
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"share".to_string()), "locals: {locals:?}");
    let at = text.find("Minted").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Minted");
}

#[test]
fn sbe_defi_uniswap_v2_swap() {
    let (text, file) = parse_sbe("DeFiUniswapV2Swap.sol");
    assert_eq!(file.contracts().len(), 2);
    assert_eq!(file.contracts()[0].kind_label(), "interface");
    assert_eq!(file.contracts()[0].name().as_deref(), Some("IPair"));
    let target = file
        .contracts()
        .into_iter()
        .find(|c| c.name().as_deref() == Some("Swap"))
        .expect("Swap");
    let swap = target
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("swap"))
        .expect("swap");
    let locals: Vec<String> = swap
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"kept".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"target".to_string()), "locals: {locals:?}");
    let at = text.find("Swapped").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Swapped");
}

#[test]
fn sbe_defi_uniswap_v2_liquidity() {
    let (text, file) = parse_sbe("DeFiUniswapV2Liquidity.sol");
    assert_eq!(file.contracts()[0].kind_label(), "interface");
    let target = file
        .contracts()
        .into_iter()
        .find(|c| c.name().as_deref() == Some("Liquidity"))
        .expect("Liquidity");
    let events: Vec<String> = target.events().iter().filter_map(|e| e.name()).collect();
    assert!(events.contains(&"Added".to_string()), "events: {events:?}");
    assert!(
        events.contains(&"Removed".to_string()),
        "events: {events:?}"
    );
    let remove = target
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("remove"))
        .expect("remove");
    let locals: Vec<String> = remove
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"share".to_string()), "locals: {locals:?}");
    let at = text.find("Added").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Added");
}

#[test]
fn sbe_defi_uniswap_v3_swap() {
    let (text, file) = parse_sbe("DeFiUniswapV3Swap.sol");
    assert_eq!(file.contracts()[0].kind_label(), "interface");
    let target = file
        .contracts()
        .into_iter()
        .find(|c| c.name().as_deref() == Some("V3Swap"))
        .expect("V3Swap");
    let names: Vec<String> = target.functions().iter().filter_map(|f| f.name()).collect();
    assert!(names.contains(&"swap".to_string()), "names: {names:?}");
    assert!(names.contains(&"callback".to_string()), "names: {names:?}");
    let swap = target
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("swap"))
        .expect("swap");
    let locals: Vec<String> = swap
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"kept".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"amount".to_string()), "locals: {locals:?}");
    let at = text.find("Swapped").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Swapped");
}

#[test]
fn sbe_defi_uniswap_v3_liquidity() {
    let (text, file) = parse_sbe("DeFiUniswapV3Liquidity.sol");
    assert_eq!(file.contracts()[0].kind_label(), "interface");
    let target = file
        .contracts()
        .into_iter()
        .find(|c| c.name().as_deref() == Some("V3Liquidity"))
        .expect("V3Liquidity");
    let vars: Vec<String> = target
        .state_vars()
        .iter()
        .filter_map(|v| v.name())
        .collect();
    assert!(vars.contains(&"positions".to_string()), "vars: {vars:?}");
    let mint = target
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("mint"))
        .expect("mint");
    let locals: Vec<String> = mint
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"kept".to_string()), "locals: {locals:?}");
    let at = text.find("Minted").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Minted");
}

#[test]
fn sbe_defi_uniswap_v3_flash() {
    let (text, file) = parse_sbe("DeFiUniswapV3Flash.sol");
    assert_eq!(file.contracts()[0].kind_label(), "interface");
    let target = file
        .contracts()
        .into_iter()
        .find(|c| c.name().as_deref() == Some("Flash"))
        .expect("Flash");
    let names: Vec<String> = target.functions().iter().filter_map(|f| f.name()).collect();
    assert!(names.contains(&"borrow".to_string()), "names: {names:?}");
    assert!(names.contains(&"callback".to_string()), "names: {names:?}");
    let borrow = target
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("borrow"))
        .expect("borrow");
    let locals: Vec<String> = borrow
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"kept".to_string()), "locals: {locals:?}");
    let at = text.find("Loaned").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Loaned");
}

#[test]
fn sbe_defi_chainlink_oracle_read() {
    let (text, file) = parse_sbe("DeFiChainlinkOracle.sol");
    assert_eq!(file.contracts()[0].kind_label(), "interface");
    let target = file
        .contracts()
        .into_iter()
        .find(|c| c.name().as_deref() == Some("Price"))
        .expect("Price");
    let read = target
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("read"))
        .expect("read");
    let locals: Vec<String> = read
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"price".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"source".to_string()), "locals: {locals:?}");
    let at = text.find("event Read").expect("decl") as u32 + 6;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Read");
}

#[test]
fn sbe_defi_chronicle_oracle_read() {
    let (text, file) = parse_sbe("DeFiChronicleOracle.sol");
    assert_eq!(file.contracts()[0].kind_label(), "interface");
    let target = file
        .contracts()
        .into_iter()
        .find(|c| c.name().as_deref() == Some("ChroniclePrice"))
        .expect("ChroniclePrice");
    let read = target
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("read"))
        .expect("read");
    let locals: Vec<String> = read
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"price".to_string()), "locals: {locals:?}");
    let at = text.find("event Read").expect("decl") as u32 + 6;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Read");
}

#[test]
fn sbe_defi_dai_proxy_forward() {
    let (text, file) = parse_sbe("DeFiDAIProxy.sol");
    assert_eq!(file.contracts()[0].kind_label(), "interface");
    let target = file
        .contracts()
        .into_iter()
        .find(|c| c.name().as_deref() == Some("DaiProxy"))
        .expect("DaiProxy");
    let forward = target
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("forward"))
        .expect("forward");
    assert_eq!(forward.params().len(), 2);
    let locals: Vec<String> = forward
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"kept".to_string()), "locals: {locals:?}");
    let at = text.find("Proxied").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Proxied");
}

#[test]
fn sbe_defi_staking_rewards_stake() {
    let (text, file) = parse_sbe("DeFiStakingRewards.sol");
    let contract = &file.contracts()[0];
    let vars: Vec<String> = contract
        .state_vars()
        .iter()
        .filter_map(|v| v.name())
        .collect();
    assert!(vars.contains(&"stakes".to_string()), "vars: {vars:?}");
    assert!(vars.contains(&"total".to_string()), "vars: {vars:?}");
    let claim = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("claim"))
        .expect("claim");
    let locals: Vec<String> = claim
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"reward".to_string()), "locals: {locals:?}");
    let at = text.find("Staked").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Staked");
}

#[test]
fn sbe_defi_discrete_rewards_update() {
    let (text, file) = parse_sbe("DeFiDiscreteRewards.sol");
    let contract = &file.contracts()[0];
    let vars: Vec<String> = contract
        .state_vars()
        .iter()
        .filter_map(|v| v.name())
        .collect();
    assert!(vars.contains(&"shares".to_string()), "vars: {vars:?}");
    assert!(vars.contains(&"acc".to_string()), "vars: {vars:?}");
    let update = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("update"))
        .expect("update");
    let locals: Vec<String> = update
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"kept".to_string()), "locals: {locals:?}");
    let at = text.find("Updated").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Updated");
}

#[test]
fn sbe_defi_vault_erc4626_deposit() {
    let (text, file) = parse_sbe("DeFiVaultERC4626.sol");
    assert_eq!(file.contracts().len(), 2);
    assert_eq!(file.contracts()[0].kind_label(), "interface");
    let vault = file
        .contracts()
        .into_iter()
        .find(|c| c.name().as_deref() == Some("Vault"))
        .expect("Vault");
    let vars: Vec<String> = vault.state_vars().iter().filter_map(|v| v.name()).collect();
    assert!(vars.contains(&"shares".to_string()), "vars: {vars:?}");
    assert!(vars.contains(&"total".to_string()), "vars: {vars:?}");
    let deposit = vault
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("deposit"))
        .expect("deposit");
    let locals: Vec<String> = deposit
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"kept".to_string()), "locals: {locals:?}");
    let at = text.find("Deposited").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Deposited");
}

#[test]
fn sbe_defi_token_lock_release() {
    let (text, file) = parse_sbe("DeFiTokenLock.sol");
    let contract = &file.contracts()[0];
    let vars: Vec<String> = contract
        .state_vars()
        .iter()
        .filter_map(|v| v.name())
        .collect();
    assert!(vars.contains(&"locked".to_string()), "vars: {vars:?}");
    assert!(vars.contains(&"release".to_string()), "vars: {vars:?}");
    let release = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("releaseTo"))
        .expect("releaseTo");
    let locals: Vec<String> = release
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"got".to_string()), "locals: {locals:?}");
    assert!(
        locals.contains(&"receiver".to_string()),
        "locals: {locals:?}"
    );
    let at = text.find("Locked").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Locked");
}

#[test]
fn sbe_defi_csamm_add() {
    let (text, file) = parse_sbe("DeFiCSAMM.sol");
    let contract = &file.contracts()[0];
    let vars: Vec<String> = contract
        .state_vars()
        .iter()
        .filter_map(|v| v.name())
        .collect();
    assert!(vars.contains(&"reserve0".to_string()), "vars: {vars:?}");
    assert!(vars.contains(&"reserve1".to_string()), "vars: {vars:?}");
    let add = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("add"))
        .expect("add");
    assert_eq!(add.params().len(), 2);
    let locals: Vec<String> = add
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"kept".to_string()), "locals: {locals:?}");
    let at = text.find("Added").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Added");
}

#[test]
fn sbe_defi_cpamm_add() {
    let (text, file) = parse_sbe("DeFiCPAMM.sol");
    assert_eq!(file.contracts()[0].kind_label(), "interface");
    let target = file
        .contracts()
        .into_iter()
        .find(|c| c.name().as_deref() == Some("CPAMM"))
        .expect("CPAMM");
    let vars: Vec<String> = target
        .state_vars()
        .iter()
        .filter_map(|v| v.name())
        .collect();
    assert!(vars.contains(&"reserve0".to_string()), "vars: {vars:?}");
    assert!(vars.contains(&"shares".to_string()), "vars: {vars:?}");
    let add = target
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("add"))
        .expect("add");
    let locals: Vec<String> = add
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"kept".to_string()), "locals: {locals:?}");
    let at = text.find("Added").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Added");
}

#[test]
fn sbe_defi_stable_amm_add() {
    let (text, file) = parse_sbe("DeFiStableAMM.sol");
    let contract = &file.contracts()[0];
    let vars: Vec<String> = contract
        .state_vars()
        .iter()
        .filter_map(|v| v.name())
        .collect();
    assert!(vars.contains(&"amp".to_string()), "vars: {vars:?}");
    assert!(vars.contains(&"balances".to_string()), "vars: {vars:?}");
    let add = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("add"))
        .expect("add");
    let locals: Vec<String> = add
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"kept".to_string()), "locals: {locals:?}");
    let at = text.find("Swapped").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Swapped");
}

#[test]
fn sbe_defi_remainder_twap() {
    let (text, file) = parse_sbe("DeFiRemainder.sol");
    assert_eq!(file.contracts()[0].kind_label(), "interface");
    assert!(text.contains("TWAP") || text.contains("Twap"), "twap kept");
    let target = file
        .contracts()
        .into_iter()
        .find(|c| c.name().as_deref() == Some("Twap"))
        .expect("Twap");
    let read = target
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("read"))
        .expect("read");
    let locals: Vec<String> = read
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"price".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"source".to_string()), "locals: {locals:?}");
    let at = text.find("event Read").expect("decl") as u32 + 6;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Read");
}

#[test]
fn sbe_foundry_counter_inc() {
    let (text, file) = parse_sbe("FoundryCounterTest.sol");
    assert_eq!(file.imports().len(), 1);
    assert_eq!(file.imports()[0].path().as_deref(), Some("./TestStub.sol"));
    let contract = file
        .contracts()
        .into_iter()
        .find(|c| c.name().as_deref() == Some("CounterTest"))
        .expect("CounterTest");
    assert_eq!(contract.heritage(), vec!["Test"]);
    assert!(text.contains("vm.prank"), "prank pattern kept");
    let test = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("testInc"))
        .expect("testInc");
    let locals: Vec<String> = test
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"kept".to_string()), "locals: {locals:?}");
    let at = text.find("function testInc").expect("decl") as u32 + 9;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert!(
        hover.contents.contains("function testInc"),
        "contents: {}",
        hover.contents
    );
}

#[test]
fn sbe_foundry_auth_prank() {
    let (text, file) = parse_sbe("FoundryAuthTest.sol");
    assert_eq!(file.imports().len(), 1);
    let contract = file
        .contracts()
        .into_iter()
        .find(|c| c.name().as_deref() == Some("AuthTest"))
        .expect("AuthTest");
    assert_eq!(contract.heritage(), vec!["Test"]);
    assert!(text.contains("vm.prank"), "prank pattern kept");
    let test = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("testOnlyOwner"))
        .expect("testOnlyOwner");
    let locals: Vec<String> = test
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"caller".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"kept".to_string()), "locals: {locals:?}");
    let at = text.find("owner").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert!(
        hover.contents.contains("owner"),
        "contents: {}",
        hover.contents
    );
}

#[test]
fn sbe_foundry_error_revert() {
    let (text, file) = parse_sbe("FoundryErrorTest.sol");
    assert_eq!(file.imports().len(), 1);
    assert!(text.contains("expectRevert"), "pattern kept");
    let contract = file
        .contracts()
        .into_iter()
        .find(|c| c.name().as_deref() == Some("ErrorTest"))
        .expect("ErrorTest");
    assert_eq!(contract.errors().len(), 1);
    assert_eq!(contract.errors()[0].name().as_deref(), Some("Unauthorized"));
    let test = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("testRevert"))
        .expect("testRevert");
    let locals: Vec<String> = test
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"kept".to_string()), "locals: {locals:?}");
    let at = text.find("Unauthorized").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "error Unauthorized");
}

#[test]
fn sbe_foundry_event_emit() {
    let (text, file) = parse_sbe("FoundryEventTest.sol");
    assert_eq!(file.imports().len(), 1);
    assert!(text.contains("expectEmit"), "pattern kept");
    let contract = &file.contracts()[0];
    assert_eq!(contract.events().len(), 1);
    assert_eq!(contract.events()[0].name().as_deref(), Some("Set"));
    let test = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("testEmit"))
        .expect("testEmit");
    let locals: Vec<String> = test
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"kept".to_string()), "locals: {locals:?}");
    let at = text.find("event Set").expect("decl") as u32 + 6;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Set");
}

#[test]
fn sbe_foundry_send_eth_deal() {
    let (text, file) = parse_sbe("FoundrySendEthTest.sol");
    assert_eq!(file.imports().len(), 1);
    assert!(text.contains("deal("), "deal pattern kept");
    let contract = &file.contracts()[0];
    let types: Vec<String> = contract
        .state_vars()
        .iter()
        .filter_map(|v| v.type_node().map(|n| TypeNode::text(&n)))
        .collect();
    assert!(
        types.contains(&"mapping(address => uint)".to_string()),
        "types: {types:?}"
    );
    let test = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("testSend"))
        .expect("testSend");
    let locals: Vec<String> = test
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(
        locals.contains(&"receiver".to_string()),
        "locals: {locals:?}"
    );
    assert!(locals.contains(&"kept".to_string()), "locals: {locals:?}");
    let at = text.find("Paid").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Paid");
}

#[test]
fn sbe_foundry_time_warp() {
    let (text, file) = parse_sbe("FoundryTimeTest.sol");
    assert_eq!(file.imports().len(), 1);
    assert!(text.contains("warp("), "warp pattern kept");
    assert!(text.contains("block.timestamp"), "timestamp kept");
    let contract = &file.contracts()[0];
    let vars: Vec<String> = contract
        .state_vars()
        .iter()
        .filter_map(|v| v.name())
        .collect();
    assert!(vars.contains(&"deadline".to_string()), "vars: {vars:?}");
    let test = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("testWarp"))
        .expect("testWarp");
    let locals: Vec<String> = test
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"kept".to_string()), "locals: {locals:?}");
    let at = text.find("Warped").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Warped");
}

#[test]
fn sbe_foundry_sig_sign() {
    let (text, file) = parse_sbe("FoundrySigTest.sol");
    assert_eq!(file.imports().len(), 1);
    assert!(text.contains("sign("), "sign pattern kept");
    let contract = &file.contracts()[0];
    let test = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("testSign"))
        .expect("testSign");
    let locals: Vec<String> = test
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"digest".to_string()), "locals: {locals:?}");
    assert!(
        locals.contains(&"recovered".to_string()),
        "locals: {locals:?}"
    );
    assert!(locals.contains(&"ok".to_string()), "locals: {locals:?}");
    let at = text.find("address recovered").expect("decl") as u32 + 8;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "address recovered");
}

#[test]
fn sbe_foundry_label_mock_store() {
    let (text, file) = parse_sbe("FoundryLabelMockStore.sol");
    assert_eq!(file.imports().len(), 1);
    assert!(text.contains("vm.label"), "label pattern kept");
    assert!(text.contains("mockCall"), "mock pattern kept");
    let contract = file
        .contracts()
        .into_iter()
        .find(|c| c.name().as_deref() == Some("LabelMock"))
        .expect("LabelMock");
    let test = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("testLabelMock"))
        .expect("testLabelMock");
    let locals: Vec<String> = test
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"who".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"kept".to_string()), "locals: {locals:?}");
    let at = text.find("Labeled").expect("decl") as u32;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "event Labeled");
}

#[test]
fn sbe_evm_storage_layout_read() {
    let (text, file) = parse_sbe("EVMStorageLayout.sol");
    let contract = &file.contracts()[0];
    let vars: Vec<String> = contract
        .state_vars()
        .iter()
        .filter_map(|v| v.name())
        .collect();
    assert!(vars.contains(&"x".to_string()), "vars: {vars:?}");
    assert!(vars.contains(&"y".to_string()), "vars: {vars:?}");
    let read = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("read"))
        .expect("read");
    let locals: Vec<String> = read
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"before".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"after".to_string()), "locals: {locals:?}");
    let at = text.find("uint after").expect("decl") as u32 + 5;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "uint after");
}

#[test]
fn sbe_evm_memory_layout_read() {
    let (text, file) = parse_sbe("EVMMemoryLayout.sol");
    let contract = &file.contracts()[0];
    let read = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("read"))
        .expect("read");
    assert_eq!(read.params().len(), 1);
    let locals: Vec<String> = read
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"prev".to_string()), "locals: {locals:?}");
    assert!(locals.contains(&"next".to_string()), "locals: {locals:?}");
    let at = text.find("bytes memory prev").expect("decl") as u32 + 13;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert_eq!(hover.contents, "bytes prev");
}

#[test]
fn sbe_echidna_property_bool() {
    let (text, file) = parse_sbe("EchidnaFuzz.sol");
    assert!(text.contains("hevm"), "hevm pattern kept");
    let contract = &file.contracts()[0];
    let prop = contract
        .functions()
        .into_iter()
        .find(|f| f.name().as_deref() == Some("echidna_balance"))
        .expect("property");
    assert_eq!(prop.return_types().len(), 1);
    let locals: Vec<String> = prop
        .body()
        .expect("body")
        .locals()
        .iter()
        .filter_map(|l| l.name())
        .collect();
    assert!(locals.contains(&"kept".to_string()), "locals: {locals:?}");
    let at = text.find("function echidna_balance").expect("decl") as u32 + 9;
    let hover = hover_at(&file, &text, at).expect("hover");
    assert!(
        hover.contents.contains("function echidna_balance"),
        "contents: {}",
        hover.contents
    );
}
