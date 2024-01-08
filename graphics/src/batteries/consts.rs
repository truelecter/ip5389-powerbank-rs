use embedded_graphics::{
  geometry::Size,
  mono_font::{ascii, MonoFont},
  pixelcolor::{Rgb565, RgbColor},
};

// TODO: get rid of these

pub const FONT: &MonoFont<'_> = &ascii::FONT_7X13;
pub const BORDER_SIZE: u32 = 1;
pub const PADDING: u32 = 1;
pub const BORDER_COLOR: Rgb565 = Rgb565::WHITE;
pub const ANODE_SIZE: Size = Size::new(2, 7);
pub const CORNER_RADIUS: u32 = 2;
