//! Interactive wizard: collects a complete [`crate::spec::ProjectSpec`].

pub mod project;
mod render;
pub mod web;

use std::fmt::Display;

use inquire::{MultiSelect, Select};

use crate::wizard::render::render_cfg;
use crate::error::Result;

/// Single-choice prompt with the rise theme.
pub(crate) fn select<T: Display>(message: &str, options: Vec<T>) -> Result<T> {
    Ok(Select::new(message, options)
        .with_render_config(render_cfg())
        .prompt()?)
}

/// Multi-choice prompt with the rise theme.
pub(crate) fn multi_select<T: Display>(message: &str, options: Vec<T>) -> Result<Vec<T>> {
    Ok(MultiSelect::new(message, options)
        .with_render_config(render_cfg())
        .prompt()?)
}
