use crate::{Config, Workspace};

#[derive(Debug, Default)]
pub struct Server {
    config: Config,
    workspace: Workspace,
}

impl Server {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            workspace: Workspace::new(),
        }
    }

    pub fn run(&self) {
        // TODO: wire transport (stdio/tcp) via forgels-protocol.
        let _ = &self.config;
        let _ = &self.workspace;
    }
}
