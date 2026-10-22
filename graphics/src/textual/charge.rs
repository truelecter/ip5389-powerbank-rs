use embedded_graphics::{
  draw_target::DrawTarget,
  geometry::Point,
  mono_font::{ascii, MonoFont},
  pixelcolor::{Rgb565, RgbColor},
};

use crate::textual::format::{fixed, line};
use crate::textual::{Align, Cell};

const FONT: &MonoFont<'_> = &ascii::FONT_9X18_BOLD;
/// Space between the three values
const SPACING: u32 = 20;

/// Pack voltage, state of charge and energy left in one line: `13.37V  89%  9.350Wh`
pub struct Charge {
  volts: Cell<6>,
  percents: Cell<4>,
  watt_hours: Cell<7>,
}

impl Charge {
  /// The line centered in `width`
  pub fn new(top: Point, width: u32, background: Rgb565, highlight: Rgb565) -> Self {
    let advance = FONT.character_size.width;
    let total = (6 + 4 + 7) * advance + 2 * SPACING;
    let x = top.x + ((width - total) / 2) as i32;
    let at = |chars_before: u32, gaps: u32| Point::new(x + (chars_before * advance + gaps * SPACING) as i32, top.y);
    Self {
      volts: cell(at(0, 0), background, highlight),
      percents: cell(at(6, 1), background, highlight),
      watt_hours: cell(at(10, 2), background, highlight),
    }
  }

  pub fn height() -> u32 {
    FONT.character_size.height
  }

  pub fn update<D: DrawTarget<Color = Rgb565>>(&mut self, volts: f32, percents: u8, watt_hours: f32, target: &mut D) -> Result<(), D::Error> {
    self.volts.update(fixed::<6>(volts, 2, "V").as_str(), target)?;
    self.percents.update(line::<4>(format_args!("{percents}%")).as_str(), target)?;
    self.watt_hours.update(fixed::<7>(watt_hours, 3, "Wh").as_str(), target)?;
    Ok(())
  }
}

fn cell<const N: usize>(top_left: Point, background: Rgb565, highlight: Rgb565) -> Cell<N> {
  Cell::fitted(top_left, FONT, Align::Right, Rgb565::WHITE, background).with_highlight(highlight)
}
