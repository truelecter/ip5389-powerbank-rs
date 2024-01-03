use embedded_graphics::{
  geometry::{Point, Size},
  primitives::{Rectangle, StyledDrawable, PrimitiveStyleBuilder},
  pixelcolor::{Rgb565, RgbColor},
  Drawable,
  draw_target::DrawTarget
};
use embedded_layout::{
  View,
  align::{Align, horizontal, vertical}
};

use super::VoltTableRow;
use super::consts::{
  BORDER_COLOR, BORDER_SIZE, PADDING, TABLE_FONT
};

pub struct VoltTable<'a> {
  usb: VoltTableRow<'a>,
  ext1: VoltTableRow<'a>,
  ext2: VoltTableRow<'a>,
  dc: VoltTableRow<'a>,

  bounds: Rectangle,
}

impl<'a> VoltTable<'a> {
  pub fn new(
    top_left: Point, width: u32,
  ) -> Self {
    let (usb, ext1, ext2, dc) = Self::generate_rows_from_point(top_left, width);

    let bounds = Rectangle::with_corners(
      top_left,
      dc.bounds().bottom_right().unwrap()
    );

    Self {
      bounds, usb, ext1, ext2, dc
    }
  }

  fn generate_rows_from_point(top_left: Point, width: u32) -> (
    VoltTableRow<'a>,
    VoltTableRow<'a>,
    VoltTableRow<'a>,
    VoltTableRow<'a>,
  ) {
    // start with border offset. it will be drawn later
    let rows_top_left = top_left + Point::new(0, BORDER_SIZE as i32);

    let usb = VoltTableRow::new(
      rows_top_left,
      width,
      Rgb565::WHITE,
      Rgb565::WHITE,
      "USB",
    );

    let ext1 = VoltTableRow::new(
      Point::zero(),
      width,
      Rgb565::GREEN,
      Rgb565::WHITE,
      "EXT1",
    ).align_to(&usb, horizontal::NoAlignment, vertical::TopToBottom);

    let ext2 = VoltTableRow::new(
      Point::zero(),
      width,
      Rgb565::RED,
      Rgb565::WHITE,
      "EXT2",
    ).align_to(&ext1, horizontal::NoAlignment, vertical::TopToBottom);

    let dc = VoltTableRow::new(
      Point::zero(),
      width,
      Rgb565::YELLOW,
      Rgb565::WHITE,
      "DC",
    ).align_to(&ext2, horizontal::NoAlignment, vertical::TopToBottom);

    (usb, ext1, ext2, dc)
  }

  pub fn draw_initial<D: DrawTarget<Color = Rgb565>>(&self, target: &mut D) -> Result<(), D::Error> {
    let border_style = PrimitiveStyleBuilder::new()
      .stroke_width(BORDER_SIZE)
      .stroke_color(BORDER_COLOR)
      .stroke_alignment(embedded_graphics::primitives::StrokeAlignment::Inside)
      .build();

    Rectangle::new(self.bounds.top_left, Size::new(self.bounds.size.width, BORDER_SIZE))
      .draw_styled(&border_style, target)?;

    self.usb.draw_initial(target)?;
    self.ext1.draw_initial(target)?;
    self.ext2.draw_initial(target)?;
    self.dc.draw_initial(target)?;

    Ok(())
  }
}

impl View for VoltTable<'_> {
  #[inline]
  fn translate_impl(&mut self, by: Point) {
    let top_left = self.bounds.top_left + by;

    let (usb, ext1, ext2, dc) = Self::generate_rows_from_point(top_left, self.bounds.size.width);

    self.usb = usb;
    self.ext1 = ext1;
    self.ext2 = ext2;
    self.dc = dc;

    self. bounds = Rectangle::with_corners(
      top_left,
      self.dc.bounds().bottom_right().unwrap()
    );
  }

  #[inline]
  fn bounds(&self) -> Rectangle {
      self.bounds
  }
}

impl<'a> Drawable for VoltTable<'a> {
  type Color = Rgb565;
  type Output = ();

  fn draw<D: DrawTarget<Color = Self::Color>>(&self, target: &mut D) -> Result<(), D::Error> {
    self.usb.draw(target)?;
    self.ext1.draw(target)?;
    self.ext2.draw(target)?;
    self.dc.draw(target)?;

    Ok(())
  }
}