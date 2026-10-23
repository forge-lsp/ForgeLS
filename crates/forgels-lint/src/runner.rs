use super::Severity;
use super::config::{LintConfig, RuleLevel};
use super::diagnostic::LintDiagnostic;
use super::plugin::PluginSet;

// Apply config levels to pushed diagnostics. Off drops, warn and err
// override severity, absent keeps the language default. Stable sort by
// span start, span end, then message. No parsing, no AST.
pub fn resolve(diagnostics: Vec<LintDiagnostic>, config: &LintConfig) -> Vec<LintDiagnostic> {
    let mut out: Vec<LintDiagnostic> = diagnostics
        .into_iter()
        .filter_map(|mut diag| match config.level_for(diag.rule) {
            None => Some(diag),
            Some(RuleLevel::Off) => None,
            Some(RuleLevel::Warn) => {
                diag.severity = Severity::Warning;
                Some(diag)
            }
            Some(RuleLevel::Err) => {
                diag.severity = Severity::Error;
                Some(diag)
            }
        })
        .collect();
    out.sort_by(|left, right| {
        left.span
            .start
            .cmp(&right.span.start)
            .then(left.span.end.cmp(&right.span.end))
            .then(left.message.cmp(&right.message))
    });
    out
}

// Group gate runs before per-rule levels: unlisted builtin groups
// drop here, then resolve applies off/warn/err. Guests pass through
// so external lint always runs when configured.
pub fn filter_by_plugin(diagnostics: Vec<LintDiagnostic>, set: &PluginSet) -> Vec<LintDiagnostic> {
    if set.is_all() {
        return diagnostics;
    }
    diagnostics
        .into_iter()
        .filter(|diag| set.allows(diag.rule))
        .collect()
}

// True when builtin formatting may run. Absent means all so format
// runs; listed needs forge:format; empty or unlisted skips builtins
// and leaves guest passes only.
pub fn builtin_format_enabled(set: &PluginSet) -> bool {
    set.contains("forge:format")
}

#[cfg(test)]
mod tests {
    use forgels_syntax::Span;

    use super::super::{RuleId, Severity};
    use super::*;

    fn diag(message: &str, rule: RuleId, severity: Severity) -> LintDiagnostic {
        LintDiagnostic::from_parts(Span::new(0, 1), message.to_string(), rule, severity)
    }

    #[test]
    fn off_drops_matching_rule() {
        let mut config = LintConfig::default();
        config
            .rules
            .insert("correctness/unused".to_string(), RuleLevel::Off);
        let out = resolve(
            vec![
                diag("a", RuleId::UNUSED, Severity::Error),
                diag("b", RuleId::OVERRIDE, Severity::Error),
            ],
            &config,
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].rule, RuleId::OVERRIDE);
    }

    #[test]
    fn warn_and_err_override_severity() {
        let mut config = LintConfig::default();
        config
            .rules
            .insert("correctness/unused".to_string(), RuleLevel::Warn);
        config
            .rules
            .insert("correctness/override".to_string(), RuleLevel::Err);
        let out = resolve(
            vec![
                diag("a", RuleId::UNUSED, Severity::Error),
                diag("b", RuleId::OVERRIDE, Severity::Warning),
            ],
            &config,
        );
        assert_eq!(out[0].severity, Severity::Warning);
        assert_eq!(out[1].severity, Severity::Error);
    }

    #[test]
    fn absent_rule_keeps_default_severity() {
        let out = resolve(
            vec![diag("a", RuleId::UNUSED, Severity::Warning)],
            &LintConfig::default(),
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].severity, Severity::Warning);
    }

    #[test]
    fn sorts_by_span_then_message() {
        let second = LintDiagnostic::from_parts(
            Span::new(5, 9),
            "b".to_string(),
            RuleId::UNUSED,
            Severity::Error,
        );
        let first = LintDiagnostic::from_parts(
            Span::new(1, 2),
            "a".to_string(),
            RuleId::UNUSED,
            Severity::Error,
        );
        let out = resolve(vec![second.clone(), first.clone()], &LintConfig::default());
        assert_eq!(out, vec![first, second]);
    }

    #[test]
    fn plugin_filter_keeps_listed_group_only() {
        let set = PluginSet::from_list(&["forge:style".to_string()]);
        let out = filter_by_plugin(
            vec![
                diag("a", RuleId::UNUSED, Severity::Error),
                diag("b", RuleId::NAMING, Severity::Warning),
            ],
            &set,
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].rule, RuleId::NAMING);
    }

    #[test]
    fn plugin_filter_empty_drops_builtins_keeps_guests() {
        let leaked: &'static str = Box::leak("example/guest-rule".to_string().into_boxed_str());
        let guest = RuleId::from_leaked(leaked);
        let set = PluginSet::from_list(&[]);
        let out = filter_by_plugin(
            vec![
                diag("a", RuleId::UNUSED, Severity::Error),
                diag("g", guest, Severity::Warning),
            ],
            &set,
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].rule, guest);
    }

    #[test]
    fn format_gate_needs_absent_or_listed() {
        assert!(builtin_format_enabled(&PluginSet::all()));
        assert!(builtin_format_enabled(&PluginSet::from_list(&[
            "forge:format".to_string()
        ])));
        assert!(!builtin_format_enabled(&PluginSet::from_list(&[
            "forge:style".to_string()
        ])));
        assert!(!builtin_format_enabled(&PluginSet::from_list(&[])));
    }
}
