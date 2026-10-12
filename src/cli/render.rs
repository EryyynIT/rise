use inquire::ui::{RenderConfig, StyleSheet};

use crate::color::{RUST_ORANGE, STEEL_GRAY};

pub fn render_cfg() -> RenderConfig<'static> {
    let option_style = StyleSheet::default().with_fg(STEEL_GRAY.into());
    let answer_style = StyleSheet::default().with_fg(RUST_ORANGE.into());
    RenderConfig::default()
        .with_option(option_style)
        .with_answer(answer_style)
        .with_selected_option(Some(answer_style))
}
