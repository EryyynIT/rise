//! Project templates and the engine that renders them.
//!
//! A template is a directory with a `template.toml` manifest plus arbitrary
//! files. Built-in templates are embedded into the binary from
//! `assets/templates/`, so `rise` works offline as a single executable.

mod engine;
mod template;

pub(crate) use engine::generate;
pub(crate) use template::list_embedded;
