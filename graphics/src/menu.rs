use embedded_graphics::{
  draw_target::DrawTarget,
  geometry::{Point, Size},
  mono_font::{ascii::FONT_8X13, MonoFont},
  pixelcolor::{Rgb565, RgbColor},
  primitives::Rectangle,
};

use crate::textual::{Align, Cell};

const FONT: &MonoFont<'static> = &FONT_8X13;
/// Characters per row: the display width less a margin
pub const COLUMNS: usize = 29;
/// Rows that fit the display height
pub const ROWS: usize = 15;
const ROW_HEIGHT: u32 = 16;
const BACKGROUND: Rgb565 = Rgb565::BLACK;

/// Full-screen list of text rows: a title, then lines of which one may be selected. Rows
/// redraw only when their text or selection changes.
pub struct Menu {
  rows: [Cell<COLUMNS>; ROWS],
  cleared: bool,
}

impl Menu {
  pub fn new() -> Self {
    let mut i = 0;
    let rows = [(); ROWS].map(|()| {
      let area = Rectangle::new(Point::new(4, (i * ROW_HEIGHT) as i32), Size::new(236, ROW_HEIGHT));
      i += 1;
      Cell::new(area, FONT, Align::Left, Rgb565::WHITE, BACKGROUND)
    });
    Self { rows, cleared: false }
  }

  /// Shows `title` and `lines`, with `selected` (an index into `lines`) drawn inverted.
  /// Rows past the lines are blank.
  pub fn update<D: DrawTarget<Color = Rgb565>>(
    &mut self,
    title: &str,
    lines: &[&str],
    selected: Option<usize>,
    target: &mut D,
  ) -> Result<(), D::Error> {
    if !self.cleared {
      target.clear(BACKGROUND)?;
      self.cleared = true;
    }
    let (title_row, rows) = self.rows.split_at_mut(1);
    title_row[0].set_color(Rgb565::YELLOW);
    title_row[0].update(title, target)?;
    for (i, row) in rows.iter_mut().enumerate() {
      let chosen = selected == Some(i);
      row.set_color(if chosen { Rgb565::BLACK } else { Rgb565::WHITE });
      row.set_background(if chosen { Rgb565::WHITE } else { BACKGROUND });
      row.update(lines.get(i).copied().unwrap_or(""), target)?;
    }
    Ok(())
  }
}

impl Default for Menu {
  fn default() -> Self {
    Self::new()
  }
}
