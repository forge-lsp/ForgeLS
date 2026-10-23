//! forgels-lint: shared lint engine library for all language crates.
//!
//! Rule IDs plus severity taxonomy, config, static plugin registry,
//! severity resolver, safe fix applier, and human/JSON reporters.
//! No parsing here: languages push diagnostics in, the runner resolves
//! them against config. No dynamic loading, no language imports.

pub mod config;
pub mod diagnostic;
pub mod filter;
pub mod fix;
pub mod plugin;
pub mod reporter;
pub mod rule;
pub mod runner;
pub mod severity;

pub use config::{
    ExternalRef, FlsConfig, FormatConfig, LintConfig, LintConfigError, RuleLevel, validate_rule_id,
};
pub use diagnostic::{Applicability, LintDiagnostic, LintFix};
pub use filter::should_lint;
pub use fix::{FixOutcome, apply};
pub use plugin::{
    CorrectnessPlugin, LintPlugin, Registry, RuleMeta, SafetyPlugin, StylePlugin,
    builtin_correctness_plugin, builtin_safety_plugin, builtin_style_plugin,
};
pub use reporter::{FileReport, report, report_json};
pub use rule::RuleId;
pub use runner::resolve;
pub use severity::Severity;
