//! WebApp wizard: completes the web part of the spec from flags + prompts.

use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter};

use crate::cli::WebFlags;
use crate::config::Config;
use crate::error::{Error, Result};
use crate::spec::{
    Backend, BackendSpec, Database, Deployment, Feature, Frontend, FrontendSpec, FullstackSpec,
    Testing, WebAppSpec,
};
use crate::wizard::{Interaction, multi_select_with_default, select, select_with_default};

/// Shape of a web app — asked only when neither `--backend` nor
/// `--frontend` was passed.
#[derive(Debug, Clone, Copy, Display, EnumIter)]
enum WebAppKind {
    Backend,
    Frontend,
    Fullstack,
}

/// The backend questions, as far as the flags already answered them.
struct BackendDraft {
    database: Option<Database>,
    deployment: Option<Deployment>,
    testing: Option<Vec<Testing>>,
    features: Option<Vec<Feature>>,
}

/// Complete a [`WebAppSpec`], asking only for what the flags left open.
///
/// The user's config file supplies defaults below the flags: interactive
/// prompts are preselected with them, `--yes` and non-interactive runs
/// take them as values.
///
/// # Errors
///
/// [`Error::MissingOptions`] when a required option is missing in a
/// non-interactive run, [`Error::Aborted`]/[`Error::Prompt`] from prompts.
pub(crate) fn complete(flags: WebFlags, mode: Interaction, config: &Config) -> Result<WebAppSpec> {
    let WebFlags {
        backend,
        frontend,
        database,
        deployment,
        testing,
        features,
    } = flags;

    match (backend, frontend) {
        (Some(backend), frontend) => {
            // `--deployment` describes the backend whenever one exists.
            let draft = BackendDraft {
                database,
                deployment,
                testing,
                features,
            };
            let backend = complete_backend(backend, draft, mode, config)?;
            Ok(match frontend {
                None => WebAppSpec::Backend(backend),
                Some(frontend) => WebAppSpec::Fullstack(FullstackSpec { frontend, backend }),
            })
        }
        (None, Some(frontend)) => {
            // Template engines render inside whatever serves them; flag
            // validation has already rejected `--deployment` for them.
            let deployment = if frontend.is_template_engine() {
                None
            } else {
                Some(fill(
                    deployment,
                    config.deployment,
                    mode,
                    "`--deployment`",
                    Deployment::Docker,
                    |preset| {
                        select_with_default(
                            "Select a deployment option",
                            Deployment::iter().collect(),
                            preset.as_ref(),
                        )
                    },
                )?)
            };
            Ok(WebAppSpec::Frontend(FrontendSpec {
                frontend,
                deployment,
            }))
        }
        (None, None) => match mode {
            // `--yes` with no shape flags: the default web app is a
            // backend-only axum service — or whatever stack the config
            // file prescribes; the re-entered flag path applies the
            // per-option defaults (sqlite, docker, no testing/features).
            Interaction::Defaults => complete(
                WebFlags {
                    backend: config.backend.or(Some(Backend::Axum)),
                    ..config_flags(config)
                },
                mode,
                config,
            ),
            // A config file that names a backend or frontend is a preset:
            // it fills the shape, not just the options. Without one the
            // shape stays required.
            Interaction::NonInteractive
                if config.backend.is_some() || config.frontend.is_some() =>
            {
                complete(config_flags(config), mode, config)
            }
            Interaction::NonInteractive => Err(Error::MissingOptions(
                "`--backend` and/or `--frontend` (the config file can set them too)".into(),
            )),
            // No web flags at all: ask everything.
            Interaction::Interactive => prompt_full(config),
        },
    }
}

/// The config file's values in the shape of flags, for the no-shape-flag
/// paths that re-enter [`complete`]. Empty selections stay `None` so the
/// per-option defaults still apply below them.
fn config_flags(config: &Config) -> WebFlags {
    WebFlags {
        backend: config.backend,
        frontend: config.frontend,
        database: config.database,
        deployment: config.deployment,
        testing: (!config.testing.is_empty()).then(|| config.testing.clone()),
        features: (!config.features.is_empty()).then(|| config.features.clone()),
    }
}

/// The fully interactive web flow, for runs without any web flags.
/// Only the shape/backend/frontend questions live here; everything else
/// re-enters [`complete`] so interactive and flag-driven runs share one
/// question path. Config values preselect the answers.
fn prompt_full(config: &Config) -> Result<WebAppSpec> {
    let (backend, frontend) = match select("Select a webapp kind", WebAppKind::iter().collect())? {
        WebAppKind::Backend => (
            Some(select_with_default(
                "Select a backend",
                implemented_backends(),
                config.backend.as_ref(),
            )?),
            None,
        ),
        WebAppKind::Frontend => (
            None,
            Some(select_with_default(
                "Select a frontend tech",
                frontend_options(None),
                config.frontend.as_ref(),
            )?),
        ),
        WebAppKind::Fullstack => {
            let backend = select_with_default(
                "Select a backend",
                implemented_backends(),
                config.backend.as_ref(),
            )?;
            let frontend = select_with_default(
                "Select a frontend tech",
                frontend_options(Some(backend)),
                config.frontend.as_ref(),
            )?;
            (Some(backend), Some(frontend))
        }
    };
    complete(
        WebFlags {
            backend,
            frontend,
            ..WebFlags::default()
        },
        Interaction::Interactive,
        config,
    )
}

/// Complete the backend spec, asking only for what the flags left open.
///
/// Flag-passed values outside the implemented list are rejected by name.
/// The config file's values sit between flags and the built-in defaults.
fn complete_backend(
    backend: Backend,
    draft: BackendDraft,
    mode: Interaction,
    config: &Config,
) -> Result<BackendSpec> {
    let database = fill(
        draft.database,
        config.database,
        mode,
        "`--db`",
        Database::SQLite,
        |preset| {
            select_with_default(
                "Select a database",
                Database::iter().collect(),
                preset.as_ref(),
            )
        },
    )?;
    let deployment = fill(
        draft.deployment,
        config.deployment,
        mode,
        "`--deployment`",
        Deployment::Docker,
        |preset| {
            select_with_default(
                "Select a deployment option",
                Deployment::iter().collect(),
                preset.as_ref(),
            )
        },
    )?;
    let implemented_testing = implemented_testing(database);
    let testing = fill_multi(draft.testing, &config.testing, mode, |preset| {
        multi_select_with_default(
            "Select testing options",
            implemented_testing.clone(),
            preset,
        )
    })?;
    reject_unavailable(&testing, &implemented_testing, Error::TestingNotAvailable)?;
    let implemented_features = implemented_features();
    let features = fill_multi(draft.features, &config.features, mode, |preset| {
        multi_select_with_default("Select features", implemented_features.clone(), preset)
    })?;
    reject_unavailable(
        &features,
        &implemented_features,
        Error::FeaturesNotAvailable,
    )?;
    Ok(BackendSpec {
        backend,
        database,
        deployment,
        testing,
        features,
    })
}

/// The testing options the templates render for the given database.
/// `mock` is offered to no stack: the generated code has no trait seam
/// for a mock to implement, so the option would emit a dead dependency
/// and nothing else. `sqlx` generates a pool test, which needs a sqlx
/// pool to exercise — both sqlx-backed databases (postgres and sqlite)
/// have one; the flag path rejects `--testing sqlx` with another
/// database in `validate_flags`. Flag-passed values outside this list
/// are rejected by [`reject_unavailable`].
fn implemented_testing(database: Database) -> Vec<Testing> {
    let mut options = vec![Testing::Api];
    if matches!(database, Database::SQLx | Database::SQLite) {
        options.push(Testing::SQLx);
    }
    options
}

/// Reject selected options with no template content for the chosen
/// stack, naming them by their CLI spellings. The implemented list is a
/// parameter so the rejection path stays testable.
fn reject_unavailable<T>(
    selected: &[T],
    implemented: &[T],
    error: impl FnOnce(String) -> Error,
) -> Result<()>
where
    T: PartialEq + clap::ValueEnum,
{
    let unavailable = selected
        .iter()
        .filter(|option| !implemented.contains(option))
        .map(|option| {
            option
                .to_possible_value()
                .expect("option values are never hidden")
                .get_name()
                .to_owned()
        })
        .collect::<Vec<_>>();
    if unavailable.is_empty() {
        return Ok(());
    }
    Err(error(unavailable.join(", ")))
}

/// The features the templates actually render — currently every
/// [`Feature`] variant. The list is the gate for future additions: a new
/// variant without template content stays off the prompts, and flag-passed
/// values are rejected up front instead of being silently ignored.
fn implemented_features() -> Vec<Feature> {
    vec![
        Feature::WebSockets,
        Feature::gRPC,
        Feature::GraphQL,
        Feature::RateLimiting,
        Feature::Cors,
        Feature::Auth,
    ]
}

/// Take the flag value, the config value, the `--yes` default, ask for
/// it, or report it missing. Precedence: flag > config > built-in
/// default (the default applies only in [`Interaction::Defaults`]). An
/// interactive run still asks — the config value preselects the answer.
fn fill<T: Copy>(
    flag: Option<T>,
    config: Option<T>,
    mode: Interaction,
    name: &'static str,
    default: T,
    ask: impl FnOnce(Option<T>) -> Result<T>,
) -> Result<T> {
    match (flag, mode) {
        (Some(value), _) => Ok(value),
        (None, Interaction::Interactive) => ask(config),
        (None, Interaction::Defaults) => Ok(config.unwrap_or(default)),
        (None, Interaction::NonInteractive) => config.ok_or_else(|| {
            Error::MissingOptions(format!("{name} (the config file can set it too)"))
        }),
    }
}

/// Multi-value flags are optional: a `--yes` or non-interactive run
/// leaves them empty, or takes the config file's selection when one is
/// configured. Interactive runs pre-tick the config values.
fn fill_multi<T: Clone>(
    flag: Option<Vec<T>>,
    config: &[T],
    mode: Interaction,
    ask: impl FnOnce(&[T]) -> Result<Vec<T>>,
) -> Result<Vec<T>> {
    match (flag, mode) {
        (Some(values), _) => Ok(values),
        (None, Interaction::Interactive) => ask(config),
        (None, Interaction::Defaults | Interaction::NonInteractive) => Ok(config.to_vec()),
    }
}

/// The backends with templates. Flag-passed others fail at generation
/// with a typed `TemplateNotFound`; the wizard does not offer them.
fn implemented_backends() -> Vec<Backend> {
    vec![Backend::Axum, Backend::Actix, Backend::Rocket]
}

/// The frontend choices that have templates, depending on the backend the
/// user picked. Without a backend: the standalone trunk SPAs. With one:
/// the WASM frontends and the server-side template engines — except that
/// Leptos integrates with axum and actix only, so a rocket backend is not
/// offered it. Other flag-passed frontends fail at generation with a typed
/// `TemplateNotFound`.
fn frontend_options(backend: Option<Backend>) -> Vec<Frontend> {
    use Frontend::*;
    match backend {
        None => vec![Yew, Leptos],
        Some(Backend::Rocket) => vec![Yew, Askama, Tera, Minijinja, Handlebars],
        Some(_) => vec![Yew, Leptos, Askama, Tera, Minijinja, Handlebars],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_backends_with_templates_are_offered() {
        assert_eq!(
            implemented_backends(),
            vec![Backend::Axum, Backend::Actix, Backend::Rocket]
        );
    }

    #[test]
    fn yes_defaults_build_the_default_stack() {
        // `--kind web --yes` with no shape flags: backend-only axum, and
        // the per-option defaults behind it — sqlite, so the project runs
        // with no external services.
        let spec = complete(
            WebFlags::default(),
            Interaction::Defaults,
            &Config::default(),
        )
        .expect("defaults");
        let WebAppSpec::Backend(backend) = spec else {
            panic!("backend-only is the default shape");
        };
        assert_eq!(backend.backend, Backend::Axum);
        assert_eq!(backend.database, Database::SQLite);
        assert_eq!(backend.deployment, Deployment::Docker);
        assert!(backend.testing.is_empty());
        assert!(backend.features.is_empty());
    }

    #[test]
    fn yes_defaults_fill_the_rest_of_a_flag_passed_stack() {
        // Flags win over defaults; only the gaps are filled.
        let spec = complete(
            WebFlags {
                backend: Some(Backend::Actix),
                database: Some(Database::Sled),
                ..WebFlags::default()
            },
            Interaction::Defaults,
            &Config::default(),
        )
        .expect("defaults");
        let WebAppSpec::Backend(backend) = spec else {
            panic!("backend-only shape");
        };
        assert_eq!(backend.backend, Backend::Actix);
        assert_eq!(backend.database, Database::Sled);
        assert_eq!(backend.deployment, Deployment::Docker);
        assert!(backend.testing.is_empty());
    }

    #[test]
    fn yes_defaults_give_a_yew_frontend_docker() {
        let spec = complete(
            WebFlags {
                frontend: Some(Frontend::Yew),
                ..WebFlags::default()
            },
            Interaction::Defaults,
            &Config::default(),
        )
        .expect("defaults");
        let WebAppSpec::Frontend(frontend) = spec else {
            panic!("frontend-only shape");
        };
        assert_eq!(frontend.frontend, Frontend::Yew);
        assert_eq!(frontend.deployment, Some(Deployment::Docker));
    }

    #[test]
    fn yes_defaults_leave_a_template_engine_without_deployment() {
        // `--deployment` is not just defaulted to None for template
        // engines — it is not a valid option there at all.
        let spec = complete(
            WebFlags {
                frontend: Some(Frontend::Askama),
                ..WebFlags::default()
            },
            Interaction::Defaults,
            &Config::default(),
        )
        .expect("defaults");
        let WebAppSpec::Frontend(frontend) = spec else {
            panic!("frontend-only shape");
        };
        assert_eq!(frontend.deployment, None);
    }

    #[test]
    fn frontend_options_with_backend() {
        let options = frontend_options(Some(Backend::Axum));
        assert!(options.contains(&Frontend::Yew));
        assert!(options.contains(&Frontend::Leptos));
        assert!(options.contains(&Frontend::Askama));
        assert!(!options.contains(&Frontend::Seed));
        assert!(!options.contains(&Frontend::React));
    }

    #[test]
    fn frontend_options_without_backend() {
        // Frontend-only: the standalone trunk SPAs.
        assert_eq!(
            frontend_options(None),
            vec![Frontend::Yew, Frontend::Leptos]
        );
    }

    #[test]
    fn rocket_backend_is_not_offered_leptos() {
        // Leptos integrates with axum and actix only: the prompt must not
        // offer a choice that would dead-end at generation.
        let options = frontend_options(Some(Backend::Rocket));
        assert!(!options.contains(&Frontend::Leptos));
        assert!(options.contains(&Frontend::Yew));
        assert!(options.contains(&Frontend::Tera));
    }

    #[test]
    fn fill_prefers_the_flag_over_asking() {
        let value = fill(
            Some(Database::Diesel),
            None,
            Interaction::Interactive,
            "`--db`",
            Database::SQLx,
            |_| panic!("must not ask when the flag is set"),
        )
        .expect("flag value");
        assert_eq!(value, Database::Diesel);
    }

    #[test]
    fn fill_takes_the_default_under_yes() {
        let value = fill(
            None,
            None,
            Interaction::Defaults,
            "`--db`",
            Database::SQLx,
            |_| unreachable!(),
        )
        .expect("default value");
        assert_eq!(value, Database::SQLx);
    }

    #[test]
    fn fill_reports_missing_options_when_non_interactive() {
        let result = fill::<Database>(
            None,
            None,
            Interaction::NonInteractive,
            "`--db`",
            Database::SQLx,
            |_| unreachable!(),
        );
        assert!(matches!(result, Err(Error::MissingOptions(_))));
    }

    #[test]
    fn fill_multi_defaults_to_empty_when_non_interactive() {
        let values =
            fill_multi::<Testing>(None, &[], Interaction::NonInteractive, |_| unreachable!())
                .expect("empty by default");
        assert!(values.is_empty());
    }

    #[test]
    fn unimplemented_features_are_rejected_by_name() {
        // The rejection path must keep working for future Feature variants
        // even though every current one has template content.
        let err = reject_unavailable(
            &[Feature::WebSockets, Feature::gRPC],
            &[Feature::WebSockets],
            Error::FeaturesNotAvailable,
        )
        .expect_err("grpc is not in the given implemented list");
        let Error::FeaturesNotAvailable(names) = err else {
            panic!("expected FeaturesNotAvailable");
        };
        assert_eq!(names, "grpc");
    }

    #[test]
    fn mock_testing_is_not_offered() {
        // No template renders mock content, so no database offers it.
        for database in Database::iter() {
            assert!(!implemented_testing(database).contains(&Testing::Mock));
        }
    }

    #[test]
    fn sqlx_testing_is_offered_only_with_a_sqlx_backed_database() {
        // The sqlx pool test exercises a sqlx pool: postgres and sqlite.
        assert_eq!(
            implemented_testing(Database::SQLx),
            vec![Testing::Api, Testing::SQLx]
        );
        assert_eq!(
            implemented_testing(Database::SQLite),
            vec![Testing::Api, Testing::SQLx]
        );
        assert_eq!(implemented_testing(Database::Diesel), vec![Testing::Api]);
        assert_eq!(implemented_testing(Database::SeaORM), vec![Testing::Api]);
        assert_eq!(implemented_testing(Database::Sled), vec![Testing::Api]);
    }

    #[test]
    fn unimplemented_testing_options_are_rejected_by_name() {
        let err = reject_unavailable(
            &[Testing::Mock],
            &[Testing::Api, Testing::SQLx],
            Error::TestingNotAvailable,
        )
        .expect_err("mock is not in the given implemented list");
        let Error::TestingNotAvailable(names) = err else {
            panic!("expected TestingNotAvailable");
        };
        assert_eq!(names, "mock");
    }

    #[test]
    fn implemented_testing_passes_validation() {
        for database in Database::iter() {
            let implemented = implemented_testing(database);
            reject_unavailable(
                &implemented.clone(),
                &implemented,
                Error::TestingNotAvailable,
            )
            .expect("the implemented list passes its own validation");
        }
    }

    #[test]
    fn config_values_fill_gaps_under_yes() {
        // A config stack replaces the built-in `--yes` default stack.
        let config = Config {
            backend: Some(Backend::Rocket),
            database: Some(Database::Sled),
            ..Config::default()
        };
        let spec =
            complete(WebFlags::default(), Interaction::Defaults, &config).expect("config defaults");
        let WebAppSpec::Backend(backend) = spec else {
            panic!("backend-only shape");
        };
        assert_eq!(backend.backend, Backend::Rocket);
        assert_eq!(backend.database, Database::Sled);
        // No config value: the built-in default still applies.
        assert_eq!(backend.deployment, Deployment::Docker);
        assert!(backend.features.is_empty());
    }

    #[test]
    fn a_config_frontend_makes_yes_fullstack() {
        let config = Config {
            frontend: Some(Frontend::Yew),
            ..Config::default()
        };
        let spec =
            complete(WebFlags::default(), Interaction::Defaults, &config).expect("config defaults");
        let WebAppSpec::Fullstack(fullstack) = spec else {
            panic!("a config frontend turns `--yes` fullstack");
        };
        assert_eq!(fullstack.frontend, Frontend::Yew);
        assert_eq!(fullstack.backend.backend, Backend::Axum);
    }

    #[test]
    fn flags_beat_config_values() {
        let config = Config {
            backend: Some(Backend::Actix),
            database: Some(Database::Sled),
            features: vec![Feature::Cors],
            ..Config::default()
        };
        let spec = complete(
            WebFlags {
                backend: Some(Backend::Axum),
                database: Some(Database::Diesel),
                ..WebFlags::default()
            },
            Interaction::Defaults,
            &config,
        )
        .expect("flags win");
        let WebAppSpec::Backend(backend) = spec else {
            panic!("backend-only shape");
        };
        assert_eq!(backend.backend, Backend::Axum);
        assert_eq!(backend.database, Database::Diesel);
        // No flag: the config value applies.
        assert_eq!(backend.features, vec![Feature::Cors]);
    }

    #[test]
    fn a_config_with_a_shape_fills_it_when_non_interactive() {
        // A config that names a backend is a preset: no shape flags needed.
        let config = Config {
            backend: Some(Backend::Actix),
            database: Some(Database::Sled),
            deployment: Some(Deployment::SelfHosted),
            ..Config::default()
        };
        let spec = complete(WebFlags::default(), Interaction::NonInteractive, &config)
            .expect("the config fills the shape");
        let WebAppSpec::Backend(backend) = spec else {
            panic!("backend-only shape");
        };
        assert_eq!(backend.backend, Backend::Actix);
    }

    #[test]
    fn no_config_still_requires_a_shape_when_non_interactive() {
        let result = complete(
            WebFlags::default(),
            Interaction::NonInteractive,
            &Config::default(),
        );
        assert!(matches!(result, Err(Error::MissingOptions(_))));
    }

    #[test]
    fn fill_prefers_the_config_over_the_builtin_default() {
        let value = fill(
            None,
            Some(Database::Sled),
            Interaction::Defaults,
            "`--db`",
            Database::SQLx,
            |_| unreachable!(),
        )
        .expect("config value");
        assert_eq!(value, Database::Sled);
    }

    #[test]
    fn an_interactive_fill_asks_with_the_config_value() {
        // The ask closure receives the config value to preselect the
        // prompt; the user's answer still wins.
        let value = fill(
            None,
            Some(Database::Sled),
            Interaction::Interactive,
            "`--db`",
            Database::SQLx,
            |preset| {
                assert_eq!(preset, Some(Database::Sled));
                Ok(Database::SeaORM)
            },
        )
        .expect("asked");
        assert_eq!(value, Database::SeaORM);
    }

    #[test]
    fn fill_multi_takes_the_config_selection_when_non_interactive() {
        let values = fill_multi(
            None,
            &[Testing::Api],
            Interaction::NonInteractive,
            |_| unreachable!(),
        )
        .expect("config selection");
        assert_eq!(values, vec![Testing::Api]);
    }

    #[test]
    fn config_values_outside_the_implemented_list_are_rejected() {
        // A config file goes through the same availability validation as
        // flags: `mock` has no template content anywhere.
        let config = Config {
            backend: Some(Backend::Axum),
            database: Some(Database::Sled),
            deployment: Some(Deployment::Docker),
            testing: vec![Testing::Mock],
            ..Config::default()
        };
        let result = complete(WebFlags::default(), Interaction::NonInteractive, &config);
        assert!(matches!(result, Err(Error::TestingNotAvailable(_))));
    }

    #[test]
    fn implemented_features_pass_validation() {
        let implemented = implemented_features();
        reject_unavailable(
            &[
                Feature::WebSockets,
                Feature::gRPC,
                Feature::GraphQL,
                Feature::RateLimiting,
            ],
            &implemented,
            Error::FeaturesNotAvailable,
        )
        .expect("all implemented features pass");
        reject_unavailable(&[], &implemented, Error::FeaturesNotAvailable)
            .expect("no features is fine");
    }

    #[test]
    fn every_feature_is_implemented() {
        // Adding a Feature variant fails this test until the templates
        // render it — or the offered list is knowingly shrunk.
        assert_eq!(implemented_features(), Feature::iter().collect::<Vec<_>>());
    }
}
