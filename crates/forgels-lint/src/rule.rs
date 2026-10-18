use super::Severity;

// Stable lint rule identifier, for example `correctness/parse-error`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RuleId(&'static str);

impl RuleId {
    pub const PARSE_ERROR: Self = Self("correctness/parse-error");
    pub const CALL_ARITY: Self = Self("correctness/call-arity");
    pub const STRUCT_ARITY: Self = Self("correctness/struct-arity");
    pub const ERROR_ARITY: Self = Self("correctness/error-arity");
    pub const DEPLOY_ARITY: Self = Self("correctness/deploy-arity");
    pub const EMIT_ARITY: Self = Self("correctness/emit-arity");
    pub const BUILTIN_ARITY: Self = Self("correctness/builtin-arity");
    pub const CALL_OPTIONS: Self = Self("correctness/call-options");
    pub const UNRESOLVED_IDENTIFIER: Self = Self("correctness/unresolved-identifier");
    pub const UNRESOLVED_IMPORT: Self = Self("correctness/unresolved-import");
    pub const UNRESOLVED_EVENT: Self = Self("correctness/unresolved-event");
    pub const VISIBILITY: Self = Self("correctness/visibility");
    pub const MUTABILITY: Self = Self("correctness/mutability");
    pub const PAYABLE_VALUE: Self = Self("correctness/payable-value");
    pub const TYPE_MISMATCH: Self = Self("correctness/type-mismatch");
    pub const SHADOWING: Self = Self("correctness/shadowing");
    pub const UNUSED: Self = Self("correctness/unused");
    pub const DATA_LOCATION: Self = Self("correctness/data-location");
    pub const OVERRIDE: Self = Self("correctness/override");
    pub const INTERFACE_RULES: Self = Self("correctness/interface-rules");
    pub const INDEXED_LIMIT: Self = Self("correctness/indexed-limit");
    pub const INCONSISTENT_INHERITANCE: Self = Self("correctness/inconsistent-inheritance");
    pub const NAMING: Self = Self("style/naming");
    pub const MODIFIER_ORDER: Self = Self("style/modifier-order");
    pub const FUNCTION_ORDER: Self = Self("style/function-order");
    pub const LAYOUT_ORDER: Self = Self("style/layout-order");
    pub const IMPORT_POSITION: Self = Self("style/import-position");
    pub const MAPPING_GLUE: Self = Self("style/mapping-glue");
    pub const ARRAY_GLUE: Self = Self("style/array-glue");
    pub const BLANK_LINES: Self = Self("style/blank-lines");
    pub const NATSPEC_MISSING: Self = Self("style/natspec-missing");
    pub const NATSPEC_PARAM: Self = Self("style/natspec-param");
    pub const LINE_LENGTH: Self = Self("style/line-length");

    pub const fn as_str(self) -> &'static str {
        self.0
    }

    pub fn default_severity(self) -> Severity {
        match self.0 {
            "style/naming"
            | "style/modifier-order"
            | "style/function-order"
            | "style/layout-order"
            | "style/import-position"
            | "style/mapping-glue"
            | "style/array-glue"
            | "style/blank-lines"
            | "style/natspec-missing"
            | "style/natspec-param"
            | "style/line-length" => Severity::Warning,
            _ => Severity::Error,
        }
    }

    pub fn all() -> &'static [Self] {
        &ALL
    }
}

const ALL: [RuleId; 33] = [
    RuleId::PARSE_ERROR,
    RuleId::CALL_ARITY,
    RuleId::STRUCT_ARITY,
    RuleId::ERROR_ARITY,
    RuleId::DEPLOY_ARITY,
    RuleId::EMIT_ARITY,
    RuleId::BUILTIN_ARITY,
    RuleId::CALL_OPTIONS,
    RuleId::UNRESOLVED_IDENTIFIER,
    RuleId::UNRESOLVED_IMPORT,
    RuleId::UNRESOLVED_EVENT,
    RuleId::VISIBILITY,
    RuleId::MUTABILITY,
    RuleId::PAYABLE_VALUE,
    RuleId::TYPE_MISMATCH,
    RuleId::SHADOWING,
    RuleId::UNUSED,
    RuleId::DATA_LOCATION,
    RuleId::OVERRIDE,
    RuleId::INTERFACE_RULES,
    RuleId::INDEXED_LIMIT,
    RuleId::INCONSISTENT_INHERITANCE,
    RuleId::NAMING,
    RuleId::MODIFIER_ORDER,
    RuleId::FUNCTION_ORDER,
    RuleId::LAYOUT_ORDER,
    RuleId::IMPORT_POSITION,
    RuleId::MAPPING_GLUE,
    RuleId::ARRAY_GLUE,
    RuleId::BLANK_LINES,
    RuleId::NATSPEC_MISSING,
    RuleId::NATSPEC_PARAM,
    RuleId::LINE_LENGTH,
];
