use crate::cli::project::prompt_project_config;
use crate::color::{FERRIS_TEAL, RUST_ORANGE, STEEL_GRAY};

use colored::Colorize;

mod cli;
mod color;
mod config;
mod error;
mod templates;

pub use error::Error;

pub fn run() -> Result<(), Error> {
    println!(
        "{}  {}\n    {}",
        "🦀".custom_color(RUST_ORANGE),
        "rise (rust initial setup engine)".custom_color(FERRIS_TEAL),
        "fast, idiomatic, async Rust scaffolding".custom_color(STEEL_GRAY)
    );

    let _project = prompt_project_config()?;

    Ok(())
}
