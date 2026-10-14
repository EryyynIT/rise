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
            let backend = complete_backend(backend, draft, frontend, mode)?;
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
                Some(fill(deployment, mode, "`--deployment`", || {
                    select("Select a deployment option", Deployment::iter().collect())
                })?)
            };
            Ok(WebAppSpec::Frontend(FrontendSpec {
                frontend,
                deployment,
            }))
        }
        (None, None) => match mode {
            Interaction::NonInteractive => Err(Error::MissingOptions(
                "`--backend` and/or `--frontend`".into(),
            )),
            // No web flags at all: ask everything.
            Interaction::Interactive => prompt_full(),
        },
    }
}

/// The fully interactive web flow, for runs without any web flags.
fn prompt_full() -> Result<WebAppSpec> {
    match select("Select a webapp kind", WebAppKind::iter().collect())? {
        WebAppKind::Backend => {
            let backend = select("Select a backend", Backend::iter().collect())?;
            Ok(WebAppSpec::Backend(prompt_backend(backend, None)?))
        }
        WebAppKind::Frontend => {
            let frontend = select("Select a frontend tech", frontend_options(false))?;
            // Server-side template engines render inside whatever serves them.
            let deployment = if frontend.is_template_engine() {
                None
            } else {
                Some(select(
                    "Select a deployment option",
                    Deployment::iter().collect(),
                )?)
            };
            Ok(WebAppSpec::Frontend(FrontendSpec {
                frontend,
                deployment,
            }))
        }
        WebAppKind::Fullstack => {
            let backend = select("Select a backend", Backend::iter().collect())?;
            let frontend = select("Select a frontend tech", frontend_options(true))?;
            let backend = prompt_backend(backend, Some(frontend))?;
            Ok(WebAppSpec::Fullstack(FullstackSpec { frontend, backend }))
        }
    }
}

/// Ask all backend questions, shared by backend-only and fullstack apps.
///
/// A Leptos fullstack app talks to its backend through server functions;
/// the database/testing/features wiring of the REST templates does not
/// apply to it yet, so those questions are skipped (deployment stays —
/// it drives the Dockerfile).
fn prompt_backend(backend: Backend, frontend: Option<Frontend>) -> Result<BackendSpec> {
    let leptos = frontend == Some(Frontend::Leptos);
    let database = match leptos {
        true => None,
        false => Some(select("Select a database", Database::iter().collect())?),
    };
    let deployment = select("Select a deployment option", Deployment::iter().collect())?;
    let (testing, features) = match leptos {
        true => (Vec::new(), Vec::new()),
        false => {
            let testing = multi_select("Select testing options", Testing::iter().collect())?;
            let features = multi_select("Select features", implemented_features())?;
            validate_features(&features)?;
            (testing, features)
        }
    };
    Ok(BackendSpec {
        backend,
        database,
        deployment,
        testing,
        features,
    })
}

/// Complete the backend spec, asking only for what the flags left open.
///
/// For a Leptos frontend the database/testing/features questions are
/// skipped like in [`prompt_backend`]; flag validation has already
/// rejected those flags, so the draft fields are guaranteed to be unset.
fn complete_backend(
    backend: Backend,
    draft: BackendDraft,
    frontend: Option<Frontend>,
    mode: Interaction,
) -> Result<BackendSpec> {
    let leptos = frontend == Some(Frontend::Leptos);
    let database = match leptos {
        true => None,
        false => Some(fill(draft.database, mode, "`--db`", || {
            select("Select a database", Database::iter().collect())
        })?),
    };
    let deployment = fill(draft.deployment, mode, "`--deployment`", || {
        select("Select a deployment option", Deployment::iter().collect())
    })?;
    let (testing, features) = match leptos {
        true => (Vec::new(), Vec::new()),
        false => {
            let testing = fill_multi(draft.testing, mode, || {
                multi_select("Select testing options", Testing::iter().collect())
            })?;
            let features = fill_multi(draft.features, mode, || {
                multi_select("Select features", implemented_features())
            })?;
            validate_features(&features)?;
            (testing, features)
        }
    };
    Ok(BackendSpec {
        backend,
        database,
        deployment,
        testing,
        features,
    })
}

/// The features the templates actually render. The rest of the
/// [`Feature`] enum is the roadmap: prompts offer only implemented ones,
/// and flag-passed values outside this list are rejected up front instead
/// of being silently ignored.
fn implemented_features() -> Vec<Feature> {
    vec![Feature::WebSockets, Feature::GraphQL, Feature::RateLimiting]
}

/// Reject features with no template content yet, naming them by their
/// CLI spellings.
fn validate_features(features: &[Feature]) -> Result<()> {
    use clap::ValueEnum;
    let unimplemented = features
        .iter()
        .filter(|feature| !implemented_features().contains(feature))
        .map(|feature| {
            feature
                .to_possible_value()
                .expect("feature values are never hidden")
                .get_name()
                .to_owned()
        })
        .collect::<Vec<_>>();
    if unimplemented.is_empty() {
        return Ok(());
    }
    Err(Error::FeaturesNotAvailable(unimplemented.join(", ")))
}

/// Take the flag value, ask for it, or report it missing.
fn fill<T: Copy>(
    flag: Option<T>,
    mode: Interaction,
    name: &'static str,
    ask: impl FnOnce() -> Result<T>,
) -> Result<T> {
    match (flag, mode) {
        (Some(value), _) => Ok(value),
        (None, Interaction::Interactive) => ask(),
        (None, Interaction::NonInteractive) => Err(Error::MissingOptions(name.into())),
    }
}

/// Multi-value flags are optional: a non-interactive run leaves them empty.
fn fill_multi<T>(
    flag: Option<Vec<T>>,
    mode: Interaction,
    ask: impl FnOnce() -> Result<Vec<T>>,
) -> Result<Vec<T>> {
    match (flag, mode) {
        (Some(values), _) => Ok(values),
        (None, Interaction::Interactive) => ask(),
        (None, Interaction::NonInteractive) => Ok(Vec::new()),
    }
}

/// Frontend choices depend on whether a Rust backend is present.
fn frontend_options(has_backend: bool) -> Vec<Frontend> {
    use Frontend::*;
    // Yew works both ways: a standalone trunk SPA or on top of a backend.
    // Leptos generates only with a backend, so it is offered only there —
    // a frontend-only Leptos run would dead-end at generation.
    let mut options = vec![Svelte, React, Vue, Minijinja, Handlebars, Yew];
    let specific: &[Frontend] = if has_backend {
        &[Leptos, Tera, Askama]
    } else {
        &[Seed, Sauron]
    };
    options.extend_from_slice(specific);
    options
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontend_options_with_backend() {
        let options = frontend_options(true);
        assert!(options.contains(&Frontend::Yew));
        assert!(options.contains(&Frontend::Minijinja));
        assert!(options.contains(&Frontend::Leptos));
        assert!(!options.contains(&Frontend::Seed));
    }

    #[test]
    fn frontend_options_without_backend() {
        let options = frontend_options(false);
        // Leptos generates only with a backend; offering it frontend-only
        // would dead-end the wizard at generation.
        assert!(!options.contains(&Frontend::Leptos));
        assert!(options.contains(&Frontend::Handlebars));
        assert!(options.contains(&Frontend::Yew));
    }

    #[test]
    fn fill_prefers_the_flag_over_asking() {
        let value = fill(
            Some(Database::Diesel),
            Interaction::Interactive,
            "`--db`",
            || panic!("must not ask when the flag is set"),
        )
        .expect("flag value");
        assert_eq!(value, Database::Diesel);
    }

    #[test]
    fn fill_reports_missing_options_when_non_interactive() {
        let result = fill::<Database>(
            None,
            Interaction::NonInteractive,
            "`--db`",
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
        let err = validate_features(&[Feature::WebSockets, Feature::gRPC])
            .expect_err("grpc has no template content yet");
        let Error::FeaturesNotAvailable(names) = err else {
            panic!("expected FeaturesNotAvailable");
        };
        assert_eq!(names, "grpc");
    }

    #[test]
    fn implemented_features_pass_validation() {
        validate_features(&[Feature::WebSockets, Feature::GraphQL, Feature::RateLimiting])
            .expect("all implemented features pass");
        validate_features(&[]).expect("no features is fine");
    }

    #[test]
    fn prompts_offer_only_implemented_features() {
        assert_eq!(
            implemented_features(),
            vec![Feature::WebSockets, Feature::GraphQL, Feature::RateLimiting]
        );
    }
}
