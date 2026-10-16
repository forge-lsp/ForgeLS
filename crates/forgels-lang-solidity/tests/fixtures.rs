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
