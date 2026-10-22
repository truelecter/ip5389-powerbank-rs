use embedded_graphics::{
  draw_target::DrawTarget,
  geometry::{Point, Size},
  pixelcolor::Rgb565,
  primitives::Rectangle,
};

use super::consts::{BORDER_COLOR, BORDER_SIZE, PADDING, TABLE_FONT};
use crate::textual::format::fixed;
use crate::textual::{Align, Cell};
use crate::utils::darken;

/// Characters of a value: `16.40V`, `2.100A`
const CHARS: usize = 6;
/// Current above which a port counts as in use: its row shows in full color
const ACTIVE_A: f32 = 0.1;

/// One port: name, volts, amps and watts in bordered cells. The row draws its left, right and
/// bottom borders; the table draws the top one.
pub struct VoltTableRow {
  bounds: Rectangle,
  active_color: Rgb565,
  inactive_color: Rgb565,
  name: Cell<CHARS>,
  /// Volts, amps, watts
  values: [Cell<CHARS>; 3],
  caption: &'static str,
}

impl VoltTableRow {
  pub fn new(top_left: Point, width: u32, active_color: Rgb565, background: Rgb565, highlight: Rgb565, caption: &'static str) -> Self {
    let bounds = Rectangle::new(top_left, Size::new(width, Self::height()));
    let inactive_color = darken(active_color, 50);

    // Four cells between five borders; the name cell takes the pixels left over
    let cell_width = (width - 5 * BORDER_SIZE) / 4;
    let name_width = width - 5 * BORDER_SIZE - 3 * cell_width;
    let cell_height = TABLE_FONT.character_size.height + 2 * PADDING;
    let cell = |x: u32, width: u32| Rectangle::new(top_left + Point::new(x as i32, 0), Size::new(width, cell_height));
    let value_x = |i: u32| BORDER_SIZE * 2 + name_width + i * (cell_width + BORDER_SIZE);

    let name = Cell::new(cell(BORDER_SIZE, name_width), TABLE_FONT, Align::Center, inactive_color, background);
    let values = [0, 1, 2].map(|i| {
      Cell::new(cell(value_x(i), cell_width), TABLE_FONT, Align::Center, inactive_color, background).with_highlight(highlight)
    });
    Self { bounds, active_color, inactive_color, name, values, caption }
  }

  pub fn height() -> u32 {
    TABLE_FONT.character_size.height + 2 * PADDING + BORDER_SIZE
  }

  pub fn bounds(&self) -> Rectangle {
    self.bounds
  }

  /// The vertical borders between the cells and the bottom border
  pub fn draw_static<D: DrawTarget<Color = Rgb565>>(&self, target: &mut D) -> Result<(), D::Error> {
    let (top_left, size) = (self.bounds.top_left, self.bounds.size);
    let mut borders = [self.name.area(), self.values[0].area(), self.values[1].area(), self.values[2].area()]
      .map(|cell| cell.top_left.x - BORDER_SIZE as i32);
    borders[0] = top_left.x;
    for x in borders.into_iter().chain([top_left.x + (size.width - BORDER_SIZE) as i32]) {
      target.fill_solid(&Rectangle::new(Point::new(x, top_left.y), Size::new(BORDER_SIZE, size.height)), BORDER_COLOR)?;
    }
    let bottom = Point::new(top_left.x, top_left.y + (size.height - BORDER_SIZE) as i32);
    target.fill_solid(&Rectangle::new(bottom, Size::new(size.width, BORDER_SIZE)), BORDER_COLOR)
  }

  /// Shows the readings; a row without a voltage shows a dash. The row dims while the port
  /// carries less than 0.1 A.
  pub fn update<D: DrawTarget<Color = Rgb565>>(&mut self, volts: Option<f32>, amps: f32, watts: f32, target: &mut D) -> Result<(), D::Error> {
    let color = if amps.abs() > ACTIVE_A { self.active_color } else { self.inactive_color };
    self.name.set_color(color);
    self.values.iter_mut().for_each(|cell| cell.set_color(color));

    self.name.update(self.caption, target)?;
    let volts = volts.map(|v| fixed::<CHARS>(v, 2, "V"));
    let [v, a, w] = &mut self.values;
    v.update(volts.as_ref().map_or("-", |v| v.as_str()), target)?;
    a.update(fixed::<CHARS>(amps, 3, "A").as_str(), target)?;
    w.update(fixed::<CHARS>(watts, 2, "W").as_str(), target)?;
    Ok(())
  }
}
