use std::path::{Path, PathBuf};
use std::time::Duration;

/// Publish debounce default: quiet period after the last keystroke.
pub const DEFAULT_PUBLISH_DELAY_MS: u64 = 200;

/// Server configuration. Defaults keep the server running with zero files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Debounce delay for diagnostic publishes, in milliseconds.
    pub publish_delay_ms: u64,
    /// Default workspace root when the editor sends none.
    pub root: Option<PathBuf>,
    /// Log level name such as `warn`. Validated where logging starts.
    pub log_level: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            publish_delay_ms: DEFAULT_PUBLISH_DELAY_MS,
            root: None,
            log_level: String::from("warn"),
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
        (config, warnings)
    }
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
}
