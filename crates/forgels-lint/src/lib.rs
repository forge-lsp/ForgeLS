//! forgels-lint: shared lint engine library for all language crates.
//!
//! Rule IDs plus severity taxonomy, config, static plugin registry,
//! severity resolver, safe fix applier, and human/JSON reporters.
//! No parsing here: languages push diagnostics in, the runner resolves
//! them against config. No dynamic loading, no language imports.

pub mod config;
pub mod diagnostic;
pub mod fix;
pub mod plugin;
pub mod reporter;
pub mod rule;
pub mod runner;
pub mod severity;

pub use config::{LintConfig, LintConfigError, RuleLevel};
pub use diagnostic::{Applicability, LintDiagnostic, LintFix};
pub use fix::{FixOutcome, apply};
pub use plugin::{CorrectnessPlugin, LintPlugin, Registry, RuleMeta, builtin_correctness_plugin};
pub use reporter::{FileReport, report, report_json};
pub use rule::RuleId;
pub use runner::resolve;
pub use severity::Severity;
