use forgels_core::{Config, Server, routing::Language};
use forgels_lang_solidity::SolidityAnalyzer;

fn main() {
    let (config, warnings) = Config::load(std::path::Path::new("forgels.toml"));
    for warning in warnings {
        eprintln!("forgels: {warning}");
    }
    let mut server = Server::new(config);
    server.set_analyzer(Language::Solidity, Box::new(SolidityAnalyzer::new()));
    server.run();
    println!("forgels: smart contract language server");
}
