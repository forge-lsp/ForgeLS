//! External WASM plugins. Loaded once at startup, reused per file.
//!
//! Sandbox per call is 10M fuel, 64 MiB memory, 2s epoch timeout.
//! Guests get zero host imports, so fs, net, and clocks stay denied.
//! Engine plus compiled modules are reused across files. No
//! hot-reload: restart the binary to pick up plugin edits.

pub mod abi;
pub mod types;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use forgels_fmt::{FormatEdit, FormatOptions};
use forgels_lint::{ExternalRef, LintDiagnostic, RuleId};
use wasmtime::{Config, Engine, Instance, Module, Store, StoreLimits, StoreLimitsBuilder};

/// Fuel per guest call. Bounds CPU for lint and format.
pub const FUEL_PER_CALL: u64 = 10_000_000;
/// Guest linear memory cap. Bounds allocation per store.
pub const MEMORY_LIMIT_BYTES: usize = 64 * 1024 * 1024;
/// Wall clock bound per guest call via epoch interruption.
pub const EPOCH_TIMEOUT_SECS: u64 = 2;

/// One compiled guest. Module reuse avoids recompile per file.
#[derive(Debug, Clone)]
pub struct CompiledGuest {
    pub path: PathBuf,
    pub module: Module,
    pub rules: Vec<String>,
}

/// Loaded external hosts. Empty when no `lint.external` is set.
#[derive(Debug)]
pub struct ExternalHosts {
    pub engine: Engine,
    pub guests: Vec<CompiledGuest>,
    pub rule_cache: HashMap<String, RuleId>,
}

impl ExternalHosts {
    /// Empty hosts: builtins run byte-identical with no guests.
    pub fn empty() -> Self {
        let mut config = Config::new();
        config.consume_fuel(true);
        config.epoch_interruption(true);
        let engine = Engine::new(&config).unwrap_or_else(|_| Engine::default());
        Self {
            engine,
            guests: Vec::new(),
            rule_cache: HashMap::new(),
        }
    }

    /// True when at least one guest is loaded.
    pub fn has_guests(&self) -> bool {
        !self.guests.is_empty()
    }

    /// All declared guest rule IDs across guests.
    pub fn guest_ids(&self) -> Vec<String> {
        let mut out = Vec::new();
        for guest in &self.guests {
            for id in &guest.rules {
                if !out.contains(id) {
                    out.push(id.clone());
                }
            }
        }
        out
    }
}

/// True for URL-like refs. Only local paths load.
fn is_url_ref(raw: &str) -> bool {
    raw.contains("://") || raw.starts_with("http:") || raw.starts_with("https:")
}

fn sandbox_engine() -> Result<Engine, String> {
    let mut config = Config::new();
    config.consume_fuel(true);
    config.epoch_interruption(true);
    Engine::new(&config).map_err(|err| format!("fls: external engine: {err}"))
}

fn sandbox_store(engine: &Engine) -> Result<Store<StoreLimits>, String> {
    let limits = StoreLimitsBuilder::new()
        .memory_size(MEMORY_LIMIT_BYTES)
        .build();
    let mut store = Store::new(engine, limits);
    store.limiter(|limits| limits as &mut dyn wasmtime::ResourceLimiter);
    store
        .set_fuel(FUEL_PER_CALL)
        .map_err(|err| format!("fls: external fuel: {err}"))?;
    store.set_epoch_deadline(1);
    Ok(store)
}

fn arm_epoch_timeout(engine: &Engine) {
    let engine = engine.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(EPOCH_TIMEOUT_SECS));
        engine.increment_epoch();
    });
}

fn instantiate(
    store: &mut Store<StoreLimits>,
    module: &Module,
    path: &str,
) -> Result<Instance, String> {
    if module.imports().next().is_some() {
        return Err(format!(
            "fls: external plugin {path}: imports denied (no fs, net, or clocks)"
        ));
    }
    Instance::new(&mut *store, module, &[])
        .map_err(|err| format!("fls: external plugin {path}: instantiate failed: {err}"))
}

/// Load external guests from `lint.external`. Canonicalizes local
/// paths only and rejects URLs. Compiles once with a shared engine.
/// All wasmtime failures become `Err(String)` for exit 2.
pub fn load_external(refs: &[ExternalRef], cwd: &Path) -> Result<ExternalHosts, String> {
    if refs.is_empty() {
        return Ok(ExternalHosts::empty());
    }
    let engine = sandbox_engine()?;
    let mut guests = Vec::new();
    let mut rule_cache: HashMap<String, RuleId> = HashMap::new();
    for item in refs {
        if is_url_ref(&item.path) {
            return Err(format!(
                "fls: external plugin {}: urls denied, use a local path",
                item.path
            ));
        }
        let joined = if Path::new(&item.path).is_absolute() {
            PathBuf::from(&item.path)
        } else {
            cwd.join(&item.path)
        };
        let canonical = joined
            .canonicalize()
            .map_err(|_| format!("fls: external plugin {}: cannot read file", item.path))?;
        let bytes = std::fs::read(&canonical)
            .map_err(|_| format!("fls: external plugin {}: cannot read file", item.path))?;
        let display = canonical.to_string_lossy().into_owned();
        let module = Module::new(&engine, &bytes)
            .map_err(|err| format!("fls: external plugin {display}: compile failed: {err}"))?;
        if module.imports().next().is_some() {
            return Err(format!(
                "fls: external plugin {display}: imports denied (no fs, net, or clocks)"
            ));
        }
        let mut store = sandbox_store(&engine)?;
        arm_epoch_timeout(&engine);
        let instance = instantiate(&mut store, &module, &display)?;
        abi::check_abi_version(&mut store, &instance, &display)?;
        let rules = abi::call_rules(&mut store, &instance, &display)?;
        for id in &rules {
            if !RuleId::is_guest_id(id) {
                return Err(format!("fls: external plugin {display}: bad rule id {id}"));
            }
            if RuleId::all().iter().any(|known| known.as_str() == id) {
                continue;
            }
            if !rule_cache.contains_key(id) {
                let leaked: &'static str = Box::leak(id.clone().into_boxed_str());
                rule_cache.insert(id.clone(), RuleId::from_leaked(leaked));
            }
        }
        guests.push(CompiledGuest {
            path: canonical,
            module,
            rules,
        });
    }
    Ok(ExternalHosts {
        engine,
        guests,
        rule_cache,
    })
}

/// Run guest lint for one file and return guest diagnostics only.
/// Skips on any guest error so builtins still report.
pub fn guest_diags(hosts: &mut ExternalHosts, path: &Path, text: &str) -> Vec<LintDiagnostic> {
    if hosts.guests.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let display = path.to_string_lossy().into_owned();
    let request = types::lint_request_json(&display, text);
    let file_len = u32::try_from(text.len()).unwrap_or(u32::MAX);
    for guest in hosts.guests.clone() {
        let guest_path = guest.path.to_string_lossy().into_owned();
        let Ok(mut store) = sandbox_store(&hosts.engine) else {
            continue;
        };
        arm_epoch_timeout(&hosts.engine);
        let Ok(instance) = instantiate(&mut store, &guest.module, &guest_path) else {
            continue;
        };
        let Ok(output) = abi::call_lint(&mut store, &instance, &guest_path, &request) else {
            continue;
        };
        let Ok(diags) = types::parse_lint_output(&output, file_len, &mut hosts.rule_cache) else {
            continue;
        };
        out.extend(diags);
    }
    out
}

/// Run guest lint for one file. Skips on any guest error so builtins
/// still report. Caller runs the single `resolve()` afterwards.
pub fn lint_with_guests(
    hosts: &mut ExternalHosts,
    path: &Path,
    text: &str,
    mut base: Vec<LintDiagnostic>,
) -> Vec<LintDiagnostic> {
    if hosts.guests.is_empty() {
        return base;
    }
    base.extend(guest_diags(hosts, path, text));
    base
}

/// Run guest format for one file. Returns guest edits only; the
/// caller applies them inside the builtin fixpoint.
pub fn format_with_guests(
    hosts: &mut ExternalHosts,
    path: &Path,
    text: &str,
    options: &FormatOptions,
) -> Vec<FormatEdit> {
    if hosts.guests.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let display = path.to_string_lossy().into_owned();
    let style = options.style.as_str().to_string();
    let request = types::format_request_json(
        &display,
        text,
        options.line_length,
        options.indent_width,
        &style,
    );
    let file_len = u32::try_from(text.len()).unwrap_or(u32::MAX);
    for guest in hosts.guests.clone() {
        let guest_path = guest.path.to_string_lossy().into_owned();
        let Ok(mut store) = sandbox_store(&hosts.engine) else {
            continue;
        };
        arm_epoch_timeout(&hosts.engine);
        let Ok(instance) = instantiate(&mut store, &guest.module, &guest_path) else {
            continue;
        };
        let Ok(output) = abi::call_format(&mut store, &instance, &guest_path, &request) else {
            continue;
        };
        let Ok(mut edits) = types::parse_format_output(&output, file_len) else {
            continue;
        };
        out.append(&mut edits);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_refs_are_rejected() {
        assert!(is_url_ref("https://example.com/p.wasm"));
        assert!(is_url_ref("http://example.com/p.wasm"));
        assert!(!is_url_ref("./plugins/p.wasm"));
        assert!(!is_url_ref("/abs/p.wasm"));
    }

    #[test]
    fn empty_hosts_have_no_guests() {
        let hosts = ExternalHosts::empty();
        assert!(!hosts.has_guests());
        assert!(hosts.guest_ids().is_empty());
    }

    #[test]
    fn sandbox_numbers_match_spec() {
        assert_eq!(FUEL_PER_CALL, 10_000_000);
        assert_eq!(MEMORY_LIMIT_BYTES, 64 * 1024 * 1024);
        assert_eq!(EPOCH_TIMEOUT_SECS, 2);
    }

    #[test]
    fn missing_file_fails_cleanly() {
        let cwd = std::env::temp_dir();
        let refs = vec![ExternalRef {
            path: "no-such-plugin-xyz.wasm".to_string(),
        }];
        assert!(load_external(&refs, &cwd).is_err());
    }

    #[test]
    fn wasi_import_module_fails_cleanly() {
        let wat_text = r#"(module (import "wasi_snapshot_preview1" "fd_write" (func (param i32 i32 i32 i32) (result i32))) (memory (export "memory") 1))"#;
        let bytes = wat::parse_str(wat_text).expect("wat parses");
        let engine = sandbox_engine().expect("engine");
        let module = Module::new(&engine, &bytes).expect("module compiles");
        assert!(module.imports().next().is_some());
        let mut store = sandbox_store(&engine).expect("store");
        assert!(instantiate(&mut store, &module, "wasi-test").is_err());
    }

    #[test]
    fn abi_mismatch_rejects_guest() {
        let wat_text = r#"(module (memory (export "memory") 1) (func (export "forgels_plugin_abi_version") (result i32) i32.const 2) (func (export "alloc") (param i32) (result i32) i32.const 0) (func (export "free") (param i32 i32)) (func (export "lint") (param i32 i32) (result i64) i64.const 0) (func (export "format") (param i32 i32) (result i64) i64.const 0))"#;
        let bytes = wat::parse_str(wat_text).expect("wat parses");
        let engine = sandbox_engine().expect("engine");
        let module = Module::new(&engine, &bytes).expect("module compiles");
        let mut store = sandbox_store(&engine).expect("store");
        let instance = instantiate(&mut store, &module, "abi-test").expect("instantiates");
        assert!(abi::check_abi_version(&mut store, &instance, "abi-test").is_err());
    }
}
