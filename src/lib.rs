//! rise — interactive scaffolding for Rust projects.

use colored::Colorize;

use crate::cli::{Command, NewArgs};
use crate::color::{FERRIS_TEAL, RUST_ORANGE, STEEL_GRAY};
use crate::spec::{Database, ProjectSpec, WebAppSpec};

mod cli;
mod color;
mod error;
mod name;
mod spec;
mod templates;
mod wizard;

pub use cli::Cli;
pub use error::Error;

/// Dispatch on the parsed command line.
///
/// # Errors
///
/// Returns [`Error::Aborted`] when the user cancels a prompt,
/// [`Error::ConflictingOptions`] on contradictory flags and
/// [`Error::MissingOptions`] when a non-interactive run lacks required
/// options. Generation can additionally fail with template or filesystem
/// errors.
pub fn run(cli: Cli) -> Result<(), Error> {
    match cli.command {
        Some(Command::List) => list_templates(),
        Some(Command::New(args)) => run_new(args),
        None => run_new(NewArgs::default()),
    }
}

/// The main mode: banner, wizard over the flag gaps, generation.
fn run_new(args: NewArgs) -> Result<(), Error> {
    print_banner();

    let (name, spec) = wizard::collect(args)?;
    let path = templates::generate(&spec, &name)?;
    init_git(&path);

    println!("created ./{}", path.display());
    let first_command = match spec {
        ProjectSpec::Library => "cargo test",
        _ => "cargo run",
    };
    println!("\nnext steps:\n  cd {name}");
    if let ProjectSpec::WebApp(WebAppSpec::Backend(backend)) = &spec {
        if backend.database == Database::SQLx {
            println!("  cp .env.example .env  # set DATABASE_URL");
        }
    }
    println!("  {first_command}");
    Ok(())
}

/// Initialize a git repository in the new project. Purely best-effort: a
/// missing or failing `git` only costs the user a manual `git init`.
fn init_git(dir: &std::path::Path) {
    let ok = std::process::Command::new("git")
        .arg("init")
        .current_dir(dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    if !ok {
        eprintln!("note: `git init` did not run; initialize git in the project manually");
    }
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
