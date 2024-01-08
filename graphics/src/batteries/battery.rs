use embedded_graphics::{
  draw_target::DrawTarget,
  geometry::{Point, Size},
  mono_font::MonoTextStyleBuilder,
  pixelcolor::{Rgb565, RgbColor},
  primitives::{
    PrimitiveStyleBuilder, Rectangle, RoundedRectangle, StrokeAlignment, StyledDrawable,
  },
  text::Text,
  Drawable,
};

use embedded_layout::{
  align::{horizontal, vertical, Align},
  View,
};

use super::consts::{ANODE_SIZE, BORDER_COLOR, BORDER_SIZE, FONT};

use crate::utils::float_to_fixed;

pub struct Battery {
  voltage: f32,
  bounds: Rectangle,
  background_color: Rgb565,
}

impl Battery {
  pub fn new(top_left: Point, background_color: Rgb565) -> Self {
    let corner_radius = Size::new_equal(2) + Size::new_equal(BORDER_SIZE);
    let height = FONT.character_size.height;
    let width = FONT.character_size.width * 5 + 2;

    let bounds = Rectangle::new(
      top_left,
      Size::new(width, height) + corner_radius + Size::new(ANODE_SIZE.width, 0),
    );

    Self {
      bounds,
      background_color,
      voltage: 0.0,
    }
  }

  pub fn draw_static<D: DrawTarget<Color = Rgb565>>(
    &self,
    target: &mut D,
  ) -> Result<&Self, D::Error> {
    let anode_style = PrimitiveStyleBuilder::new()
      .fill_color(BORDER_COLOR)
      .build();

    let anode_rect = Rectangle::new(Point::zero(), ANODE_SIZE).align_to(
      &self.bounds,
      horizontal::Left,
      vertical::Center,
    );

    RoundedRectangle::new(
      anode_rect,
      embedded_graphics::primitives::CornerRadii {
        top_left: Size::new_equal(2),
        bottom_left: Size::new_equal(2),
        top_right: Size::zero(),
        bottom_right: Size::zero(),
      },
    )
    .draw_styled(&anode_style, target)?;

    let border_style = PrimitiveStyleBuilder::new()
      .stroke_width(BORDER_SIZE)
      .stroke_color(BORDER_COLOR)
      .stroke_alignment(StrokeAlignment::Inside)
      .build();

    let body_rect = Rectangle::new(
      Point::zero(),
      Size::new(
        self.bounds.size.width - ANODE_SIZE.width,
        self.bounds.size.height,
      ),
    )
    .align_to(&anode_rect, horizontal::LeftToRight, vertical::Center);

    RoundedRectangle::with_equal_corners(body_rect, Size::new_equal(2))
      .draw_styled(&border_style, target)?;

    Ok(self)
  }

  pub fn set_voltage(&mut self, voltage: f32) -> &Self {
    self.voltage = voltage;

    self
  }
}

impl View for Battery {
  #[inline]
  fn translate_impl(&mut self, by: Point) {
    // make sure you don't accidentally call `translate`!
    self.bounds.translate_mut(by);
  }

  #[inline]
  fn bounds(&self) -> Rectangle {
    self.bounds
  }
}

impl<'a> Drawable for Battery {
  type Color = Rgb565;
  type Output = ();

  fn draw<D: DrawTarget<Color = Self::Color>>(&self, target: &mut D) -> Result<(), D::Error> {
    let style = MonoTextStyleBuilder::new()
      .background_color(self.background_color)
      .text_color(Rgb565::WHITE)
      .font(&FONT)
      .build();

    let volts = float_to_fixed::<5>(self.voltage);
    let volts = core::str::from_utf8(&volts).unwrap();

    let ch_size = style.font;

    let y_diff = self.bounds.size.height - ch_size.baseline;
    let y_diff = y_diff / 2 + y_diff % 2; // ceil for ints

    let center = self.bounds.top_left
      + Point::new(
        (self.bounds.size.width / 2 + 1) as i32,
        (self.bounds.size.height - y_diff - 1) as i32,
      );

    let text_anchor = center;

    Text::with_alignment(
      volts,
      text_anchor,
      style,
      embedded_graphics::text::Alignment::Center,
    )
    .draw(target)?;

    Ok(())
  }
}
