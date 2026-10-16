//! Command-line interface definition (clap). Argument description only;
//! the interactive side lives in [`crate::wizard`].

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::name::ProjectName;
use crate::spec::{Backend, Database, Deployment, Feature, Frontend, ProjectKind, Testing};

/// Interactive scaffolding for Rust projects.
#[derive(Debug, Parser)]
#[command(version, long_about = None)]
pub struct Cli {
    /// What to do. With no subcommand, the interactive wizard runs
    /// (equivalent to `rise new`).
    #[command(subcommand)]
    pub(crate) command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Create a new project; options not passed as flags are asked
    /// interactively.
    New(NewArgs),
    /// List the available project templates.
    List,
    /// Check that the prerequisites of the generated stacks (trunk, the
    /// wasm32 target, cargo-leptos, protoc, probe-rs) are installed.
    Doctor,
    /// Print a shell completion script to stdout.
    Completions {
        /// The shell to generate completions for.
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

/// `rise new` arguments. Every option the flags leave out is asked
/// interactively, unless stdin is not a terminal or `--no-input` is passed.
#[derive(Debug, Default, Clone, Args)]
pub(crate) struct NewArgs {
    /// Project name; must be a valid crate name.
    pub(crate) name: Option<ProjectName>,

    /// What kind of project to create. Inferred as `web` when any web
    /// stack flag is passed.
    #[arg(long, value_enum)]
    pub(crate) kind: Option<ProjectKind>,

    #[command(flatten)]
    pub(crate) web: WebFlags,

    /// Load the template from this directory instead of the embedded one
    /// for the selected stack.
    #[arg(long, value_name = "DIR")]
    pub(crate) template: Option<PathBuf>,

    /// Take the built-in defaults for every option the flags leave out,
    /// without asking anything (backend `axum`, database `sqlite`,
    /// deployment `docker`, no testing/features). The project name and
    /// `--kind` have no defaults and are still required.
    #[arg(long, conflicts_with = "no_input")]
    pub(crate) yes: bool,

    /// Do not ask any questions; missing options become an error.
    #[arg(long)]
    pub(crate) no_input: bool,

    /// Do not run `git init` in the generated project.
    #[arg(long)]
    pub(crate) no_git: bool,
}

/// Web stack options. `--backend` alone means a backend-only app,
/// `--frontend` alone a frontend-only one, both together a fullstack one.
#[derive(Debug, Default, Clone, Args)]
pub(crate) struct WebFlags {
    /// Web backend framework; implies a backend (or fullstack) web app.
    #[arg(long, value_enum)]
    pub(crate) backend: Option<Backend>,

    /// Frontend framework or template engine.
    #[arg(long, value_enum)]
    pub(crate) frontend: Option<Frontend>,

    /// Database / ORM layer (requires `--backend`).
    #[arg(long = "db", visible_alias = "database", value_enum)]
    pub(crate) database: Option<Database>,

    /// Deployment target; the backend's when both stacks are present.
    #[arg(long, value_enum)]
    pub(crate) deployment: Option<Deployment>,

    /// Testing tools (requires `--backend`); repeatable, comma-separated.
    #[arg(long, value_enum, value_delimiter = ',')]
    pub(crate) testing: Option<Vec<Testing>>,

    /// Optional backend features (requires `--backend`); repeatable,
    /// comma-separated.
    #[arg(long, value_enum, value_delimiter = ',')]
    pub(crate) features: Option<Vec<Feature>>,
}

impl WebFlags {
    /// Whether any web stack flag was passed.
    pub(crate) fn any_set(&self) -> bool {
        self.backend.is_some()
            || self.frontend.is_some()
            || self.database.is_some()
            || self.deployment.is_some()
            || self.testing.is_some()
            || self.features.is_some()
    }
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
