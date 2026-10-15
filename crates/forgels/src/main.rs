use forgels_core::{Config, Server};

fn main() {
    let server = Server::new(Config::new());
    server.run();
    println!("forgels: smart contract language server");
}
