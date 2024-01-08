use embedded_graphics::{
  draw_target::DrawTarget,
  geometry::{Point, Size},
  pixelcolor::{Rgb565, RgbColor},
  primitives::Rectangle,
  Drawable,
};

use embedded_layout::{
  align::{horizontal, vertical, Align},
  View,
};

use super::Battery;

pub struct Batteries {
  // bounds: RoundedRectangle,
  bounds: Rectangle,
  b1: Battery,
  b2: Battery,
  b3: Battery,
  b4: Battery,
}

impl Batteries {
  pub fn new(top_left: Point, width: u32) -> Self {
    let (b1, b2, b3, b4, bounds) = Self::generate_layout(top_left, width);

    Self {
      bounds,
      b1,
      b2,
      b3,
      b4,
    }
  }

  fn generate_layout(
    top_left: Point,
    width: u32,
  ) -> (Battery, Battery, Battery, Battery, Rectangle) {
    let background_color = Rgb565::BLACK;

    let b1 = Battery::new(Point::zero(), background_color);

    let margin = 5;

    let total_w = b1.size().width * 4 + margin * 3;
    let x_offset = (width - total_w) / 2;

    let b1 = b1.translate(top_left + Point::new(x_offset.try_into().unwrap(), 0));

    let b2 = Battery::new(Point::zero(), background_color)
      .align_to(&b1, horizontal::LeftToRight, vertical::Center)
      .translate(Point::new(margin.try_into().unwrap(), 0));

    let b3 = Battery::new(Point::zero(), background_color)
      .align_to(&b2, horizontal::LeftToRight, vertical::Center)
      .translate(Point::new(margin.try_into().unwrap(), 0));

    let b4 = Battery::new(Point::zero(), background_color)
      .align_to(&b3, horizontal::LeftToRight, vertical::Center)
      .translate(Point::new(margin.try_into().unwrap(), 0));

    let bounds = Rectangle::new(top_left, Size::new(width, b1.size().height));

    return (b1, b2, b3, b4, bounds);
  }

  pub fn draw_static<D: DrawTarget<Color = Rgb565>>(
    &self,
    target: &mut D,
  ) -> Result<&Self, D::Error> {
    self.b1.draw_static(target)?;
    self.b2.draw_static(target)?;
    self.b3.draw_static(target)?;
    self.b4.draw_static(target)?;

    Ok(self)
  }

  pub fn set_voltage1(&mut self, voltage: f32) -> &Self {
    self.b1.set_voltage(voltage);

    self
  }

  pub fn set_voltage2(&mut self, voltage: f32) -> &Self {
    self.b2.set_voltage(voltage);

    self
  }

  pub fn set_voltage3(&mut self, voltage: f32) -> &Self {
    self.b3.set_voltage(voltage);

    self
  }

  pub fn set_voltage4(&mut self, voltage: f32) -> &Self {
    self.b4.set_voltage(voltage);

    self
  }
}

impl View for Batteries {
  #[inline]
  fn translate_impl(&mut self, by: Point) {
    let top_left = self.bounds.top_left + by;

    let (b1, b2, b3, b4, bounds) =
      Self::generate_layout(top_left, self.bounds.size.width);

    self.b1 = b1;
    self.b2 = b2;
    self.b3 = b3;
    self.b4 = b4;
    self.bounds = bounds;
  }

  #[inline]
  fn bounds(&self) -> Rectangle {
    self.bounds
  }
}

impl<'a> Drawable for Batteries {
  type Color = Rgb565;
  type Output = ();

  fn draw<D: DrawTarget<Color = Self::Color>>(&self, target: &mut D) -> Result<(), D::Error> {
    self.b1.draw(target)?;
    self.b2.draw(target)?;
    self.b3.draw(target)?;
    self.b4.draw(target)?;

    Ok(())
  }
}
