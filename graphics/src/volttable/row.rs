use embedded_graphics::{
  draw_target::DrawTarget,
  geometry::{AnchorPoint, Point, Size},
  mono_font::MonoTextStyle,
  pixelcolor::Rgb565,
  primitives::{PrimitiveStyleBuilder, Rectangle, StyledDrawable},
  text::Text,
  Drawable,
};

use embedded_layout::{
  align::{horizontal, vertical, Align},
  View,
};

use super::consts::{BORDER_COLOR, BORDER_SIZE, PADDING, TABLE_FONT};
use crate::utils::{darken, float_to_fixed_with_suffix};

pub struct VoltTableRow<'a> {
  bounds: Rectangle,
  voltage: f32,
  current: f32,
  power: f32,
  active_text_color: Rgb565,
  inactive_text_color: Rgb565,
  background_color: Rgb565,
  caption: &'a str,
  cells: VoltTableCells,
}

struct VoltTableCell {
  bounds: Rectangle,
  right_border: Rectangle,
}

impl VoltTableCell {
  fn new(left_border: &Rectangle, size: &Size) -> Self {
    let bounds = Rectangle::new(Point::zero(), Size::clone(size))
      .align_to(left_border, horizontal::LeftToRight, vertical::Top)
      .translate(Point {
        x: 0,
        y: PADDING as i32,
      });

    Self {
      bounds,
      right_border: left_border.align_to(&bounds, horizontal::LeftToRight, vertical::NoAlignment),
    }
  }

  fn next_to_with_size(cell: &Self, size: &Size) -> VoltTableCell {
    VoltTableCell::new(&cell.right_border, size)
  }

  fn next_to(cell: &Self) -> VoltTableCell {
    VoltTableCell::next_to_with_size(cell, &cell.bounds.size)
  }

  fn draw_text<D: DrawTarget<Color = Rgb565>>(
    &self,
    text: &str,
    background_color: Rgb565,
    style: MonoTextStyle<Rgb565>,
    target: &mut D,
  ) -> Result<(), D::Error> {
    let ch_size = style.font;

    let y_diff = self.bounds.size.height - ch_size.baseline;
    let y_diff = y_diff / 2 + y_diff % 2; // ceil for ints

    let center_diff = Point::new(0, y_diff as i32);

    let text_anchor = self.bounds.anchor_point(AnchorPoint::BottomCenter) - center_diff;

    // TODO: clear cell more efficiently. May be use framebuffer
    self.bounds.draw_styled(
      &PrimitiveStyleBuilder::new()
        .fill_color(background_color)
        .build(),
      target,
    )?;

    Text::with_alignment(
      text,
      text_anchor,
      style,
      embedded_graphics::text::Alignment::Center,
    )
    .draw(target)?;

    Ok(())
  }
}

struct VoltTableCells {
  left_border: Rectangle,
  name: VoltTableCell,
  voltage: VoltTableCell,
  current: VoltTableCell,
  power: VoltTableCell,
}

impl VoltTableCells {
  fn for_bounds(bounds: &Rectangle) -> Self {
    let size = bounds.size;

    let left_border = Rectangle::new(bounds.top_left, Size::new(BORDER_SIZE, size.height));

    let cell_size = Size {
      width: (size.width - 5 * BORDER_SIZE) / 4,
      height: size.height - (2 * PADDING + BORDER_SIZE),
    };

    let diff = size.width - cell_size.width * 4 - 5 * BORDER_SIZE;

    let first_cell_size = Size {
      width: cell_size.width + diff,
      height: cell_size.height,
    };

    let name_cell = VoltTableCell::new(&left_border, &first_cell_size);

    let volts_cell = VoltTableCell::next_to_with_size(&name_cell, &cell_size);

    let amps_cell = VoltTableCell::next_to(&volts_cell);

    let watts_cell = VoltTableCell::next_to(&amps_cell);

    VoltTableCells {
      name: name_cell,
      voltage: volts_cell,
      current: amps_cell,
      power: watts_cell,
      left_border,
    }
  }
}

impl<'a> VoltTableRow<'a> {
  pub fn new(
    top_left: Point,
    width: u32,
    active_text_color: Rgb565,
    background_color: Rgb565,
    caption: &'a str,
  ) -> Self {
    let size = Size {
      width: width as u32,
      height: TABLE_FONT.character_size.height + 2 * PADDING + BORDER_SIZE,
    };

    let inactive_text_color = darken(active_text_color, 50);

    let bounds = Rectangle::new(top_left, size);

    Self {
      bounds,
      caption,
      active_text_color,
      inactive_text_color,
      background_color,
      cells: VoltTableCells::for_bounds(&bounds),
      voltage: 0 as f32,
      current: 0 as f32,
      power: 0 as f32,
    }
  }

  pub fn draw_static<D: DrawTarget<Color = Rgb565>>(&self, target: &mut D) -> Result<(), D::Error> {
    let border_style = PrimitiveStyleBuilder::new()
      .stroke_width(BORDER_SIZE)
      .stroke_color(BORDER_COLOR)
      .stroke_alignment(embedded_graphics::primitives::StrokeAlignment::Inside)
      .build();

    let cells = &self.cells;

    cells.left_border.draw_styled(&border_style, target)?;
    cells.name.right_border.draw_styled(&border_style, target)?;

    cells
      .voltage
      .right_border
      .draw_styled(&border_style, target)?;

    cells
      .current
      .right_border
      .draw_styled(&border_style, target)?;

    cells
      .power
      .right_border
      .draw_styled(&border_style, target)?;

    // Bottom border

    let br = cells.power.right_border.bottom_right().unwrap();
    Rectangle::with_corners(
      Point::new(self.bounds.top_left.x, br.y) - Point::new(0, BORDER_SIZE as i32),
      br,
    )
    .draw_styled(&border_style, target)?;

    Ok(())
  }

  pub fn update_values(&mut self, voltage: f32, current: f32, power: f32) -> &Self {
    self.voltage = voltage;
    self.current = current;
    self.power = power;

    self
  }
}

/// Implementing `View` is required by the layout and alignment operations
/// `View` teaches `embedded-layout` where our object is, how big it is and how to move it.
impl View for VoltTableRow<'_> {
  #[inline]
  fn translate_impl(&mut self, by: Point) {
    // make sure you don't accidentally call `translate`!
    self.bounds.translate_mut(by);
    self.cells = VoltTableCells::for_bounds(&self.bounds);
  }

  #[inline]
  fn bounds(&self) -> Rectangle {
    self.bounds
  }
}

impl<'a> Drawable for VoltTableRow<'a> {
  type Color = Rgb565;
  type Output = ();

  fn draw<D: DrawTarget<Color = Self::Color>>(&self, target: &mut D) -> Result<(), D::Error> {
    let text_color = if self.current > 0.1 {
      self.active_text_color
    } else {
      self.inactive_text_color
    };

    let text_style = MonoTextStyle::new(TABLE_FONT, text_color);

    let cells = &self.cells;

    cells
      .name
      .draw_text(self.caption, self.background_color, text_style, target)?;

    let volts = float_to_fixed_with_suffix::<6>(self.voltage, b'V');
    let volts = core::str::from_utf8(&volts).unwrap();
    cells
      .voltage
      .draw_text(volts, self.background_color, text_style, target)?;

    let amps = float_to_fixed_with_suffix::<6>(self.current, b'A');
    let amps = core::str::from_utf8(&amps).unwrap();
    cells
      .current
      .draw_text(amps, self.background_color, text_style, target)?;

    let watts = float_to_fixed_with_suffix::<6>(self.power, b'W');
    let watts = core::str::from_utf8(&watts).unwrap();
    cells
      .power
      .draw_text(watts, self.background_color, text_style, target)?;

    Ok(())
  }
}
