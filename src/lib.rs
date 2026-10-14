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
        Some(Command::Completions { shell }) => {
            print_completions(shell);
            Ok(())
        }
        Some(Command::New(args)) => run_new(args),
        None => run_new(NewArgs::default()),
    }
}

/// The main mode: banner, wizard over the flag gaps, generation.
fn run_new(mut args: NewArgs) -> Result<(), Error> {
    print_banner();

    let source = match args.template.take() {
        Some(dir) => templates::TemplateSource::Dir(dir),
        None => templates::TemplateSource::Embedded,
    };
    let (name, spec) = wizard::collect(args)?;
    let path = templates::generate(&spec, &name, &source)?;
    init_git(&path);

    println!("created ./{}", path.display());
    let first_command = match spec {
        ProjectSpec::Library => "cargo test",
        _ => "cargo run",
    };
    println!("\nnext steps:\n  cd {name}");
    let backend = match &spec {
        ProjectSpec::WebApp(WebAppSpec::Backend(backend)) => Some(backend),
        ProjectSpec::WebApp(WebAppSpec::Fullstack(fullstack)) => Some(&fullstack.backend),
        _ => None,
    };
    if let Some(backend) = backend {
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

/// `rise completions <shell>`: print the completion script to stdout, for
/// the user to source or save into their shell's completion directory.
fn print_completions(shell: clap_complete::Shell) {
    use clap::CommandFactory;
    let mut command = Cli::command();
    // The package is `rise-cli`, but the installed binary is `rise`;
    // completions must register for the binary name.
    clap_complete::generate(shell, &mut command, "rise", &mut std::io::stdout());
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
