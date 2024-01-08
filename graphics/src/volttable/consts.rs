use embedded_graphics::{
  mono_font::{ascii::FONT_8X13_BOLD, MonoFont},
  pixelcolor::{Rgb565, RgbColor},
};

// TODO: get rid of these
pub const TABLE_FONT: &MonoFont<'_> = &FONT_8X13_BOLD;
pub const BORDER_SIZE: u32 = 2;
pub const PADDING: u32 = 1;
pub const BORDER_COLOR: Rgb565 = Rgb565::WHITE;
