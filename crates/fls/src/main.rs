mod bridge;
mod cli;
mod config;
mod fix;
mod init;
mod lsp;
mod run;
mod walk;

use std::path::PathBuf;

use cli::{Cli, Command};

fn cwd() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn main() {
    let Cli { command } = cli::parse();
    match command {
        Command::Check(args) => {
            let root = cwd();
            let config = match config::load(&root) {
                Ok(config) => config,
                Err(message) => {
                    eprintln!("{message}");
                    std::process::exit(2);
                }
            };
            let code = run::run(&args, &root, &config);
            std::process::exit(code);
        }
        Command::Lsp(args) => {
            lsp::run(args.log_level.as_deref());
        }
        Command::Init(args) => {
            let code = init::scaffold(&cwd(), &args);
            std::process::exit(code);
        }
    }
}
