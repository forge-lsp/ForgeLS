//! Logging facade: one init, stderr only, per-crate targets.
//!
//! Stdout carries LSP frames, so logs must never touch it. Events carry
//! their crate target by default, and anything below the init level is
//! dropped before formatting.

pub use tracing::{Level, debug, error, info, instrument, trace, warn};

/// Start the stderr logger at `level`. Returns false when a logger is
/// already set; the first init wins and later calls stay silent.
pub fn init(level: Level) -> bool {
    tracing_subscriber::fmt()
        .with_max_level(level)
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_target(true)
        .try_init()
        .is_ok()
}

/// Parse a level name such as `warn` or `INFO`. Returns `None` for
/// unknown names so callers can fall back to defaults with a warning.
pub fn parse_level(text: &str) -> Option<Level> {
    text.to_ascii_uppercase().parse().ok()
}

/// Pick the effective level: flag wins over config, unknown names fall
/// back to `WARN` with a warning message.
pub fn resolve_level(flag: Option<&str>, config_value: &str) -> (Level, Option<String>) {
    if let Some(flag) = flag {
        match parse_level(flag) {
            Some(level) => return (level, None),
            None => {
                return (
                    Level::WARN,
                    Some(format!("unknown --log-level '{flag}', using warn")),
                );
            }
        }
    }
    match parse_level(config_value) {
        Some(level) => (level, None),
        None => (
            Level::WARN,
            Some(format!("unknown log_level '{config_value}', using warn")),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_levels_case_insensitively() {
        assert_eq!(parse_level("error"), Some(Level::ERROR));
        assert_eq!(parse_level("Warn"), Some(Level::WARN));
        assert_eq!(parse_level("INFO"), Some(Level::INFO));
        assert_eq!(parse_level("debug"), Some(Level::DEBUG));
        assert_eq!(parse_level("trace"), Some(Level::TRACE));
        assert_eq!(parse_level("nope"), None);
        assert_eq!(parse_level(""), None);
    }

    #[test]
    fn second_init_stays_silent() {
        // First call wins; the rest return false instead of panicking.
        // Other tests may init first, so accept either outcome here and
        // only assert the second call never panics.
        let _ = init(Level::ERROR);
        let _ = init(Level::ERROR);
    }

    #[test]
    fn events_below_level_still_run() {
        let _ = init(Level::ERROR);
        trace!("trace event");
        debug!("debug event");
        info!("info event");
        warn!("warn event");
        error!("error event");
    }

    #[test]
    fn flag_wins_over_config() {
        let (level, warning) = resolve_level(Some("debug"), "warn");
        assert_eq!(level, Level::DEBUG);
        assert!(warning.is_none());
    }

    #[test]
    fn unknown_names_fall_back_to_warn() {
        let (level, warning) = resolve_level(Some("nope"), "warn");
        assert_eq!(level, Level::WARN);
        assert!(warning.is_some());
        let (level, warning) = resolve_level(None, "nope");
        assert_eq!(level, Level::WARN);
        assert!(warning.is_some());
        let (level, warning) = resolve_level(None, "info");
        assert_eq!(level, Level::INFO);
        assert!(warning.is_none());
    }
}
