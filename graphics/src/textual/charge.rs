use embedded_graphics::{
  draw_target::DrawTarget,
  geometry::{Point, Size},
  mono_font::{MonoTextStyleBuilder, ascii, MonoFont},
  pixelcolor::{Rgb565, RgbColor},
  primitives::
    Rectangle
  ,
  text::Text,
  Drawable,
};

use embedded_layout::{
  View, layout::linear::{LinearLayout, FixedMargin}, object_chain::Chain, align::{Align, horizontal, vertical}}
;

use crate::utils::{float_to_fixed_with_suffix, to_integer_percents, float_to_fixed_with_suffix2};

const FONT: &MonoFont<'_> = &ascii::FONT_9X18_BOLD;

pub struct Charge {
  power: f32,
  voltage: f32,
  percents: f32,
  bounds: Rectangle,
}

impl Charge {
  pub fn new(top_left: Point) -> Self {
    let height = FONT.character_size.height;
    let width = FONT.character_size.width * (6 + 4 +  6) + 20 * 2;

    let bounds = Rectangle::new(
      top_left,
      Size::new(width, height),
    );

    Self {
      bounds,
      power: 0.0,
      voltage: 0.0,
      percents: 0.0,
    }
  }

  pub fn draw_static<D: DrawTarget<Color = Rgb565>>(
    &self,
    _: &mut D,
  ) -> Result<&Self, D::Error> {
    Ok(self)
  }

  pub fn set_voltage(&mut self, voltage: f32) -> &mut Self {
    self.voltage = voltage;

    self
  }

  pub fn set_power(&mut self, power: f32) -> &mut Self {
    self.power = power;

    self
  }

  pub fn set_percents(&mut self, percents: f32) -> &mut Self {
    self.percents = percents;

    self
  }
}

impl View for Charge {
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

impl<'a> Drawable for Charge {
  type Color = Rgb565;
  type Output = ();

  fn draw<D: DrawTarget<Color = Self::Color>>(&self, target: &mut D) -> Result<(), D::Error> {
    let style = MonoTextStyleBuilder::new()
      .background_color(Rgb565::BLACK)
      .text_color(Rgb565::WHITE)
      .font(&FONT)
      .build();

    let volts = float_to_fixed_with_suffix::<6>(self.voltage, b'V');
    let volts = core::str::from_utf8(&volts).unwrap();

    let percents = to_integer_percents(self.percents);
    let percents = core::str::from_utf8(&percents).unwrap();

    let watt_hours = float_to_fixed_with_suffix2::<7>(self.power, [b'W', b'h']);
    let watt_hours = core::str::from_utf8(&watt_hours).unwrap();

    LinearLayout::horizontal(
      Chain::new(Text::new(volts, Point::zero(), style))
        .append(Text::new(percents, Point::zero(), style))
        .append(Text::new(watt_hours, Point::zero(), style))
    )
      .with_spacing(FixedMargin(20))
      .arrange()
      .align_to(&self.bounds, horizontal::Center, vertical::Center)
      .draw(target)?;

    Ok(())
  }
}
