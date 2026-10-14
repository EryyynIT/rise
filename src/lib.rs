//! rise — interactive scaffolding for Rust projects.

use colored::Colorize;

use crate::cli::{Command, NewArgs};
use crate::color::{FERRIS_TEAL, RUST_ORANGE, STEEL_GRAY};
use crate::spec::{Database, Frontend, ProjectSpec, WebAppSpec};

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
        Some(Command::Completions { shell }) => print_completions(shell),
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
    let wasm_frontend = match &spec {
        ProjectSpec::WebApp(WebAppSpec::Frontend(frontend)) => frontend.frontend == Frontend::Yew,
        ProjectSpec::WebApp(WebAppSpec::Fullstack(fullstack)) => {
            fullstack.frontend == Frontend::Yew
        }
        _ => false,
    };
    let fullstack_wasm =
        wasm_frontend && matches!(&spec, ProjectSpec::WebApp(WebAppSpec::Fullstack(_)));
    println!("\nnext steps:\n  cd {name}");
    let backend = match &spec {
        ProjectSpec::WebApp(WebAppSpec::Backend(backend)) => Some(backend),
        ProjectSpec::WebApp(WebAppSpec::Fullstack(fullstack)) => Some(&fullstack.backend),
        _ => None,
    };
    if let Some(backend) = backend {
        let dir = if fullstack_wasm { "backend/" } else { "" };
        let var = if backend.database == Database::Sled {
            "DB_PATH"
        } else {
            "DATABASE_URL"
        };
        println!("  cp {dir}.env.example {dir}.env  # set {var}");
    }
    if wasm_frontend {
        println!(
            "  rustup target add wasm32-unknown-unknown && cargo install trunk  # one-time setup"
        );
    }
    match &spec {
        ProjectSpec::Library => println!("  cargo test"),
        _ if fullstack_wasm => {
            println!("  cargo run -p {name}-backend  # terminal 1: API on :3000");
            println!("  cd frontend && trunk serve   # terminal 2: SPA on :8080");
        }
        _ if wasm_frontend => println!("  trunk serve"),
        _ => println!("  cargo run"),
    }
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
fn print_completions(shell: clap_complete::Shell) -> Result<(), Error> {
    use clap::CommandFactory;
    use std::io::Write;
    let mut command = Cli::command();
    // The package is `rise-cli`, but the installed binary is `rise`;
    // completions must register for the binary name. Render into memory
    // first: clap_complete panics on write errors, so piping into `head`
    // would crash with BrokenPipe instead of exiting quietly.
    let mut script = Vec::new();
    clap_complete::generate(shell, &mut command, "rise", &mut script);
    // clap_complete's fish generator skips positional arguments, so
    // `rise completions <TAB>` would fall back to file names; append the
    // shell list ourselves (bash and zsh generators emit it natively).
    if matches!(shell, clap_complete::Shell::Fish) {
        use clap::ValueEnum;
        let names = clap_complete::Shell::value_variants()
            .iter()
            .filter_map(|shell| shell.to_possible_value())
            .map(|value| value.get_name().to_owned())
            .collect::<Vec<_>>()
            .join(" ");
        let line = format!(
            "complete -c rise -n \"__fish_rise_using_subcommand completions\" -f -a \"{names}\" -d 'The shell to generate completions for'\n"
        );
        script.extend_from_slice(line.as_bytes());
    }
    match std::io::stdout().write_all(&script) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        Err(err) => Err(Error::Io(err)),
    }
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
