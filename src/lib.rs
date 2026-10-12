//! rise — interactive scaffolding for Rust projects.

use colored::Colorize;

use crate::color::{FERRIS_TEAL, RUST_ORANGE, STEEL_GRAY};
use crate::wizard::project::prompt_project_spec;

mod color;
mod error;
mod spec;
mod templates;
mod wizard;

pub use error::Error;

/// Placeholder project name until stage 3 adds the name prompt and `--name`.
const PLACEHOLDER_NAME: &str = "my-app";

/// Run the interactive wizard, then generate the project on disk.
///
/// # Errors
///
/// Returns [`Error::Aborted`] when the user cancels a prompt, or
/// [`Error::Prompt`] when a prompt fails (e.g. stdin is not a TTY).
/// Generation can additionally fail with template or filesystem errors.
pub fn run() -> Result<(), Error> {
    println!(
        "{}  {}\n    {}",
        "🦀".custom_color(RUST_ORANGE),
        "rise (rust initial setup engine)".custom_color(FERRIS_TEAL),
        "fast, idiomatic, async Rust scaffolding".custom_color(STEEL_GRAY)
    );

    let spec = prompt_project_spec()?;
    let path = templates::generate(&spec, PLACEHOLDER_NAME)?;

    println!("created ./{}", path.display());
    Ok(())
}
