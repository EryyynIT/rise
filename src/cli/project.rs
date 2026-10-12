use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter};

use crate::cli::{select, web::prompt_web_spec};
use crate::error::Result;
use crate::spec::ProjectSpec;

/// Project kinds offered by the wizard's first question.
#[derive(Debug, Clone, Copy, Display, EnumIter)]
enum ProjectKind {
    WebApp,
    CliApp,
    Library,
    Embedded,
    Game,
}

/// Ask what to build and collect a complete [`ProjectSpec`].
pub fn prompt_project_spec() -> Result<ProjectSpec> {
    let kind = select(
        "What type of project do you want to create?",
        ProjectKind::iter().collect(),
    )?;
    let spec = match kind {
        ProjectKind::WebApp => ProjectSpec::WebApp(prompt_web_spec()?),
        ProjectKind::CliApp => ProjectSpec::CliApp,
        ProjectKind::Library => ProjectSpec::Library,
        ProjectKind::Embedded => ProjectSpec::Embedded,
        ProjectKind::Game => ProjectSpec::Game,
    };
    Ok(spec)
}
