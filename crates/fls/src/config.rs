use std::path::{Path, PathBuf};

use forgels_common::paths::Remapping;
use forgels_core::Config;
use forgels_lint::{FlsConfig, LintConfig, LintConfigError};

/// Loaded lint setup plus import resolution base.
pub struct LoadedConfig {
    pub lint: LintConfig,
    pub root: PathBuf,
    pub remappings: Vec<Remapping>,
    pub warnings: Vec<String>,
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
                root,
                remappings,
                warnings,
            });
        }
    };
    match FlsConfig::parse_json(&text) {
        Ok(config) => Ok(LoadedConfig {
            lint: config.lint,
            root,
            remappings,
            warnings,
        }),
        Err(LintConfigError::UnknownRule(id)) => Err(format!("fls: unknown lint rule: {id}")),
        Err(other) => {
            warnings.push(format!("fls: {other}"));
            Ok(LoadedConfig {
                lint: LintConfig::default(),
                root,
                remappings,
                warnings,
            })
        }
    }
}

fn load_legacy_toml(cwd: &Path) -> Result<LoadedConfig, String> {
    let path = cwd.join("forgels.toml");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(_) => {
            return Ok(LoadedConfig {
                lint: LintConfig::default(),
                root: cwd.to_path_buf(),
                remappings: Vec::new(),
                warnings: Vec::new(),
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
        root,
        remappings,
        warnings,
    })
}
