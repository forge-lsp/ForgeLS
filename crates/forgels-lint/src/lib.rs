//! forgels-lint: shared lint severity taxonomy and rule registry.
//!
//! Severity plus rule IDs only. No engine, runner, reporter, or config here.

pub mod rule;
pub mod severity;

pub use rule::RuleId;
pub use severity::Severity;
