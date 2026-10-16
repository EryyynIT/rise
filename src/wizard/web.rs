//! WebApp wizard: completes the web part of the spec from flags + prompts.

use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter};

use crate::cli::WebFlags;
use crate::error::{Error, Result};
use crate::spec::{
    Backend, BackendSpec, Database, Deployment, Feature, Frontend, FrontendSpec, FullstackSpec,
    Testing, WebAppSpec,
};
use crate::wizard::{Interaction, multi_select, select};

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
/// # Errors
///
/// [`Error::MissingOptions`] when a required option is missing in a
/// non-interactive run, [`Error::Aborted`]/[`Error::Prompt`] from prompts.
pub(crate) fn complete(flags: WebFlags, mode: Interaction) -> Result<WebAppSpec> {
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
            let backend = complete_backend(backend, draft, mode)?;
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
                    mode,
                    "`--deployment`",
                    Deployment::Docker,
                    || select("Select a deployment option", Deployment::iter().collect()),
                )?)
            };
            Ok(WebAppSpec::Frontend(FrontendSpec {
                frontend,
                deployment,
            }))
        }
        (None, None) => match mode {
            // `--yes` with no shape flags: the default web app is a
            // backend-only axum service; the flag path applies the
            // per-option defaults (sqlx, docker, no testing/features).
            Interaction::Defaults => complete(
                WebFlags {
                    backend: Some(Backend::Axum),
                    ..WebFlags::default()
                },
                mode,
            ),
            Interaction::NonInteractive => Err(Error::MissingOptions(
                "`--backend` and/or `--frontend`".into(),
            )),
            // No web flags at all: ask everything.
            Interaction::Interactive => prompt_full(),
        },
    }
}

/// The fully interactive web flow, for runs without any web flags.
/// Only the shape/backend/frontend questions live here; everything else
/// re-enters [`complete`] so interactive and flag-driven runs share one
/// question path.
fn prompt_full() -> Result<WebAppSpec> {
    let (backend, frontend) = match select("Select a webapp kind", WebAppKind::iter().collect())? {
        WebAppKind::Backend => (
            Some(select("Select a backend", implemented_backends())?),
            None,
        ),
        WebAppKind::Frontend => (
            None,
            Some(select("Select a frontend tech", frontend_options(None))?),
        ),
        WebAppKind::Fullstack => {
            let backend = select("Select a backend", implemented_backends())?;
            let frontend = select("Select a frontend tech", frontend_options(Some(backend)))?;
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
    )
}

/// Complete the backend spec, asking only for what the flags left open.
///
/// Flag-passed values outside the implemented list are rejected by name.
fn complete_backend(
    backend: Backend,
    draft: BackendDraft,
    mode: Interaction,
) -> Result<BackendSpec> {
    let database = fill(draft.database, mode, "`--db`", Database::SQLx, || {
        select("Select a database", Database::iter().collect())
    })?;
    let deployment = fill(
        draft.deployment,
        mode,
        "`--deployment`",
        Deployment::Docker,
        || select("Select a deployment option", Deployment::iter().collect()),
    )?;
    let implemented_testing = implemented_testing(database);
    let testing = fill_multi(draft.testing, mode, || {
        multi_select("Select testing options", implemented_testing.clone())
    })?;
    reject_unavailable(&testing, &implemented_testing, Error::TestingNotAvailable)?;
    let implemented_features = implemented_features();
    let features = fill_multi(draft.features, mode, || {
        multi_select("Select features", implemented_features.clone())
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
/// pool to exercise — the flag path rejects `--testing sqlx` with another
/// database in `validate_flags`. Flag-passed values outside this list are
/// rejected by [`reject_unavailable`].
fn implemented_testing(database: Database) -> Vec<Testing> {
    let mut options = vec![Testing::Api];
    if database == Database::SQLx {
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

/// Take the flag value, the `--yes` default, ask for it, or report it
/// missing. The default applies only in [`Interaction::Defaults`].
fn fill<T: Copy>(
    flag: Option<T>,
    mode: Interaction,
    name: &'static str,
    default: T,
    ask: impl FnOnce() -> Result<T>,
) -> Result<T> {
    match (flag, mode) {
        (Some(value), _) => Ok(value),
        (None, Interaction::Interactive) => ask(),
        (None, Interaction::Defaults) => Ok(default),
        (None, Interaction::NonInteractive) => Err(Error::MissingOptions(name.into())),
    }
}

/// Multi-value flags are optional: a `--yes` or non-interactive run leaves
/// them empty.
fn fill_multi<T>(
    flag: Option<Vec<T>>,
    mode: Interaction,
    ask: impl FnOnce() -> Result<Vec<T>>,
) -> Result<Vec<T>> {
    match (flag, mode) {
        (Some(values), _) => Ok(values),
        (None, Interaction::Interactive) => ask(),
        (None, Interaction::Defaults | Interaction::NonInteractive) => Ok(Vec::new()),
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
        // the per-option defaults behind it.
        let spec = complete(WebFlags::default(), Interaction::Defaults).expect("defaults");
        let WebAppSpec::Backend(backend) = spec else {
            panic!("backend-only is the default shape");
        };
        assert_eq!(backend.backend, Backend::Axum);
        assert_eq!(backend.database, Database::SQLx);
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
            Interaction::Interactive,
            "`--db`",
            Database::SQLx,
            || panic!("must not ask when the flag is set"),
        )
        .expect("flag value");
        assert_eq!(value, Database::Diesel);
    }

    #[test]
    fn fill_takes_the_default_under_yes() {
        let value = fill(
            None,
            Interaction::Defaults,
            "`--db`",
            Database::SQLx,
            || unreachable!(),
        )
        .expect("default value");
        assert_eq!(value, Database::SQLx);
    }

    #[test]
    fn fill_reports_missing_options_when_non_interactive() {
        let result = fill::<Database>(
            None,
            Interaction::NonInteractive,
            "`--db`",
            Database::SQLx,
            || unreachable!(),
        );
        assert!(matches!(result, Err(Error::MissingOptions(_))));
    }

    #[test]
    fn fill_multi_defaults_to_empty_when_non_interactive() {
        let values = fill_multi::<Testing>(None, Interaction::NonInteractive, || unreachable!())
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
    fn sqlx_testing_is_offered_only_with_the_sqlx_database() {
        // The sqlx pool test exercises a sqlx pool.
        assert_eq!(
            implemented_testing(Database::SQLx),
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
