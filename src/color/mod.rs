// Brand palette: the full color set is kept even where not all colors are used yet.
#[allow(dead_code)]
mod palette;
pub use palette::*;

pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8) -> Color {
        Color { r, g, b }
    }
}

impl From<Color> for inquire::ui::Color {
    fn from(value: Color) -> Self {
        inquire::ui::Color::Rgb {
            r: value.r,
            g: value.g,
            b: value.b,
        }
    }
}

impl From<Color> for colored::CustomColor {
    fn from(value: Color) -> Self {
        colored::CustomColor {
            r: value.r,
            g: value.g,
            b: value.b,
        }
    }
}
