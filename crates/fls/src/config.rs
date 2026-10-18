use std::path::{Path, PathBuf};

use forgels_common::paths::Remapping;
use forgels_core::Config;
use forgels_lint::{LintConfig, LintConfigError};

/// Loaded lint setup plus import resolution base.
pub struct LoadedConfig {
    pub lint: LintConfig,
    pub root: PathBuf,
    pub remappings: Vec<Remapping>,
    pub warnings: Vec<String>,
}

/// Load `forgels.toml` next to cwd, like the server.
///
/// Missing file means defaults. Broken TOML means defaults plus a
/// stderr warning. Unknown rule IDs are operational failures.
/// Bad levels fall back to defaults plus a warning.
pub fn load(cwd: &Path) -> Result<LoadedConfig, String> {
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
