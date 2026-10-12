//! rise — interactive scaffolding for Rust projects.

use colored::Colorize;

use crate::cli::Command;
use crate::color::{FERRIS_TEAL, RUST_ORANGE, STEEL_GRAY};
use crate::name::ProjectName;
use crate::wizard::project::prompt_project_spec;

mod cli;
mod color;
mod error;
mod name;
mod spec;
mod templates;
mod wizard;

pub use cli::Cli;
pub use error::Error;

/// Placeholder project name until the wizard asks for one.
const PLACEHOLDER_NAME: &str = "my-app";

/// Dispatch on the parsed command line.
///
/// # Errors
///
/// Returns [`Error::Aborted`] when the user cancels a prompt, or
/// [`Error::Prompt`] when a prompt fails (e.g. stdin is not a TTY).
/// Generation can additionally fail with template or filesystem errors.
pub fn run(cli: Cli) -> Result<(), Error> {
    match cli.command {
        Some(Command::List) => list_templates(),
        None => run_wizard(),
    }
}

/// The default mode: banner, interactive wizard, generation.
fn run_wizard() -> Result<(), Error> {
    print_banner();

    let spec = prompt_project_spec()?;
    let name = ProjectName::new(PLACEHOLDER_NAME.to_owned())
        .expect("the placeholder name is a valid crate name");
    let path = templates::generate(&spec, &name)?;

    println!("created ./{}", path.display());
    Ok(())
}

/// `rise list`: print the embedded templates.
fn list_templates() -> Result<(), Error> {
    println!("available templates:");
    for manifest in templates::list_embedded()? {
        println!("  {} — {}", manifest.name, manifest.description);
    }
    Ok(())
}

/// The rise banner: crab, name, tagline.
fn print_banner() {
    println!(
        "{}  {}\n    {}",
        "🦀".custom_color(RUST_ORANGE),
        "rise (rust initial setup engine)".custom_color(FERRIS_TEAL),
        "fast, idiomatic, async Rust scaffolding".custom_color(STEEL_GRAY)
    );
}
