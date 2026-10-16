//! Interactive wizard: fills the gaps the CLI flags left in the spec.

mod render;
mod web;

use std::fmt::Display;
use std::io::IsTerminal;
use std::path::Path;

use inquire::error::CustomUserError;
use inquire::validator::Validation;
use inquire::{MultiSelect, Select, Text};

use crate::cli::{NewArgs, WebFlags};
use crate::error::{Error, Result};
use crate::name::ProjectName;
use crate::spec::{Database, ProjectKind, ProjectSpec, Testing};
use crate::wizard::render::render_cfg;

/// Whether the wizard may ask questions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Interaction {
    /// Missing options are prompted for.
    Interactive,
    /// Missing options take their built-in defaults (`--yes`); nothing is
    /// asked regardless of the terminal.
    Defaults,
    /// Missing options become [`Error::MissingOptions`].
    NonInteractive,
}

/// Fill every gap the CLI flags left and return a complete, validated spec.
///
/// # Errors
///
/// [`Error::ConflictingOptions`] on contradictory flags,
/// [`Error::MissingOptions`] when a required option without a default is
/// missing in a non-interactive or `--yes` run, [`Error::Aborted`]
/// /[`Error::Prompt`] from prompts.
pub(crate) fn collect(args: NewArgs) -> Result<(ProjectName, ProjectSpec)> {
    validate_flags(&args)?;

    let mode = if args.yes {
        // `--yes` never asks, on a terminal or not.
        Interaction::Defaults
    } else if std::io::stdin().is_terminal() && !args.no_input {
        Interaction::Interactive
    } else {
        Interaction::NonInteractive
    };
    let NewArgs {
        name, kind, web, ..
    } = args;

    // Wizard defaults from the user's config file; flags still win over
    // them. A broken file fails the run with the path named.
    let config = crate::config::load()?.unwrap_or_default();

    let name = match name {
        Some(name) => name,
        None => prompt_name(mode)?,
    };
    let kind = resolve_kind(kind, &web, mode)?;
    let spec = match kind {
        ProjectKind::WebApp => ProjectSpec::WebApp(web::complete(web, mode, &config)?),
        ProjectKind::CliApp => ProjectSpec::CliApp,
        ProjectKind::Library => ProjectSpec::Library,
        ProjectKind::Embedded => ProjectSpec::Embedded,
        ProjectKind::Game => ProjectSpec::Game,
    };
    Ok((name, spec))
}

/// Reject contradictory flag combinations before any prompt runs.
fn validate_flags(args: &NewArgs) -> Result<()> {
    let web = &args.web;

    if web.any_set() && args.kind.is_some_and(|kind| kind != ProjectKind::WebApp) {
        return Err(Error::ConflictingOptions(
            "web stack flags require `--kind web` (or drop `--kind`: it is inferred)".into(),
        ));
    }

    if web.backend.is_none() {
        if web.database.is_some() || web.testing.is_some() || web.features.is_some() {
            return Err(Error::ConflictingOptions(
                "`--db`, `--testing` and `--features` require `--backend`".into(),
            ));
        }
        match (web.deployment, web.frontend) {
            (Some(_), None) => {
                return Err(Error::ConflictingOptions(
                    "`--deployment` is ambiguous without `--backend` or `--frontend`".into(),
                ));
            }
            (Some(_), Some(frontend)) if frontend.is_template_engine() => {
                return Err(Error::ConflictingOptions(
                    "`--deployment` is unused with template-engine frontends".into(),
                ));
            }
            _ => {}
        }
    }

    // sqlx-based tests need the sqlx database stack they exercise.
    let testing_sqlx = web
        .testing
        .as_ref()
        .is_some_and(|testing| testing.contains(&Testing::SQLx));
    if testing_sqlx && web.database != Some(Database::SQLx) {
        return Err(Error::ConflictingOptions(
            "`--testing sqlx` requires `--db sqlx`".into(),
        ));
    }
    Ok(())
}

/// The project kind: the flag, the inference from web flags, or a question.
/// There is no default kind — `--yes` still requires one.
fn resolve_kind(
    kind: Option<ProjectKind>,
    web: &WebFlags,
    mode: Interaction,
) -> Result<ProjectKind> {
    match (kind, web.any_set(), mode) {
        (Some(kind), _, _) => Ok(kind),
        (None, true, _) => Ok(ProjectKind::WebApp),
        (None, false, Interaction::Interactive) => select(
            "What type of project do you want to create?",
            vec![
                ProjectKind::WebApp,
                ProjectKind::CliApp,
                ProjectKind::Library,
                ProjectKind::Embedded,
                ProjectKind::Game,
            ],
        ),
        (None, false, Interaction::Defaults | Interaction::NonInteractive) => {
            Err(Error::MissingOptions("`--kind`".into()))
        }
    }
}

/// The project name is the first question of an interactive run. It has no
/// default: `--yes` still requires it, like `--kind`.
fn prompt_name(mode: Interaction) -> Result<ProjectName> {
    match mode {
        Interaction::Interactive => {
            let name = Text::new("What is the name of your project?")
                .with_render_config(render_cfg())
                .with_validator(validate_name_availability)
                .prompt()?;
            ProjectName::new(name)
        }
        Interaction::Defaults | Interaction::NonInteractive => {
            Err(Error::MissingOptions("`NAME`".into()))
        }
    }
}

/// Prompt validator: crate-name syntax, and the target directory must not
/// already exist with content (the engine re-checks this at generation).
fn validate_name_availability(input: &str) -> std::result::Result<Validation, CustomUserError> {
    if let Err(reason) = ProjectName::validate(input) {
        return Ok(Validation::Invalid(reason.into()));
    }
    let taken = Path::new(input)
        .read_dir()
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false);
    if taken {
        return Ok(Validation::Invalid(
            format!("directory `{input}` already exists and is not empty").into(),
        ));
    }
    Ok(Validation::Valid)
}

/// Single-choice prompt with the rise theme.
pub(crate) fn select<T: Display>(message: &str, options: Vec<T>) -> Result<T> {
    Ok(Select::new(message, options)
        .with_render_config(render_cfg())
        .prompt()?)
}

/// Single-choice prompt with a preselected option (the config file's
/// value): the cursor starts on it, so Enter accepts the preset.
pub(crate) fn select_with_default<T: Display + PartialEq>(
    message: &str,
    options: Vec<T>,
    default: Option<&T>,
) -> Result<T> {
    let start = default.and_then(|value| options.iter().position(|option| option == value));
    let prompt = Select::new(message, options).with_render_config(render_cfg());
    let prompt = match start {
        Some(index) => prompt.with_starting_cursor(index),
        None => prompt,
    };
    Ok(prompt.prompt()?)
}

/// Multi-choice prompt with pre-ticked options (the config file's
/// values); values outside the option list are ignored here and rejected
/// later by the same validation flags go through.
pub(crate) fn multi_select_with_default<T: Display + PartialEq>(
    message: &str,
    options: Vec<T>,
    defaults: &[T],
) -> Result<Vec<T>> {
    let indices: Vec<usize> = options
        .iter()
        .enumerate()
        .filter(|(_, option)| defaults.contains(option))
        .map(|(index, _)| index)
        .collect();
    let prompt = MultiSelect::new(message, options).with_render_config(render_cfg());
    let prompt = if indices.is_empty() {
        prompt
    } else {
        prompt.with_default(&indices)
    };
    Ok(prompt.prompt()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{Backend, Database, Deployment, Feature, Frontend};

    #[test]
    fn web_flags_conflict_with_non_web_kind() {
        let args = NewArgs {
            kind: Some(ProjectKind::CliApp),
            web: WebFlags {
                backend: Some(Backend::Axum),
                ..WebFlags::default()
            },
            ..NewArgs::default()
        };
        assert!(matches!(
            validate_flags(&args),
            Err(Error::ConflictingOptions(_))
        ));
    }

    #[test]
    fn backend_stack_flags_require_backend() {
        let args = NewArgs {
            web: WebFlags {
                frontend: Some(Frontend::React),
                database: Some(Database::SQLx),
                ..WebFlags::default()
            },
            ..NewArgs::default()
        };
        assert!(matches!(
            validate_flags(&args),
            Err(Error::ConflictingOptions(_))
        ));
    }

    #[test]
    fn deployment_alone_is_ambiguous() {
        let args = NewArgs {
            web: WebFlags {
                deployment: Some(Deployment::Docker),
                ..WebFlags::default()
            },
            ..NewArgs::default()
        };
        assert!(matches!(
            validate_flags(&args),
            Err(Error::ConflictingOptions(_))
        ));
    }

    #[test]
    fn deployment_conflicts_with_template_engine_frontend() {
        let args = NewArgs {
            web: WebFlags {
                frontend: Some(Frontend::Tera),
                deployment: Some(Deployment::Docker),
                ..WebFlags::default()
            },
            ..NewArgs::default()
        };
        assert!(matches!(
            validate_flags(&args),
            Err(Error::ConflictingOptions(_))
        ));
    }

    #[test]
    fn deployment_with_plain_frontend_is_fine() {
        let args = NewArgs {
            web: WebFlags {
                frontend: Some(Frontend::React),
                deployment: Some(Deployment::Docker),
                ..WebFlags::default()
            },
            ..NewArgs::default()
        };
        assert!(validate_flags(&args).is_ok());
    }

    #[test]
    fn leptos_accepts_testing_features_and_a_database() {
        // The leptos templates wire databases into server functions and
        // render the same testing/features content as the REST templates;
        // only `mock` testing is rejected, later, by `validate_testing`.
        let args = NewArgs {
            web: WebFlags {
                backend: Some(Backend::Axum),
                frontend: Some(Frontend::Leptos),
                database: Some(Database::SQLx),
                testing: Some(vec![Testing::Api, Testing::SQLx]),
                features: Some(vec![Feature::WebSockets]),
                ..WebFlags::default()
            },
            ..NewArgs::default()
        };
        assert!(validate_flags(&args).is_ok());
    }

    #[test]
    fn sqlx_testing_requires_sqlx_database() {
        let args = NewArgs {
            web: WebFlags {
                backend: Some(Backend::Axum),
                database: Some(Database::Diesel),
                testing: Some(vec![Testing::SQLx]),
                ..WebFlags::default()
            },
            ..NewArgs::default()
        };
        assert!(matches!(
            validate_flags(&args),
            Err(Error::ConflictingOptions(_))
        ));
    }

    #[test]
    fn sqlx_testing_with_sqlx_database_is_fine() {
        let args = NewArgs {
            web: WebFlags {
                backend: Some(Backend::Axum),
                database: Some(Database::SQLx),
                testing: Some(vec![Testing::SQLx]),
                ..WebFlags::default()
            },
            ..NewArgs::default()
        };
        assert!(validate_flags(&args).is_ok());
    }

    #[test]
    fn web_flags_infer_web_kind() {
        let web = WebFlags {
            backend: Some(Backend::Axum),
            ..WebFlags::default()
        };
        let kind = resolve_kind(None, &web, Interaction::NonInteractive).expect("inferred");
        assert_eq!(kind, ProjectKind::WebApp);
    }

    #[test]
    fn missing_kind_is_an_error_when_non_interactive() {
        let web = WebFlags::default();
        assert!(matches!(
            resolve_kind(None, &web, Interaction::NonInteractive),
            Err(Error::MissingOptions(_))
        ));
    }

    #[test]
    fn kind_has_no_default_under_yes() {
        // `--yes` fills option defaults, but there is no default kind.
        let web = WebFlags::default();
        assert!(matches!(
            resolve_kind(None, &web, Interaction::Defaults),
            Err(Error::MissingOptions(_))
        ));
    }
}
