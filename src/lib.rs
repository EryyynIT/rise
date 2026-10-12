//! rise — interactive scaffolding for Rust projects.

use colored::Colorize;

use crate::cli::project::prompt_project_spec;
use crate::color::{FERRIS_TEAL, RUST_ORANGE, STEEL_GRAY};

mod cli;
mod color;
mod error;
mod spec;
mod templates;

pub use error::Error;

/// Run the interactive wizard.
///
/// # Errors
///
/// Returns [`Error::Aborted`] when the user cancels a prompt, or
/// [`Error::Prompt`] when a prompt fails (e.g. stdin is not a TTY).
pub fn run() -> Result<(), Error> {
    println!(
        "{}  {}\n    {}",
        "🦀".custom_color(RUST_ORANGE),
        "rise (rust initial setup engine)".custom_color(FERRIS_TEAL),
        "fast, idiomatic, async Rust scaffolding".custom_color(STEEL_GRAY)
    );

    let _spec = prompt_project_spec()?;

    Ok(())
}
