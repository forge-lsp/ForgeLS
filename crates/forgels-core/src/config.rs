use std::path::{Path, PathBuf};
use std::time::Duration;

use forgels_common::paths::{Remapping, parse_remapping};
use forgels_lint::LintConfig;

/// Publish debounce default: quiet period after the last keystroke.
pub const DEFAULT_PUBLISH_DELAY_MS: u64 = 200;

/// Deps with Foundry `src` layout: `lib/<name>/src/` holds sources.
const KNOWN_SRC_DEPS: &[&str] = &["forge-std", "ds-test"];

/// Server configuration. Defaults keep the server running with zero files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Debounce delay for diagnostic publishes, in milliseconds.
    pub publish_delay_ms: u64,
    /// Default workspace root when the editor sends none.
    pub root: Option<PathBuf>,
    /// Log level name such as `warn`. Validated where logging starts.
    pub log_level: String,
    /// Prefix remappings in Foundry `prefix=target` form.
    pub remappings: Vec<Remapping>,
    /// Lint levels plus include/exclude filters for publish.
    pub lint: LintConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            publish_delay_ms: DEFAULT_PUBLISH_DELAY_MS,
            root: None,
            log_level: String::from("warn"),
            remappings: Vec::new(),
            lint: LintConfig::default(),
        }
    }
}

impl Config {
    pub fn new() -> Self {
        Self::default()
    }

    /// Publish delay as a duration. Zero stays zero: no floor is forced.
    pub fn publish_delay(&self) -> Duration {
        Duration::from_millis(self.publish_delay_ms)
    }

    /// Load `forgels.toml` from `path`.
    ///
    /// Missing files mean defaults with no warnings. Broken files mean
    /// defaults plus one warning per problem; valid keys still apply.
    pub fn load(path: &Path) -> (Self, Vec<String>) {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(_) => return (Self::default(), Vec::new()),
        };
        let table: toml::Table = match text.parse() {
            Ok(table) => table,
            Err(error) => {
                return (
                    Self::default(),
                    vec![format!("config {} unreadable: {error}", path.display())],
                );
            }
        };
        let mut config = Self::default();
        let mut warnings = Vec::new();
        if let Some(value) = table.get("publish_delay_ms") {
            match value.as_integer() {
                Some(millis) if millis >= 0 => {
                    config.publish_delay_ms = millis as u64;
                }
                _ => warnings.push(format!(
                    "config {}: publish_delay_ms must be a non-negative integer",
                    path.display()
                )),
            }
        }
        if let Some(value) = table.get("root") {
            match value.as_str() {
                Some(root) => {
                    config.root = Some(PathBuf::from(root));
                }
                None => warnings.push(format!(
                    "config {}: root must be a string path",
                    path.display()
                )),
            }
        }
        if let Some(value) = table.get("log_level") {
            match value.as_str() {
                Some(level) => {
                    config.log_level = level.to_string();
                }
                None => warnings.push(format!(
                    "config {}: log_level must be a string",
                    path.display()
                )),
            }
        }
        let (mapped, mut map_warnings) = collect_remappings(table.get("remappings"), path);
        config.remappings = mapped;
        warnings.append(&mut map_warnings);
        (config, warnings)
    }

    /// Load `foundry.toml` remappings with `libs` fallback.
    ///
    /// Missing files mean defaults with no warnings. Explicit `remappings`
    /// win; when absent, known `src` deps resolve to `lib/<name>/src/`,
    /// then each remaining subdir of every `libs` dir becomes
    /// `name/=lib/name/`. Checks top level then `profile.default`.
    pub fn load_foundry(path: &Path) -> (Self, Vec<String>) {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(_) => return (Self::default(), Vec::new()),
        };
        let table: toml::Table = match text.parse() {
            Ok(table) => table,
            Err(error) => {
                return (
                    Self::default(),
                    vec![format!("config {} unreadable: {error}", path.display())],
                );
            }
        };
        let mut config = Self::default();
        let mut warnings = Vec::new();
        let raw = pick_foundry_value(&table, "remappings");
        let (mapped, mut map_warnings) = collect_remappings(raw, path);
        warnings.append(&mut map_warnings);
        if !mapped.is_empty() {
            config.remappings = mapped;
            return (config, warnings);
        }
        let libs = pick_foundry_strings(&table, "libs").unwrap_or(vec![String::from("lib")]);
        let base = path.parent().unwrap_or(Path::new("."));
        let mut known = default_known_remappings(base, &libs);
        let mut auto = auto_remappings(base, &libs);
        auto.retain(|item| {
            !is_known_src(&item.prefix) && !known.iter().any(|keep| keep.prefix == item.prefix)
        });
        known.extend(auto);
        known.sort_by(|a, b| a.prefix.cmp(&b.prefix));
        config.remappings = known;
        (config, warnings)
    }

    /// Fill empty remappings from `fallback`. Explicit entries always win.
    pub fn merge_remappings(&mut self, fallback: Self) {
        if self.remappings.is_empty() {
            self.remappings = fallback.remappings;
        }
    }
}

/// Collect `prefix=target` strings from a TOML array value.
fn collect_remappings(value: Option<&toml::Value>, path: &Path) -> (Vec<Remapping>, Vec<String>) {
    let Some(list) = value.and_then(toml::Value::as_array) else {
        if value.is_some() {
            return (
                Vec::new(),
                vec![format!(
                    "config {}: remappings must be an array of \"prefix=target\" strings",
                    path.display()
                )],
            );
        }
        return (Vec::new(), Vec::new());
    };
    let mut out = Vec::new();
    let mut warnings = Vec::new();
    for item in list {
        let Some(raw) = item.as_str() else {
            warnings.push(format!(
                "config {}: remapping entry must be a string",
                path.display()
            ));
            continue;
        };
        match parse_remapping(raw) {
            Some(mapped) => out.push(mapped),
            None => warnings.push(format!(
                "config {}: invalid remapping \"{raw}\", expected \"prefix=target\"",
                path.display()
            )),
        }
    }
    (out, warnings)
}

/// Foundry nests keys under `profile.default`; top level wins first.
fn pick_foundry_value<'a>(table: &'a toml::Table, key: &str) -> Option<&'a toml::Value> {
    if let Some(value) = table.get(key) {
        return Some(value);
    }
    table
        .get("profile")
        .and_then(|profile| profile.get("default"))
        .and_then(|defaults| defaults.get(key))
}

/// String array for a Foundry key, top level then `profile.default`.
fn pick_foundry_strings(table: &toml::Table, key: &str) -> Option<Vec<String>> {
    let value = pick_foundry_value(table, key)?;
    let list = value.as_array()?;
    let mut out = Vec::new();
    for item in list {
        out.push(item.as_str()?.to_string());
    }
    Some(out)
}

/// Known `src` remappings: `name/` points at `lib/name/src/` when present.
/// Respects custom `libs`, skips missing `src` silently, first wins.
fn default_known_remappings(base: &Path, libs: &[String]) -> Vec<Remapping> {
    let mut out: Vec<Remapping> = Vec::new();
    for lib in libs {
        let dir = if Path::new(lib).is_absolute() {
            PathBuf::from(lib)
        } else {
            base.join(lib)
        };
        for &name in KNOWN_SRC_DEPS {
            let prefix = format!("{name}/");
            if out.iter().any(|item| item.prefix == prefix) {
                continue;
            }
            if dir.join(name).join("src").is_dir() {
                out.push(Remapping::new(
                    prefix,
                    PathBuf::from(lib).join(name).join("src"),
                ));
            }
        }
    }
    out.sort_by(|a, b| a.prefix.cmp(&b.prefix));
    out
}

/// True when `prefix` names a known `src` dep such as `forge-std/`.
fn is_known_src(prefix: &str) -> bool {
    KNOWN_SRC_DEPS
        .iter()
        .any(|name| prefix == format!("{name}/"))
}

/// Auto remappings from `libs` dirs: each immediate subdir `name` in
/// `lib` becomes `name/=lib/name/`. Missing dirs are skipped silently.
fn auto_remappings(base: &Path, libs: &[String]) -> Vec<Remapping> {
    let mut out = Vec::new();
    for lib in libs {
        let dir = if Path::new(lib).is_absolute() {
            PathBuf::from(lib)
        } else {
            base.join(lib)
        };
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                let target = PathBuf::from(lib).join(&name);
                out.push(Remapping::new(format!("{name}/"), target));
            }
        }
    }
    out.sort_by(|a, b| a.prefix.cmp(&b.prefix));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_means_defaults_without_warnings() {
        let (config, warnings) = Config::load(Path::new("/no/such/forgels.toml"));
        assert_eq!(config, Config::default());
        assert!(warnings.is_empty());
        assert_eq!(config.publish_delay(), Duration::from_millis(200));
    }

    #[test]
    fn valid_file_applies_keys() {
        let dir = std::env::temp_dir();
        let path = dir.join("forgels-valid-test.toml");
        std::fs::write(&path, "publish_delay_ms = 50\nroot = \"/repo\"\n").expect("write");
        let (config, warnings) = Config::load(&path);
        assert!(warnings.is_empty());
        assert_eq!(config.publish_delay_ms, 50);
        assert_eq!(config.root, Some(PathBuf::from("/repo")));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn broken_file_falls_back_with_warning() {
        let dir = std::env::temp_dir();
        let path = dir.join("forgels-broken-test.toml");
        std::fs::write(&path, "publish_delay_ms = [\n").expect("write");
        let (config, warnings) = Config::load(&path);
        assert_eq!(config, Config::default());
        assert_eq!(warnings.len(), 1);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn bad_keys_warn_but_valid_keys_apply() {
        let dir = std::env::temp_dir();
        let path = dir.join("forgels-partial-test.toml");
        std::fs::write(&path, "publish_delay_ms = \"soon\"\nroot = \"/repo\"\n").expect("write");
        let (config, warnings) = Config::load(&path);
        assert_eq!(config.publish_delay_ms, DEFAULT_PUBLISH_DELAY_MS);
        assert_eq!(config.root, Some(PathBuf::from("/repo")));
        assert_eq!(warnings.len(), 1);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn known_src_layout_resolves() {
        let root = std::env::temp_dir().join(format!("forgels-known-{}-a", std::process::id()));
        std::fs::create_dir_all(root.join("lib/forge-std/src")).expect("mkdir");
        std::fs::write(root.join("foundry.toml"), "[profile.default]\n").expect("write");
        let (config, warnings) = Config::load_foundry(&root.join("foundry.toml"));
        assert!(warnings.is_empty());
        assert!(
            config
                .remappings
                .iter()
                .any(|item| item.prefix == "forge-std/"
                    && item.target.as_path() == Path::new("lib/forge-std/src"))
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn explicit_remappings_win_over_known() {
        let root = std::env::temp_dir().join(format!("forgels-known-{}-b", std::process::id()));
        std::fs::create_dir_all(root.join("lib/forge-std/src")).expect("mkdir");
        std::fs::write(
            root.join("foundry.toml"),
            "remappings = [\"forge-std/=custom/std/\"]\n",
        )
        .expect("write");
        let (config, warnings) = Config::load_foundry(&root.join("foundry.toml"));
        assert!(warnings.is_empty());
        assert_eq!(config.remappings.len(), 1);
        assert_eq!(config.remappings[0].prefix, "forge-std/");
        assert_eq!(config.remappings[0].target, PathBuf::from("custom/std/"));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn missing_src_stays_silent() {
        let root = std::env::temp_dir().join(format!("forgels-known-{}-c", std::process::id()));
        std::fs::create_dir_all(root.join("lib/forge-std")).expect("mkdir");
        std::fs::write(root.join("foundry.toml"), "[profile.default]\n").expect("write");
        let (config, warnings) = Config::load_foundry(&root.join("foundry.toml"));
        assert!(warnings.is_empty());
        assert!(config.remappings.is_empty());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn custom_libs_honored_for_known() {
        let root = std::env::temp_dir().join(format!("forgels-known-{}-d", std::process::id()));
        std::fs::create_dir_all(root.join("custom/forge-std/src")).expect("mkdir");
        std::fs::write(root.join("foundry.toml"), "libs = [\"custom\"]\n").expect("write");
        let (config, warnings) = Config::load_foundry(&root.join("foundry.toml"));
        assert!(warnings.is_empty());
        assert!(
            config
                .remappings
                .iter()
                .any(|item| item.prefix == "forge-std/"
                    && item.target.as_path() == Path::new("custom/forge-std/src"))
        );
        std::fs::remove_dir_all(&root).ok();
    }
}
