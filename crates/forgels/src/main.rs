use forgels_core::{Config, Server, routing::Language};
use forgels_lang_solidity::SolidityAnalyzer;

fn main() {
    let mut server = Server::new(Config::new());
    server.set_analyzer(Language::Solidity, Box::new(SolidityAnalyzer::new()));
    server.run();
    println!("forgels: smart contract language server");
}
