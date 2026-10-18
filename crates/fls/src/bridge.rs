use std::path::Path;

use forgels_common::fs::is_trackable_file;
use forgels_common::paths::{Remapping, resolve_import_full};
use forgels_core::Analyzer;
use forgels_core::diagnostics::unresolved_import_to_diagnostic;
use forgels_lang_solidity::SolidityAnalyzer;
use forgels_lint::{LintConfig, LintDiagnostic, RuleId, Severity, resolve};

/// Build lint diagnostics for one file.
///
/// Runs the single-file analyzer, then adds unresolved imports via
/// the shared server resolver, then applies config levels with the
/// lint engine resolver. Dep-aware rules run without tracked deps,
/// so cross-file findings may be missing: single-file degrade by
/// design. Root is cwd or the config root.
pub fn diagnostics_for_file(
    path: &Path,
    text: &str,
    lint: &LintConfig,
    root: &Path,
    remappings: &[Remapping],
) -> Vec<LintDiagnostic> {
    let analyzer = SolidityAnalyzer::new();
    let mut out: Vec<LintDiagnostic> = analyzer
        .diagnostics(text)
        .into_iter()
        .map(|item| LintDiagnostic::from_parts(item.span, item.message, item.rule, item.severity))
        .collect();
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
    resolve(out, lint)
}
