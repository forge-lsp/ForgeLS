use std::sync::Mutex;

use forgels_common::log;
use forgels_core::{
    Analyzer, CallHierarchyCallData, CallHierarchyItemData, CodeActionData, CompletionData, Config,
    DiagnosticData, DocumentSymbolData, FoldingRangeData, GotoTarget, HoverData, ImportRef,
    InlayHintData, ProjectContext, ReferenceData, SemanticTokenData, Server, SignatureData,
    TypeHierarchyItemData, routing::Language,
};
use forgels_lang_solidity::SolidityAnalyzer;
use forgels_lint::{FlsConfig, LintConfig, should_lint};
use forgels_syntax::Span;

use super::external::{ExternalHosts, guest_diags, load_external};

/// Analyzer wrapper that appends guest diagnostics. All other calls
/// delegate to the inner Solidity analyzer. No core changes.
#[derive(Debug)]
struct GuestAnalyzer {
    inner: SolidityAnalyzer,
    lint: LintConfig,
    external: Mutex<ExternalHosts>,
}

impl GuestAnalyzer {
    fn new(lint: LintConfig, external: ExternalHosts) -> Self {
        Self {
            inner: SolidityAnalyzer::new(),
            lint,
            external: Mutex::new(external),
        }
    }
}

fn publish_rel(ctx: &ProjectContext) -> String {
    if let Some(root) = ctx.workspace.root_for(ctx.path)
        && let Ok(rel) = ctx.path.strip_prefix(root)
        && !rel.as_os_str().is_empty()
    {
        return rel.to_string_lossy().replace('\\', "/");
    }
    ctx.path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| ctx.path.to_string_lossy().into_owned())
}

impl Analyzer for GuestAnalyzer {
    fn hover(&self, ctx: &ProjectContext) -> Option<HoverData> {
        self.inner.hover(ctx)
    }

    fn complete(&self, ctx: &ProjectContext) -> Vec<CompletionData> {
        self.inner.complete(ctx)
    }

    fn completion_is_incomplete(&self, ctx: &ProjectContext) -> bool {
        self.inner.completion_is_incomplete(ctx)
    }

    fn goto(&self, ctx: &ProjectContext) -> Option<Span> {
        self.inner.goto(ctx)
    }

    fn goto_location(&self, ctx: &ProjectContext) -> Option<GotoTarget> {
        self.inner.goto_location(ctx)
    }

    fn diagnostics(&self, text: &str) -> Vec<DiagnosticData> {
        self.inner.diagnostics(text)
    }

    fn diagnostics_ctx(&self, ctx: &ProjectContext) -> Vec<DiagnosticData> {
        let mut out = self.inner.diagnostics_ctx(ctx);
        // Host-side filter before any guest call: excluded files stay
        // empty so guests never run for them.
        let rel = publish_rel(ctx);
        let (keep, _) = should_lint(&rel, &self.lint.includes, &self.lint.excludes);
        if !keep {
            return out;
        }
        let Ok(mut hosts) = self.external.lock() else {
            return out;
        };
        if !hosts.has_guests() {
            return out;
        }
        for item in guest_diags(&mut hosts, ctx.path, ctx.text) {
            // Guest off levels apply later in the single core resolve.
            out.push(DiagnosticData {
                span: item.span,
                message: item.message,
                severity: item.severity,
                rule: item.rule,
            });
        }
        out
    }

    fn imports(&self, text: &str) -> Vec<ImportRef> {
        self.inner.imports(text)
    }

    fn signature_help(&self, ctx: &ProjectContext) -> Option<Vec<SignatureData>> {
        self.inner.signature_help(ctx)
    }

    fn document_symbols(&self, text: &str) -> Vec<DocumentSymbolData> {
        self.inner.document_symbols(text)
    }

    fn references(&self, ctx: &ProjectContext) -> Option<Vec<ReferenceData>> {
        self.inner.references(ctx)
    }

    fn rename(&self, ctx: &ProjectContext, new_name: &str) -> Option<Vec<ReferenceData>> {
        self.inner.rename(ctx, new_name)
    }

    fn inlay_hints(&self, ctx: &ProjectContext, range: Span) -> Vec<InlayHintData> {
        self.inner.inlay_hints(ctx, range)
    }

    fn semantic_tokens(&self, text: &str) -> Vec<SemanticTokenData> {
        self.inner.semantic_tokens(text)
    }

    fn prepare_call_hierarchy(&self, ctx: &ProjectContext) -> Option<CallHierarchyItemData> {
        self.inner.prepare_call_hierarchy(ctx)
    }

    fn incoming_calls(&self, ctx: &ProjectContext) -> Option<Vec<CallHierarchyCallData>> {
        self.inner.incoming_calls(ctx)
    }

    fn outgoing_calls(&self, ctx: &ProjectContext) -> Option<Vec<CallHierarchyCallData>> {
        self.inner.outgoing_calls(ctx)
    }

    fn code_actions(&self, ctx: &ProjectContext, range: Span) -> Vec<CodeActionData> {
        self.inner.code_actions(ctx, range)
    }

    fn folding_ranges(&self, text: &str) -> Vec<FoldingRangeData> {
        self.inner.folding_ranges(text)
    }

    fn selection_range(&self, ctx: &ProjectContext) -> Vec<Span> {
        self.inner.selection_range(ctx)
    }

    fn prepare_type_hierarchy(&self, ctx: &ProjectContext) -> Option<TypeHierarchyItemData> {
        self.inner.prepare_type_hierarchy(ctx)
    }

    fn supertypes(&self, ctx: &ProjectContext) -> Option<Vec<TypeHierarchyItemData>> {
        self.inner.supertypes(ctx)
    }

    fn subtypes(&self, ctx: &ProjectContext) -> Option<Vec<TypeHierarchyItemData>> {
        self.inner.subtypes(ctx)
    }
}

pub fn run(level_flag: Option<&str>) {
    let (mut config, warnings) = Config::load(std::path::Path::new("forgels.toml"));
    for warning in warnings {
        eprintln!("fls: {warning}");
    }
    let (foundry, foundry_warnings) = Config::load_foundry(std::path::Path::new("foundry.toml"));
    for warning in foundry_warnings {
        eprintln!("fls: {warning}");
    }
    config.merge_remappings(foundry);
    // No watcher: lint config plus guests load once at startup,
    // restart to pick up edits.
    let (lint, _plugin, external) = load_lint_with_guests();
    // Scaffold only: top-level plugin parses but never gates in V1.
    // Kept in parity with `check` so both paths carry the same vec.
    config.lint = lint.clone();
    let (level, warning) = log::resolve_level(level_flag, &config.log_level);
    if let Some(warning) = warning {
        eprintln!("fls: {warning}");
    }
    log::init(level);
    log::info!("starting fls lsp");
    let mut server = Server::new(config);
    server.set_analyzer(
        Language::Solidity,
        Box::new(GuestAnalyzer::new(lint, external)),
    );
    server.run();
}

// Prefer fls.json, fall back to forgels.toml lint text, else defaults.
// Broken files warn and run with defaults so the server never fails open.
// Guests load once here; unknown rules warn with defaults in LSP.
// Top-level plugin parses alongside lint for `check` parity but stays
// unused in V1: no gating, no guest change.
fn load_lint_with_guests() -> (LintConfig, Vec<String>, ExternalHosts) {
    if let Ok(text) = std::fs::read_to_string("fls.json") {
        let refs = match FlsConfig::parse_external_refs(&text) {
            Ok(refs) => refs,
            Err(err) => {
                eprintln!("fls: {err}");
                return (LintConfig::default(), Vec::new(), ExternalHosts::empty());
            }
        };
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let external = match load_external(&refs, &cwd) {
            Ok(hosts) => hosts,
            Err(err) => {
                eprintln!("{err}");
                return (LintConfig::default(), Vec::new(), ExternalHosts::empty());
            }
        };
        let guest_ids = external.guest_ids();
        return match FlsConfig::parse_json_with_known(&text, &guest_ids) {
            Ok(config) => (config.lint, config.plugin, external),
            Err(err) => {
                eprintln!("fls: {err}");
                (LintConfig::default(), Vec::new(), external)
            }
        };
    }
    if let Ok(text) = std::fs::read_to_string("forgels.toml") {
        return match FlsConfig::parse_str(&text) {
            Ok(config) => (config.lint, config.plugin, ExternalHosts::empty()),
            Err(err) => {
                eprintln!("fls: {err}");
                (LintConfig::default(), Vec::new(), ExternalHosts::empty())
            }
        };
    }
    (LintConfig::default(), Vec::new(), ExternalHosts::empty())
}
