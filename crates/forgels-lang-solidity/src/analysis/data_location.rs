use forgels_core::DiagnosticData;
use forgels_lint::RuleId;
use forgels_syntax::AstNode;

use crate::{DataLocation, SolidityFile};

fn first_word(type_text: &str) -> &str {
    type_text.split_whitespace().next().unwrap_or("")
}

fn is_mapping_text(type_text: &str) -> bool {
    type_text.trim_start().starts_with("mapping")
}

fn is_array_text(type_text: &str) -> bool {
    type_text.contains('[')
}

fn is_string_text(type_text: &str) -> bool {
    first_word(type_text) == "string"
}

fn is_dynamic_bytes_text(type_text: &str) -> bool {
    first_word(type_text) == "bytes"
}

fn is_sized_bytes_text(word: &str) -> bool {
    if let Some(rest) = word.strip_prefix("bytes")
        && !rest.is_empty()
        && rest.bytes().all(|byte| byte.is_ascii_digit())
    {
        return true;
    }
    false
}

fn is_value_text(type_text: &str) -> bool {
    let word = first_word(type_text);
    if word.is_empty() {
        return false;
    }
    if word == "bool" || word == "address" {
        return true;
    }
    if word == "uint" || word == "int" || word == "fixed" || word == "ufixed" {
        return true;
    }
    if word.starts_with("uint") || word.starts_with("int") {
        let rest = word
            .strip_prefix("uint")
            .or_else(|| word.strip_prefix("int"));
        if let Some(rest) = rest
            && !rest.is_empty()
            && rest.bytes().all(|byte| byte.is_ascii_digit())
        {
            return true;
        }
    }
    if is_sized_bytes_text(word) {
        return true;
    }
    if word.starts_with("fixed") || word.starts_with("ufixed") {
        return true;
    }
    false
}

fn is_reference_text(type_text: &str) -> bool {
    if is_mapping_text(type_text) {
        return true;
    }
    if is_array_text(type_text) {
        return true;
    }
    if is_string_text(type_text) || is_dynamic_bytes_text(type_text) {
        return true;
    }
    false
}

fn is_user_text(type_text: &str) -> bool {
    if type_text.trim().is_empty() {
        return false;
    }
    if is_mapping_text(type_text) || is_array_text(type_text) {
        return false;
    }
    if is_value_text(type_text) {
        return false;
    }
    if is_string_text(type_text) || is_dynamic_bytes_text(type_text) {
        return false;
    }
    true
}

fn span_or_full(
    name_span: Option<forgels_syntax::Span>,
    full: forgels_syntax::Span,
) -> forgels_syntax::Span {
    name_span.unwrap_or(full)
}

/// Data location checks for a sound subset.
///
/// State vars never carry a location. Value params and locals must not
/// carry one. Reference params and locals (`string`, `bytes`, arrays,
/// mappings) must carry one; mappings need `storage`. User types stay
/// silent since struct versus contract cannot be told from text.
/// Struct fields and event or error params stay silent.
pub fn data_location_diagnostics(file: &SolidityFile) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    for contract in file.contracts() {
        for var in contract.state_vars() {
            let Some(location) = var.data_location() else {
                continue;
            };
            let span = var
                .data_location_span()
                .unwrap_or_else(|| var.syntax().span);
            out.push(DiagnosticData::error(
                span,
                format!(
                    "data location '{}' not allowed for state variable",
                    location.as_str()
                ),
                RuleId::DATA_LOCATION,
            ));
        }
        for function in contract.functions() {
            for param in function.params() {
                check_param(&mut out, &param);
            }
            let Some(body) = function.body() else {
                continue;
            };
            for local in body.all_locals() {
                check_local(&mut out, &local);
            }
        }
    }
    out
}

fn check_param(out: &mut Vec<DiagnosticData>, param: &crate::Param) {
    use forgels_syntax::AstNode as _;
    let Some(type_text) = param.type_node().map(|node| node.text()) else {
        return;
    };
    if type_text.trim_start().starts_with("function") {
        return;
    }
    if is_user_text(&type_text) && !is_array_text(&type_text) {
        return;
    }
    let location = param.data_location();
    if is_mapping_text(&type_text) {
        match location {
            Some(DataLocation::Storage) => {}
            Some(other) => {
                let span = param
                    .data_location_span()
                    .unwrap_or_else(|| param.syntax().span);
                out.push(DiagnosticData::error(
                    span,
                    format!("mapping can only be 'storage', found '{}'", other.as_str()),
                    RuleId::DATA_LOCATION,
                ));
            }
            None => {
                let span = span_or_full(param.name_span(), param.syntax().span);
                out.push(DiagnosticData::error(
                    span,
                    format!("mapping '{type_text}' needs 'storage' data location"),
                    RuleId::DATA_LOCATION,
                ));
            }
        }
        return;
    }
    if is_value_text(&type_text) {
        if let Some(other) = location {
            let span = param
                .data_location_span()
                .unwrap_or_else(|| param.syntax().span);
            out.push(DiagnosticData::error(
                span,
                format!(
                    "data location '{}' not allowed for '{type_text}'",
                    other.as_str()
                ),
                RuleId::DATA_LOCATION,
            ));
        }
        return;
    }
    if is_reference_text(&type_text) && location.is_none() {
        let span = span_or_full(param.name_span(), param.syntax().span);
        out.push(DiagnosticData::error(
            span,
            format!("data location must be specified for '{type_text}'"),
            RuleId::DATA_LOCATION,
        ));
    }
}

fn check_local(out: &mut Vec<DiagnosticData>, local: &crate::LocalVar) {
    use forgels_syntax::AstNode as _;
    let Some(type_text) = local.type_node().map(|node| node.text()) else {
        return;
    };
    if type_text.trim_start().starts_with("function") {
        return;
    }
    if is_user_text(&type_text) && !is_array_text(&type_text) {
        return;
    }
    let location = local.data_location();
    if is_mapping_text(&type_text) {
        match location {
            Some(DataLocation::Storage) => {}
            Some(other) => {
                let span = local
                    .data_location_span()
                    .unwrap_or_else(|| local.syntax().span);
                out.push(DiagnosticData::error(
                    span,
                    format!("mapping can only be 'storage', found '{}'", other.as_str()),
                    RuleId::DATA_LOCATION,
                ));
            }
            None => {
                let span = span_or_full(local.name_span(), local.syntax().span);
                out.push(DiagnosticData::error(
                    span,
                    format!("mapping '{type_text}' needs 'storage' data location"),
                    RuleId::DATA_LOCATION,
                ));
            }
        }
        return;
    }
    if is_value_text(&type_text) {
        if let Some(other) = location {
            let span = local
                .data_location_span()
                .unwrap_or_else(|| local.syntax().span);
            out.push(DiagnosticData::error(
                span,
                format!(
                    "data location '{}' not allowed for '{type_text}'",
                    other.as_str()
                ),
                RuleId::DATA_LOCATION,
            ));
        }
        return;
    }
    if let Some(DataLocation::Calldata) = location {
        let span = local
            .data_location_span()
            .unwrap_or_else(|| local.syntax().span);
        out.push(DiagnosticData::error(
            span,
            String::from("data location 'calldata' not allowed for local variable"),
            RuleId::DATA_LOCATION,
        ));
        return;
    }
    if is_reference_text(&type_text) && location.is_none() {
        let span = span_or_full(local.name_span(), local.syntax().span);
        out.push(DiagnosticData::error(
            span,
            format!("data location must be specified for '{type_text}'"),
            RuleId::DATA_LOCATION,
        ));
    }
}
