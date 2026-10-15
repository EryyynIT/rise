use inquire::ui::{Color, RenderConfig, StyleSheet};

/// The wizard theme: steel-gray options, rust-orange selection and answer.
pub fn render_cfg() -> RenderConfig<'static> {
    let option_style = StyleSheet::default().with_fg(Color::Rgb { r: 51, g: 51, b: 51 });
    let answer_style = StyleSheet::default().with_fg(Color::Rgb { r: 247, g: 76, b: 0 });
    RenderConfig::default()
        .with_option(option_style)
        .with_answer(answer_style)
        .with_selected_option(Some(answer_style))
}
