use forgels_common::log;
use forgels_core::{Config, Server, routing::Language};
use forgels_lang_solidity::SolidityAnalyzer;

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
