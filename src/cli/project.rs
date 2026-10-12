use inquire::Select;
use strum::IntoEnumIterator;

use crate::cli::render::render_cfg;
use crate::{cli::web::prompt_web_config, config::project::ProjectConfig};

pub fn prompt_project_config() -> crate::error::Result<ProjectConfig> {
    let kinds = ProjectConfig::iter().collect();
    let mut project = Select::new("What type of project do you want to create?", kinds)
        .with_render_config(render_cfg())
        .prompt()?;
    match &mut project {
        ProjectConfig::WebApp(config) => {
            prompt_web_config(config)?;
        }
        ProjectConfig::CliApp => (),
        ProjectConfig::Library => (),
        ProjectConfig::Embedded => (),
        ProjectConfig::Game => (),
    };
    Ok(project)
}
