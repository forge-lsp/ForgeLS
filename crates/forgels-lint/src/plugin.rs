use std::collections::HashMap;

use super::{RuleId, Severity};

// One rule entry in a plugin: stable ID plus default severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleMeta {
    pub id: RuleId,
    pub default: Severity,
}

// Static plugin: name plus rule list. No check fn here on purpose:
// checks live in language crates (they own the AST) and call into
// this engine with plain diagnostics, so core never imports lang.
pub trait LintPlugin {
    fn name() -> &'static str;
    fn rules() -> Vec<RuleMeta>;
}

// Builtin correctness rules: today's diagnostics, unchanged behavior.
pub struct CorrectnessPlugin;

impl LintPlugin for CorrectnessPlugin {
    fn name() -> &'static str {
        "correctness"
    }

    fn rules() -> Vec<RuleMeta> {
        RuleId::all()
            .iter()
            .map(|id| RuleMeta {
                id: *id,
                default: id.default_severity(),
            })
            .collect()
    }
}

pub fn builtin_correctness_plugin() -> Vec<RuleMeta> {
    CorrectnessPlugin::rules()
}

// Rule lookup keyed by full `group/name` string.
#[derive(Debug, Clone, Default)]
pub struct Registry {
    rules: HashMap<&'static str, RuleMeta>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_builtins() -> Self {
        let mut out = Self::new();
        out.register::<CorrectnessPlugin>();
        out
    }

    pub fn register<P: LintPlugin>(&mut self) {
        for meta in P::rules() {
            self.rules.insert(meta.id.as_str(), meta);
        }
    }

    pub fn get(&self, id: &str) -> Option<&RuleMeta> {
        self.rules.get(id)
    }

    pub fn len(&self) -> usize {
        self.rules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correctness_plugin_lists_all_rules() {
        let rules = builtin_correctness_plugin();
        assert_eq!(rules.len(), RuleId::all().len());
        assert_eq!(rules.len(), 22);
        for id in RuleId::all() {
            assert!(rules.iter().any(|meta| meta.id == *id));
        }
    }

    #[test]
    fn registry_resolves_full_rule_id() {
        let registry = Registry::with_builtins();
        let meta = registry
            .get("correctness/parse-error")
            .expect("builtin rule resolves");
        assert_eq!(meta.id, RuleId::PARSE_ERROR);
        assert_eq!(meta.default, Severity::Error);
        assert!(registry.get("correctness/nope").is_none());
    }

    #[test]
    fn register_covers_every_rule_id() {
        let registry = Registry::with_builtins();
        assert_eq!(registry.len(), 22);
        for id in RuleId::all() {
            assert!(registry.get(id.as_str()).is_some());
        }
    }
}
