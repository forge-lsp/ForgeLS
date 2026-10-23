use std::path::{Path, PathBuf};

use forgels_common::paths::Remapping;
use forgels_core::Config;
use forgels_fmt::{FormatOptions, IndentStyle};
use forgels_lint::{FlsConfig, FormatConfig, LintConfig, LintConfigError};

use super::external::{ExternalHosts, load_external};

/// Loaded lint setup plus import resolution base.
pub struct LoadedConfig {
    pub lint: LintConfig,
    pub format: FormatConfig,
    pub root: PathBuf,
    pub remappings: Vec<Remapping>,
    pub warnings: Vec<String>,
    pub external: ExternalHosts,
}

/// Map typed format config to engine options. Missing keys fall back
/// to engine defaults (120 columns, width 4, space indent).
pub fn format_options_from(config: &FormatConfig) -> FormatOptions {
    let base = FormatOptions::default();
    let line_length = config.line_length.unwrap_or(base.line_length);
    let indent_width = config.tab_width.unwrap_or(base.indent_width);
    let style = config
        .style
        .as_deref()
        .and_then(IndentStyle::parse)
        .unwrap_or(base.style);
    FormatOptions {
        line_length: if line_length == 0 {
            base.line_length
        } else {
            line_length
        },
        indent_width: if indent_width == 0 {
            base.indent_width
        } else {
            indent_width
        },
        style,
    }
}

/// Load grouped config with cwd-only discovery (no upward search).
///
/// Precedence is `fls.json` wins, else `forgels.toml` lint text, else
/// defaults. Broken `fls.json` means defaults plus one warning and the
/// run continues. Unknown rule IDs are operational failures.
pub fn load(cwd: &Path) -> Result<LoadedConfig, String> {
    let fls_path = cwd.join("fls.json");
    if fls_path.exists() {
        return load_fls_json(cwd, &fls_path);
    }
    load_legacy_toml(cwd)
}

fn core_base(cwd: &Path) -> (PathBuf, Vec<Remapping>, Vec<String>) {
    let path = cwd.join("forgels.toml");
    let (core, warnings) = Config::load(&path);
    let root = core.root.clone().unwrap_or_else(|| cwd.to_path_buf());
    (root, core.remappings.clone(), warnings)
}

fn load_fls_json(cwd: &Path, path: &Path) -> Result<LoadedConfig, String> {
    let (root, remappings, mut warnings) = core_base(cwd);
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            warnings.push(format!("fls: cannot read {}: {err}", path.display()));
            return Ok(LoadedConfig {
                lint: LintConfig::default(),
                format: FormatConfig::default(),
                root,
                remappings,
                warnings,
                external: ExternalHosts::empty(),
            });
        }
    };
    // Startup order: read external paths leniently, load guests once,
    // collect guest IDs, then validate rules. Truly unknown stays
    // exit 2. No hot-reload: restart to pick up plugin edits.
    let refs = match FlsConfig::parse_external_refs(&text) {
        Ok(refs) => refs,
        Err(other) => {
            warnings.push(format!("fls: {other}"));
            return Ok(LoadedConfig {
                lint: LintConfig::default(),
                format: FormatConfig::default(),
                root,
                remappings,
                warnings,
                external: ExternalHosts::empty(),
            });
        }
    };
    let external = load_external(&refs, cwd)?;
    let guest_ids = external.guest_ids();
    match FlsConfig::parse_json_with_known(&text, &guest_ids) {
        Ok(config) => Ok(LoadedConfig {
            lint: config.lint,
            format: config.format,
            root,
            remappings,
            warnings,
            external,
        }),
        Err(LintConfigError::UnknownRule(id)) => Err(format!("fls: unknown lint rule: {id}")),
        Err(other) => {
            warnings.push(format!("fls: {other}"));
            Ok(LoadedConfig {
                lint: LintConfig::default(),
                format: FormatConfig::default(),
                root,
                remappings,
                warnings,
                external,
            })
        }
    }
}

fn load_legacy_toml(cwd: &Path) -> Result<LoadedConfig, String> {
    // Legacy TOML carries lint only and always yields default format.
    // Typed format keys live in fls.json going forward.
    let path = cwd.join("forgels.toml");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(_) => {
            return Ok(LoadedConfig {
                lint: LintConfig::default(),
                format: FormatConfig::default(),
                root: cwd.to_path_buf(),
                remappings: Vec::new(),
                warnings: Vec::new(),
                external: ExternalHosts::empty(),
            });
        }
    };
    let (core, mut warnings) = Config::load(&path);
    let root = core.root.clone().unwrap_or_else(|| cwd.to_path_buf());
    let remappings = core.remappings.clone();
    let lint = match LintConfig::parse_str(&text) {
        Ok(config) => config,
        Err(LintConfigError::UnknownRule(id)) => {
            return Err(format!("fls: unknown lint rule: {id}"));
        }
        Err(other) => {
            warnings.push(format!("fls: {other}"));
            LintConfig::default()
        }
    };
    Ok(LoadedConfig {
        lint,
        format: FormatConfig::default(),
        root,
        remappings,
        warnings,
        external: ExternalHosts::empty(),
    })
}
