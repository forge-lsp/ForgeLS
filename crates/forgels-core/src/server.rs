use forgels_common::log::{info, warn};

use crate::analyzer::Analyzer;
use crate::routing::Language;
use crate::{Config, Workspace, dispatch};

#[derive(Debug, Default)]
pub struct Server {
    config: Config,
    workspace: Workspace,
    session: dispatch::Session,
}

impl Server {
    pub fn new(config: Config) -> Self {
        let delay = config.publish_delay();
        Self {
            workspace: Workspace::with_delay(delay),
            session: dispatch::Session::new(),
            config,
        }
    }

    pub fn set_analyzer(&mut self, language: Language, analyzer: Box<dyn Analyzer>) {
        self.session.analyzers.insert(language, analyzer);
    }

    pub fn run(&mut self) {
        // Session drives initialize, shutdown, and exit; dispatch answers.
        info!("accepting lsp frames on stdio");
        let stdin = std::io::stdin();
        let stdout = std::io::stdout();
        let mut input = std::io::BufReader::new(stdin.lock());
        let mut output = stdout.lock();
        let code = dispatch::serve_session(
            &mut input,
            &mut output,
            &mut self.session,
            &mut self.workspace,
        )
        .unwrap_or(1);
        let _ = &self.config;
        let _ = &self.workspace;
        if code == 0 {
            info!("server exiting cleanly");
        } else {
            warn!("server exiting with code {code}");
        }
        std::process::exit(code);
    }
}
