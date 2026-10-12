use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter};

use crate::wizard::{multi_select, select};
use crate::error::Result;
use crate::spec::{
    Backend, BackendSpec, Database, Deployment, Feature, Frontend, FrontendSpec, FullstackSpec,
    Testing, WebAppSpec,
};

/// Shape of a web app — decides which stack questions the wizard asks.
#[derive(Debug, Clone, Copy, Display, EnumIter)]
enum WebAppKind {
    Backend,
    Frontend,
    Fullstack,
}

/// Interactively collect a complete [`WebAppSpec`].
pub fn prompt_web_spec() -> Result<WebAppSpec> {
    match select("Select a webapp kind", WebAppKind::iter().collect())? {
        WebAppKind::Backend => {
            let backend = select("Select a backend", Backend::iter().collect())?;
            Ok(WebAppSpec::Backend(prompt_backend_spec(backend)?))
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
            let backend = prompt_backend_spec(backend)?;
            Ok(WebAppSpec::Fullstack(FullstackSpec { frontend, backend }))
        }
    }
}

/// Ask the questions shared by backend-only and fullstack apps.
fn prompt_backend_spec(backend: Backend) -> Result<BackendSpec> {
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
}
