use crate::{Config, Workspace, dispatch};

#[derive(Debug, Default)]
pub struct Server {
    config: Config,
    workspace: Workspace,
    session: dispatch::Session,
}

impl Server {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            workspace: Workspace::new(),
            session: dispatch::Session::new(),
        }
    }

    pub fn run(&mut self) {
        // Session owns initialize, shutdown, and exit. Unknown methods
        // stay silent until the unknown-method leaf lands.
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
        std::process::exit(code);
    }
}
