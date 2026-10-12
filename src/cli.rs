//! Command-line interface definition (clap). Argument description only;
//! the interactive side lives in [`crate::wizard`].

use clap::{Parser, Subcommand};

/// Interactive scaffolding for Rust projects.
#[derive(Debug, Parser)]
#[command(version, long_about = None)]
pub struct Cli {
    /// What to do. With no subcommand, the interactive wizard runs.
    #[command(subcommand)]
    pub(crate) command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// List the available project templates.
    List,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn verify_cli() {
        Cli::command().debug_assert();
    }
}
