use embedded_graphics::pixelcolor::{Rgb565, RgbColor};

/// `color` at `percentage` of its brightness
pub fn darken(color: Rgb565, percentage: u32) -> Rgb565 {
  let scale = |channel: u8| (u32::from(channel) * percentage / 100) as u8;
  Rgb565::new(scale(color.r()), scale(color.g()), scale(color.b()))
}
