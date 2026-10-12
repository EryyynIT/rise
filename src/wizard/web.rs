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
            Ok(WebAppSpec::Backend(prompt_backend(backend)?))
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
            let backend = prompt_backend(backend)?;
            Ok(WebAppSpec::Fullstack(FullstackSpec { frontend, backend }))
        }
    }
}

/// Ask all backend questions, shared by backend-only and fullstack apps.
fn prompt_backend(backend: Backend) -> Result<BackendSpec> {
    let database = select("Select a database", Database::iter().collect())?;
    let deployment = select("Select a deployment option", Deployment::iter().collect())?;
    let testing = multi_select("Select testing options", Testing::iter().collect())?;
    let features = multi_select("Select features", Feature::iter().collect())?;
    Ok(BackendSpec {
        backend,
        database,
        deployment,
        testing,
        features,
    })
}

/// Complete the backend spec, asking only for what the flags left open.
fn complete_backend(
    backend: Backend,
    draft: BackendDraft,
    mode: Interaction,
) -> Result<BackendSpec> {
    let database = fill(draft.database, mode, "`--db`", || {
        select("Select a database", Database::iter().collect())
    })?;
    let deployment = fill(draft.deployment, mode, "`--deployment`", || {
        select("Select a deployment option", Deployment::iter().collect())
    })?;
    let testing = fill_multi(draft.testing, mode, || {
        multi_select("Select testing options", Testing::iter().collect())
    })?;
    let features = fill_multi(draft.features, mode, || {
        multi_select("Select features", Feature::iter().collect())
    })?;
    Ok(BackendSpec {
        backend,
        database,
        deployment,
        testing,
        features,
    })
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
    let mut options = vec![Svelte, React, Vue, Minijinja, Handlebars];
    let specific = if has_backend {
        [Yew, Tera, Askama]
    } else {
        [Leptos, Seed, Sauron]
    };
    options.extend(specific);
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
        assert!(!options.contains(&Frontend::Leptos));
    }

    #[test]
    fn frontend_options_without_backend() {
        let options = frontend_options(false);
        assert!(options.contains(&Frontend::Leptos));
        assert!(options.contains(&Frontend::Handlebars));
        assert!(!options.contains(&Frontend::Yew));
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
}
