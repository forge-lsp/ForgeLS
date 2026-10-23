use std::path::Path;

use forgels_common::fs::is_trackable_file;
use forgels_common::paths::{Remapping, resolve_import_full};
use forgels_core::Analyzer;
use forgels_core::diagnostics::unresolved_import_to_diagnostic;
use forgels_lang_solidity::{SolidityAnalyzer, filename_diagnostics};
use forgels_lint::{
    LintConfig, LintDiagnostic, PluginSet, RuleId, Severity, filter_by_plugin, resolve,
};

use super::external::{ExternalHosts, lint_with_guests};

/// Build lint diagnostics for one file.
///
/// Runs the single-file analyzer, then adds unresolved imports via
/// the shared server resolver, then guest lint calls, then the
/// plugin group gate for builtins only, then one `resolve()`.
/// Guests always run when configured; empty hosts keep builtins
/// byte-identical. Group gate wins over per-rule levels.
pub fn diagnostics_for_file(
    path: &Path,
    text: &str,
    lint: &LintConfig,
    plugins: Option<&[String]>,
    root: &Path,
    remappings: &[Remapping],
    external: &mut ExternalHosts,
) -> Vec<LintDiagnostic> {
    let analyzer = SolidityAnalyzer::new();
    let mut out: Vec<LintDiagnostic> = analyzer
        .diagnostics(text)
        .into_iter()
        .map(|item| LintDiagnostic::from_parts(item.span, item.message, item.rule, item.severity))
        .collect();
    // Check runs on real files only, never stdin, so path is always
    // present. Pathless inputs stay skipped by design.
    for item in filename_diagnostics(text, path) {
        out.push(LintDiagnostic::from_parts(
            item.span,
            item.message,
            item.rule,
            item.severity,
        ));
    }
    for item in analyzer.imports(text) {
        let resolved = resolve_import_full(path, &item.raw, root, remappings, is_trackable_file);
        let missing = match &resolved {
            None => true,
            Some(target) => !is_trackable_file(target),
        };
        if !missing {
            continue;
        }
        let mapped = unresolved_import_to_diagnostic(text, item.span, &item.raw);
        out.push(LintDiagnostic::from_parts(
            item.span,
            mapped.message,
            RuleId::UNRESOLVED_IMPORT,
            Severity::Error,
        ));
    }
    let combined = lint_with_guests(external, path, text, out);
    let gated = filter_by_plugin(combined, &PluginSet::from_option(plugins));
    resolve(gated, lint)
}
