use forgels_syntax::Span;

use super::{RuleId, Severity};

// Fix safety. V1 emits Safe only; Unsafe exists so the applier and
// reporters can already filter it. No `--unsafe` apply mode in V1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applicability {
    Safe,
    Unsafe,
}

impl Applicability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Safe => "safe",
            Self::Unsafe => "unsafe",
        }
    }
}

// One safe edit: replace `span` with `replacement`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintFix {
    pub span: Span,
    pub replacement: String,
    pub applicability: Applicability,
}

impl LintFix {
    pub fn safe(span: Span, replacement: String) -> Self {
        Self {
            span,
            replacement,
            applicability: Applicability::Safe,
        }
    }
}

// One lint finding. Languages build this from their own analysis;
// Solidity does not produce it yet, so no language implements the
// plugin check trait in V1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintDiagnostic {
    pub span: Span,
    pub message: String,
    pub rule: RuleId,
    pub severity: Severity,
    pub fix: Option<LintFix>,
}

impl LintDiagnostic {
    // Mirrors `forgels-core` DiagnosticData fields one to one, kept as
    // plain parts (not the core type) to avoid a dependency cycle:
    // core already depends on this crate.
    pub fn from_parts(span: Span, message: String, rule: RuleId, severity: Severity) -> Self {
        Self {
            span,
            message,
            rule,
            severity,
            fix: None,
        }
    }

    pub fn error(span: Span, message: String, rule: RuleId) -> Self {
        Self::from_parts(span, message, rule, Severity::Error)
    }

    pub fn warning(span: Span, message: String, rule: RuleId) -> Self {
        Self::from_parts(span, message, rule, Severity::Warning)
    }

    pub fn with_fix(mut self, fix: LintFix) -> Self {
        self.fix = Some(fix);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_parts_carries_fields_without_fix() {
        let diag = LintDiagnostic::error(Span::new(1, 4), "bad".to_string(), RuleId::UNUSED);
        assert_eq!(diag.span, Span::new(1, 4));
        assert_eq!(diag.severity, Severity::Error);
        assert!(diag.fix.is_none());
    }

    #[test]
    fn with_fix_attaches_safe_edit() {
        let diag = LintDiagnostic::warning(Span::new(0, 0), "tip".to_string(), RuleId::OVERRIDE)
            .with_fix(LintFix::safe(Span::new(0, 0), "override ".to_string()));
        let fix = diag.fix.expect("fix attached");
        assert_eq!(fix.applicability, Applicability::Safe);
        assert_eq!(fix.applicability.as_str(), "safe");
        assert_eq!(Applicability::Unsafe.as_str(), "unsafe");
    }
}
