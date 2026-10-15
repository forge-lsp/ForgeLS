//! forgels-core: server lifecycle, workspace manager, config, diagnostics hub.
//!
//! This crate owns cross language orchestration. It must not contain
//! language specific grammar. It dispatches to `forgels-lang-*` crates.

pub mod config;
pub mod diagnostics;
pub mod dispatch;
pub mod routing;
pub mod server;
pub mod transport;
pub mod workspace;

pub use config::Config;
pub use server::Server;
pub use workspace::Workspace;
