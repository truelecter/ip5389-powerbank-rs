use embedded_graphics::{
  draw_target::DrawTarget,
  geometry::{Point, Size},
  pixelcolor::{Rgb565, RgbColor},
  primitives::{CornerRadii, PrimitiveStyleBuilder, Rectangle, RoundedRectangle, StrokeAlignment, StyledDrawable},
};

use super::consts::{ANODE_SIZE, BORDER_COLOR, BORDER_SIZE, CORNER_RADIUS, FONT};
use crate::textual::format::fixed;
use crate::textual::{Align, Cell};

/// Characters of the voltage: `4.181`
const CHARS: usize = 5;

/// A cell drawn as a battery lying on its side, anode to the left, with its voltage inside
pub struct Battery {
  bounds: Rectangle,
  voltage: Cell<CHARS>,
}

impl Battery {
  pub fn new(top_left: Point, background: Rgb565, highlight: Rgb565) -> Self {
    let size = Self::size();
    let bounds = Rectangle::new(top_left, size);
    let inside = Rectangle::new(
      top_left + Point::new((ANODE_SIZE.width + BORDER_SIZE) as i32, BORDER_SIZE as i32),
      size - Size::new(ANODE_SIZE.width + 2 * BORDER_SIZE, 2 * BORDER_SIZE),
    );
    let voltage = Cell::new(inside, FONT, Align::Center, Rgb565::WHITE, background).with_highlight(highlight);
    Self { bounds, voltage }
  }

  pub fn size() -> Size {
    let corners = Size::new_equal(CORNER_RADIUS + BORDER_SIZE);
    let text = Size::new(FONT.character_size.width * CHARS as u32 + 4, FONT.character_size.height);
    text + corners + Size::new(ANODE_SIZE.width, 0)
  }

  /// The outline
  pub fn draw_static<D: DrawTarget<Color = Rgb565>>(&self, target: &mut D) -> Result<(), D::Error> {
    let anode_top = self.bounds.top_left.y + (self.bounds.size.height - ANODE_SIZE.height) as i32 / 2;
    let anode = Rectangle::new(Point::new(self.bounds.top_left.x, anode_top), ANODE_SIZE);
    let radius = Size::new_equal(CORNER_RADIUS);
    let anode_corners = CornerRadii { top_left: radius, bottom_left: radius, top_right: Size::zero(), bottom_right: Size::zero() };
    RoundedRectangle::new(anode, anode_corners)
      .draw_styled(&PrimitiveStyleBuilder::new().fill_color(BORDER_COLOR).build(), target)?;

    let border = PrimitiveStyleBuilder::new()
      .stroke_width(BORDER_SIZE)
      .stroke_color(BORDER_COLOR)
      .stroke_alignment(StrokeAlignment::Inside)
      .build();
    let body = Rectangle::new(
      self.bounds.top_left + Point::new(ANODE_SIZE.width as i32, 0),
      self.bounds.size - Size::new(ANODE_SIZE.width, 0),
    );
    RoundedRectangle::with_equal_corners(body, radius).draw_styled(&border, target)
  }

  pub fn update<D: DrawTarget<Color = Rgb565>>(&mut self, volts: f32, target: &mut D) -> Result<(), D::Error> {
    self.voltage.update(fixed::<CHARS>(volts, 3, "").as_str(), target)?;
    Ok(())
  }
}
