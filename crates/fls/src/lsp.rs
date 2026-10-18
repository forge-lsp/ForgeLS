use forgels_common::log;
use forgels_core::{Config, Server, routing::Language};
use forgels_lang_solidity::SolidityAnalyzer;
use forgels_lint::{FlsConfig, LintConfig};

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
    // No watcher: lint config loads once at startup, restart to pick up edits.
    config.lint = load_lint();
    let (level, warning) = log::resolve_level(level_flag, &config.log_level);
    if let Some(warning) = warning {
        eprintln!("fls: {warning}");
    }
    log::init(level);
    log::info!("starting fls lsp");
    let mut server = Server::new(config);
    server.set_analyzer(Language::Solidity, Box::new(SolidityAnalyzer::new()));
    server.run();
}

// Prefer fls.json, fall back to forgels.toml lint text, else defaults.
// Broken files warn and run with defaults so the server never fails open.
fn load_lint() -> LintConfig {
    if let Ok(text) = std::fs::read_to_string("fls.json") {
        return match FlsConfig::parse_json(&text) {
            Ok(config) => config.lint,
            Err(err) => {
                eprintln!("fls: {err}");
                LintConfig::default()
            }
        };
    }
    if let Ok(text) = std::fs::read_to_string("forgels.toml") {
        return match LintConfig::parse_str(&text) {
            Ok(config) => config,
            Err(err) => {
                eprintln!("fls: {err}");
                LintConfig::default()
            }
        };
    }
    LintConfig::default()
}
