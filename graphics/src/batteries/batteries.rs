use embedded_graphics::{draw_target::DrawTarget, geometry::Point, pixelcolor::Rgb565};

use super::Battery;

/// Space between two batteries
const MARGIN: u32 = 5;

/// The four cells of the pack side by side, centered in a row
pub struct Batteries {
  cells: [Battery; 4],
}

impl Batteries {
  pub fn new(top_left: Point, width: u32, background: Rgb565, highlight: Rgb565) -> Self {
    let step = Battery::size().width + MARGIN;
    let total = step * 4 - MARGIN;
    let left = top_left + Point::new(((width - total) / 2) as i32, 0);
    let cells = [0, 1, 2, 3].map(|i| Battery::new(left + Point::new((i * step) as i32, 0), background, highlight));
    Self { cells }
  }

  pub fn height() -> u32 {
    Battery::size().height
  }

  pub fn draw_static<D: DrawTarget<Color = Rgb565>>(&self, target: &mut D) -> Result<(), D::Error> {
    self.cells.iter().try_for_each(|cell| cell.draw_static(target))
  }

  pub fn update<D: DrawTarget<Color = Rgb565>>(&mut self, volts: &[f32; 4], target: &mut D) -> Result<(), D::Error> {
    self.cells.iter_mut().zip(volts).try_for_each(|(cell, &v)| cell.update(v, target))
  }
}
