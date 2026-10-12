use inquire::{MultiSelect, Select};
use strum::IntoEnumIterator;

use crate::cli::render::render_cfg;
use crate::config::web::*;

pub fn prompt_web_config(config: &mut WebAppConfig) -> Result<(), Box<dyn std::error::Error>> {
    let kind = Select::new("Select a webapp kind", WebApp::iter().collect())
        .with_render_config(render_cfg())
        .prompt()?;
    config.kind = Some(kind);

    if let Some(backends) = get_backends_options(&config) {
        let backend = Select::new("Select a backend", backends)
            .with_render_config(render_cfg())
            .prompt()?;

        config.backend = Some(backend);
    }

    if let Some(frontends) = get_frontends_options(&config) {
        let frontend = Select::new("Select a frontend tech", frontends)
            .with_render_config(render_cfg())
            .prompt()?;

        config.frontend = Some(frontend);
    }

    if let Some(databases) = get_databases_options(&config) {
        let database = Select::new("Select a database", databases)
            .with_render_config(render_cfg())
            .prompt()?;

        config.database = Some(database);
    }

    if let Some(deployments) = get_deployment_options(&config) {
        let deployment = Select::new("Select a deployment option", deployments)
            .with_render_config(render_cfg())
            .prompt()?;

        config.deployment = Some(deployment);
    }

    if let Some(testings) = get_testing_options(&config) {
        let testings = MultiSelect::new("Select testing options", testings)
            .with_render_config(render_cfg())
            .prompt()?;

        config.testings = testings;
    }

    if let Some(features) = get_features_options(&config) {
        let features = MultiSelect::new("Select features", features)
            .with_render_config(render_cfg())
            .prompt()?;

        config.features = features;
    }
    Ok(())
}

pub fn get_backends_options(config: &WebAppConfig) -> Option<Vec<Backend>> {
    if config.kind.clone().unwrap() == WebApp::Frontend {
        return None;
    }

    let backends = Backend::iter().collect();
    Some(backends)
}
pub fn get_frontends_options(config: &WebAppConfig) -> Option<Vec<Frontend>> {
    if config.kind.clone().unwrap() == WebApp::Backend {
        return None;
    }

    let common_frontends = vec![
        Frontend::Svelte,
        Frontend::React,
        Frontend::Vue,
        Frontend::Minijinja,
        Frontend::Handlebars,
    ];

    match config.backend {
        Some(_) => {
            let backend_specific_frontends = vec![Frontend::Yew, Frontend::Tera, Frontend::Askama];
            Some([common_frontends, backend_specific_frontends].concat())
        }
        None => {
            let no_backend_frontends = vec![Frontend::Leptos, Frontend::Seed, Frontend::Sauron];
            Some([common_frontends, no_backend_frontends].concat())
        }
    }
}

#[allow(clippy::all)]
pub fn get_databases_options(config: &WebAppConfig) -> Option<Vec<Database>> {
    if config.backend.is_none() {
        return None;
    }

    let databases = Database::iter().collect();
    Some(databases)
}

pub fn get_deployment_options(config: &WebAppConfig) -> Option<Vec<Deployment>> {
    let frontends_without_deployment = vec![
        Frontend::Tera,
        Frontend::Askama,
        Frontend::Minijinja,
        Frontend::Handlebars,
    ];
    if config.backend.is_none()
        && frontends_without_deployment.contains(&config.frontend.clone().unwrap())
    {
        return None;
    }

    let deployments = Deployment::iter().collect();
    Some(deployments)
}

#[allow(clippy::all)]
pub fn get_testing_options(config: &WebAppConfig) -> Option<Vec<Testing>> {
    if config.backend.is_none() {
        return None;
    }

    let testings = Testing::iter().collect();
    Some(testings)
}

#[allow(clippy::all)]
pub fn get_features_options(config: &WebAppConfig) -> Option<Vec<Features>> {
    if config.backend.is_none() {
        return None;
    }

    let features = Features::iter().collect();
    Some(features)
}
