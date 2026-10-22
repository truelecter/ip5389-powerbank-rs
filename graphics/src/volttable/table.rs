use embedded_graphics::{
  draw_target::DrawTarget,
  geometry::{Point, Size},
  pixelcolor::{Rgb565, RgbColor},
  primitives::Rectangle,
};

use super::consts::{BORDER_COLOR, BORDER_SIZE};
use super::VoltTableRow;

/// One row per port: USB1, USB2, DC and the main ports
pub struct VoltTable {
  bounds: Rectangle,
  rows: [VoltTableRow; 4],
}

impl VoltTable {
  pub fn new(top_left: Point, width: u32, background: Rgb565, highlight: Rgb565) -> Self {
    let rows = [(Rgb565::WHITE, "USB1"), (Rgb565::GREEN, "USB2"), (Rgb565::CYAN, "DC"), (Rgb565::YELLOW, "MAIN")];
    let mut i = 0;
    let rows = rows.map(|(color, caption)| {
      // Rows start below the top border
      let y = BORDER_SIZE + i * VoltTableRow::height();
      i += 1;
      VoltTableRow::new(top_left + Point::new(0, y as i32), width, color, background, highlight, caption)
    });
    Self { bounds: Rectangle::new(top_left, Size::new(width, Self::height())), rows }
  }

  pub fn height() -> u32 {
    BORDER_SIZE + 4 * VoltTableRow::height()
  }

  pub fn draw_static<D: DrawTarget<Color = Rgb565>>(&self, target: &mut D) -> Result<(), D::Error> {
    target.fill_solid(&Rectangle::new(self.bounds.top_left, Size::new(self.bounds.size.width, BORDER_SIZE)), BORDER_COLOR)?;
    self.rows.iter().try_for_each(|row| row.draw_static(target))
  }

  /// One `(volts, amps, watts)` per row
  pub fn update<D: DrawTarget<Color = Rgb565>>(&mut self, values: [(Option<f32>, f32, f32); 4], target: &mut D) -> Result<(), D::Error> {
    for (row, (volts, amps, watts)) in self.rows.iter_mut().zip(values) {
      row.update(volts, amps, watts, target)?;
    }
    Ok(())
  }
}
