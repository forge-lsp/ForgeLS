use forgels_common::log;
use forgels_core::{Config, Server, routing::Language};
use forgels_lang_solidity::SolidityAnalyzer;

fn main() {
    let flag = log_level_flag();
    let (mut config, warnings) = Config::load(std::path::Path::new("forgels.toml"));
    for warning in warnings {
        eprintln!("forgels: {warning}");
    }
    let (foundry, foundry_warnings) = Config::load_foundry(std::path::Path::new("foundry.toml"));
    for warning in foundry_warnings {
        eprintln!("forgels: {warning}");
    }
    config.merge_remappings(foundry);
    let (level, warning) = log::resolve_level(flag.as_deref(), &config.log_level);
    if let Some(warning) = warning {
        eprintln!("forgels: {warning}");
    }
    log::init(level);
    log::info!("starting forgels");
    let mut server = Server::new(config);
    server.set_analyzer(Language::Solidity, Box::new(SolidityAnalyzer::new()));
    server.run();
    println!("forgels: smart contract language server");
}

/// Value of `--log-level <level>` from argv, if present.
fn log_level_flag() -> Option<String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--log-level" {
            return args.next();
        }
    }
    None
}
