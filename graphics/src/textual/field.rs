use embedded_graphics::{
  draw_target::DrawTarget,
  geometry::Point,
  mono_font::{MonoFont, MonoTextStyle, MonoTextStyleBuilder},
  pixelcolor::Rgb565,
  text::{Baseline, Text},
  Drawable,
};

/// Fixed-width line of monospace text that redraws only the characters that changed.
///
/// Every character is drawn with the background color, so a new character replaces the old
/// one in a single pass: no erase step, no flicker. Text shorter than `N` is padded with
/// spaces, so leftovers of a longer previous text get cleared.
pub struct TextField<const N: usize> {
  top_left: Point,
  style: MonoTextStyle<'static, Rgb565>,
  advance: i32,
  /// What is on the screen; `None` until the first draw
  shown: Option<[u8; N]>,
}

impl<const N: usize> TextField<N> {
  pub fn new(
    top_left: Point,
    font: &'static MonoFont<'static>,
    text_color: Rgb565,
    background_color: Rgb565,
  ) -> Self {
    Self {
      top_left,
      style: MonoTextStyleBuilder::new()
        .font(font)
        .text_color(text_color)
        .background_color(background_color)
        .build(),
      advance: (font.character_size.width + font.character_spacing) as i32,
      shown: None,
    }
  }

  /// Draws the characters of `text` that differ from the screen and returns how many it drew.
  /// `text` is cut or space-padded to `N` characters; non-ASCII characters become `?`.
  pub fn update<D: DrawTarget<Color = Rgb565>>(
    &mut self,
    text: &str,
    target: &mut D,
  ) -> Result<usize, D::Error> {
    let mut next = [b' '; N];
    for (slot, c) in next.iter_mut().zip(text.chars()) {
      *slot = if c.is_ascii() { c as u8 } else { b'?' };
    }

    let mut drawn = 0;
    let mut i = 0;
    while i < N {
      if self.shown.is_some_and(|shown| shown[i] == next[i]) {
        i += 1;
        continue;
      }
      // Draw each run of changed characters with one `Text`
      let start = i;
      while i < N && self.shown.is_none_or(|shown| shown[i] != next[i]) {
        i += 1;
      }
      // Only ASCII bytes are stored, so any slice is valid UTF-8
      let run = core::str::from_utf8(&next[start..i]).unwrap_or("");
      let position = self.top_left + Point::new(start as i32 * self.advance, 0);
      Text::with_baseline(run, position, self.style, Baseline::Top).draw(target)?;
      drawn += i - start;
    }

    self.shown = Some(next);
    Ok(drawn)
  }

  /// Makes the next `update` draw every character, for when something else drew over the field
  pub fn invalidate(&mut self) {
    self.shown = None;
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use embedded_graphics::{
    geometry::{OriginDimensions, Size},
    mono_font::ascii::FONT_6X10,
    pixelcolor::RgbColor,
    Pixel,
  };

  /// Records the screen and counts the pixels written
  struct Screen {
    pixels: [[Rgb565; 64]; 10],
    written: usize,
  }

  impl Screen {
    fn new() -> Self {
      Self { pixels: [[Rgb565::RED; 64]; 10], written: 0 }
    }

    /// Characters on the screen, decoded by comparing each cell with a reference rendering
    fn text(&self) -> [u8; 8] {
      let mut out = [0; 8];
      for (cell, slot) in out.iter_mut().enumerate() {
        *slot = (b' '..=b'~')
          .find(|&c| {
            let mut reference = Screen::new();
            let s = [c];
            Text::with_baseline(core::str::from_utf8(&s).unwrap(), Point::zero(), style(), Baseline::Top)
              .draw(&mut reference)
              .unwrap();
            (0..10).all(|y| (0..6).all(|x| reference.pixels[y][x] == self.pixels[y][cell * 6 + x]))
          })
          .unwrap_or(0);
      }
      out
    }
  }

  impl OriginDimensions for Screen {
    fn size(&self) -> Size {
      Size::new(64, 10)
    }
  }

  impl DrawTarget for Screen {
    type Color = Rgb565;
    type Error = core::convert::Infallible;

    fn draw_iter<I: IntoIterator<Item = Pixel<Rgb565>>>(&mut self, pixels: I) -> Result<(), Self::Error> {
      for Pixel(p, color) in pixels {
        self.pixels[p.y as usize][p.x as usize] = color;
        self.written += 1;
      }
      Ok(())
    }
  }

  fn style() -> MonoTextStyle<'static, Rgb565> {
    MonoTextStyleBuilder::new()
      .font(&FONT_6X10)
      .text_color(Rgb565::WHITE)
      .background_color(Rgb565::BLACK)
      .build()
  }

  fn field() -> TextField<8> {
    TextField::new(Point::zero(), &FONT_6X10, Rgb565::WHITE, Rgb565::BLACK)
  }

  const CELL: usize = 6 * 10;

  #[test]
  fn first_update_draws_every_cell() {
    let (mut screen, mut field) = (Screen::new(), field());
    assert_eq!(field.update("12.5V", &mut screen), Ok(8));
    assert_eq!(screen.written, 8 * CELL);
    assert_eq!(&screen.text(), b"12.5V   ");
  }

  #[test]
  fn only_changed_characters_are_redrawn() {
    let (mut screen, mut field) = (Screen::new(), field());
    field.update("12.5V", &mut screen).unwrap();
    screen.written = 0;

    assert_eq!(field.update("12.5V", &mut screen), Ok(0));
    assert_eq!(field.update("13.7V", &mut screen), Ok(2));
    assert_eq!(screen.written, 2 * CELL);
    assert_eq!(&screen.text(), b"13.7V   ");
  }

  #[test]
  fn shorter_text_clears_the_old_tail() {
    let (mut screen, mut field) = (Screen::new(), field());
    field.update("100.00V", &mut screen).unwrap();
    assert_eq!(field.update("9.0V", &mut screen), Ok(6));
    assert_eq!(&screen.text(), b"9.0V    ");
  }

  #[test]
  fn longer_text_is_cut_and_non_ascii_replaced() {
    let (mut screen, mut field) = (Screen::new(), field());
    field.update("25°C and more", &mut screen).unwrap();
    assert_eq!(&screen.text(), b"25?C and");
  }

  #[test]
  fn invalidate_redraws_everything() {
    let (mut screen, mut field) = (Screen::new(), field());
    field.update("12.5V", &mut screen).unwrap();
    field.invalidate();
    assert_eq!(field.update("12.5V", &mut screen), Ok(8));
  }
}
