use embedded_graphics::{
  draw_target::DrawTarget,
  geometry::{Point, Size},
  mono_font::{MonoFont, MonoTextStyleBuilder},
  pixelcolor::Rgb565,
  primitives::Rectangle,
  text::{Baseline, Text},
  Drawable,
};

/// Where the text sits in its cell, horizontally. Vertically it is always centered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
  Left,
  Center,
  Right,
}

/// Up to `N` characters of monospace text in a fixed area of the screen, redrawn only when the
/// text or its color changes.
///
/// With a highlight color, a cell whose text changed is drawn on that color, and the next
/// update with the same text puts it back on the normal background: a changed value stays lit
/// for one update.
pub struct Cell<const N: usize> {
  area: Rectangle,
  font: &'static MonoFont<'static>,
  align: Align,
  color: Rgb565,
  background: Rgb565,
  highlight: Option<Rgb565>,
  /// What is on the screen, padded to `N`; `None` until the first draw
  shown: Option<[u8; N]>,
  /// Drawn on the highlight color
  lit: bool,
  /// The color changed since the last draw
  stale: bool,
}

impl<const N: usize> Cell<N> {
  /// A cell that fills `area` with `background`. The area must fit `N` characters of `font`.
  pub fn new(area: Rectangle, font: &'static MonoFont<'static>, align: Align, color: Rgb565, background: Rgb565) -> Self {
    Self { area, font, align, color, background, highlight: None, shown: None, lit: false, stale: false }
  }

  /// Lights up the cell on `color` for one update after its text changes
  pub fn with_highlight(mut self, color: Rgb565) -> Self {
    self.highlight = Some(color);
    self
  }

  /// A cell `N` characters wide and one line high, with its top left corner at `top_left`
  pub fn fitted(top_left: Point, font: &'static MonoFont<'static>, align: Align, color: Rgb565, background: Rgb565) -> Self {
    let size = Size::new(N as u32 * advance(font), font.character_size.height);
    Self::new(Rectangle::new(top_left, size), font, align, color, background)
  }

  pub fn area(&self) -> Rectangle {
    self.area
  }

  /// Changes the text color; the next update redraws the cell
  pub fn set_color(&mut self, color: Rgb565) {
    if color != self.color {
      self.color = color;
      self.stale = true;
    }
  }

  /// Changes the background; the next update redraws the cell
  pub fn set_background(&mut self, background: Rgb565) {
    if background != self.background {
      self.background = background;
      self.stale = true;
    }
  }

  /// Shows `text`, cut or padded to `N` characters; non-ASCII characters become `?`.
  /// Returns whether the cell was drawn.
  pub fn update<D: DrawTarget<Color = Rgb565>>(&mut self, text: &str, target: &mut D) -> Result<bool, D::Error> {
    let next = self.pad(text);
    let changed = self.shown.is_some_and(|shown| shown != next);
    if !(changed || self.shown.is_none() || self.lit || self.stale) {
      return Ok(false);
    }

    let lit = changed && self.highlight.is_some();
    let background = if lit { self.highlight.unwrap_or(self.background) } else { self.background };
    self.draw(&next, background, target)?;
    self.shown = Some(next);
    self.lit = lit;
    self.stale = false;
    Ok(true)
  }

  /// Makes the next update draw the cell, for when something else drew over it
  pub fn invalidate(&mut self) {
    self.shown = None;
    self.lit = false;
  }

  fn pad(&self, text: &str) -> [u8; N] {
    let len = text.chars().count().min(N);
    let skip = match self.align {
      Align::Left => 0,
      Align::Center => (N - len) / 2,
      Align::Right => N - len,
    };
    let mut padded = [b' '; N];
    for (slot, c) in padded[skip..].iter_mut().zip(text.chars()) {
      *slot = if c.is_ascii() { c as u8 } else { b'?' };
    }
    padded
  }

  /// Draws the text block with its background, then the rest of the area around it
  fn draw<D: DrawTarget<Color = Rgb565>>(&self, text: &[u8; N], background: Rgb565, target: &mut D) -> Result<(), D::Error> {
    let text_size = Size::new(N as u32 * advance(self.font), self.font.character_size.height);
    let free = self.area.size.saturating_sub(text_size);
    let offset = match self.align {
      Align::Left => 0,
      Align::Center => free.width / 2,
      Align::Right => free.width,
    };
    let block = Rectangle::new(self.area.top_left + Point::new(offset as i32, (free.height / 2) as i32), text_size);

    let style = MonoTextStyleBuilder::new()
      .font(self.font)
      .text_color(self.color)
      .background_color(background)
      .build();
    // Only ASCII bytes are stored, so the text is valid UTF-8
    let text = core::str::from_utf8(text).unwrap_or("");
    Text::with_baseline(text, block.top_left, style, Baseline::Top).draw(target)?;

    // Strips above, below, left and right of the text block, as [start, end) corners
    let (start, end) = (self.area.top_left, self.area.top_left + self.area.size);
    let (inner, inner_end) = (block.top_left, block.top_left + block.size);
    let strips = [
      (start, Point::new(end.x, inner.y)),
      (Point::new(start.x, inner_end.y), end),
      (Point::new(start.x, inner.y), Point::new(inner.x, inner_end.y)),
      (Point::new(inner_end.x, inner.y), Point::new(end.x, inner_end.y)),
    ];
    for (from, to) in strips {
      if to.x > from.x && to.y > from.y {
        let size = Size::new((to.x - from.x) as u32, (to.y - from.y) as u32);
        target.fill_solid(&Rectangle::new(from, size), background)?;
      }
    }
    Ok(())
  }
}

fn advance(font: &MonoFont) -> u32 {
  font.character_size.width + font.character_spacing
}

#[cfg(test)]
mod tests {
  use super::*;
  use embedded_graphics::{geometry::OriginDimensions, mono_font::ascii::FONT_6X10, pixelcolor::RgbColor, Pixel};

  const HIGHLIGHT: Rgb565 = Rgb565::BLUE;

  /// Records the screen and counts the pixels written
  struct Screen {
    pixels: [[Rgb565; 64]; 16],
    written: usize,
  }

  impl Screen {
    fn new() -> Self {
      Self { pixels: [[Rgb565::RED; 64]; 16], written: 0 }
    }

    fn count(&self, color: Rgb565) -> usize {
      self.pixels.iter().flatten().filter(|&&p| p == color).count()
    }
  }

  impl OriginDimensions for Screen {
    fn size(&self) -> Size {
      Size::new(64, 16)
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

  /// 4 characters of 6x10 in a 30x14 area at (2, 1)
  fn cell(align: Align) -> Cell<4> {
    let area = Rectangle::new(Point::new(2, 1), Size::new(30, 14));
    Cell::new(area, &FONT_6X10, align, Rgb565::WHITE, Rgb565::BLACK).with_highlight(HIGHLIGHT)
  }

  const AREA: usize = 30 * 14;

  #[test]
  fn first_draw_fills_the_area_without_highlight() {
    let (mut screen, mut cell) = (Screen::new(), cell(Align::Center));
    assert_eq!(cell.update("1.5", &mut screen), Ok(true));
    assert_eq!(screen.written, AREA);
    assert_eq!(screen.count(HIGHLIGHT), 0);
    assert_eq!(screen.count(Rgb565::RED), 64 * 16 - AREA);
  }

  #[test]
  fn changed_text_stays_lit_for_one_update() {
    let (mut screen, mut cell) = (Screen::new(), cell(Align::Right));
    cell.update("1.5", &mut screen).unwrap();

    assert_eq!(cell.update("1.6", &mut screen), Ok(true));
    let background = AREA - screen.count(Rgb565::WHITE);
    assert_eq!(screen.count(HIGHLIGHT), background);

    assert_eq!(cell.update("1.6", &mut screen), Ok(true));
    assert_eq!(screen.count(HIGHLIGHT), 0);

    screen.written = 0;
    assert_eq!(cell.update("1.6", &mut screen), Ok(false));
    assert_eq!(screen.written, 0);
  }

  #[test]
  fn a_value_changing_every_update_stays_lit() {
    let (mut screen, mut cell) = (Screen::new(), cell(Align::Left));
    cell.update("1", &mut screen).unwrap();
    for text in ["2", "3", "4"] {
      cell.update(text, &mut screen).unwrap();
      assert!(screen.count(HIGHLIGHT) > 0);
    }
  }

  #[test]
  fn color_change_redraws_without_highlight() {
    let (mut screen, mut cell) = (Screen::new(), cell(Align::Center));
    cell.update("1.5", &mut screen).unwrap();
    cell.set_color(Rgb565::GREEN);
    assert_eq!(cell.update("1.5", &mut screen), Ok(true));
    assert_eq!(screen.count(HIGHLIGHT), 0);
    assert!(screen.count(Rgb565::GREEN) > 0);
    assert_eq!(screen.count(Rgb565::WHITE), 0);
  }

  #[test]
  fn alignment_moves_the_text() {
    let left_edge = |align| {
      let (mut screen, mut cell) = (Screen::new(), cell(align));
      cell.update("1", &mut screen).unwrap();
      (0..64).find(|&x| (0..16).any(|y| screen.pixels[y][x] == Rgb565::WHITE)).unwrap()
    };
    let (left, center, right) = (left_edge(Align::Left), left_edge(Align::Center), left_edge(Align::Right));
    assert!(left < center && center < right, "{left} {center} {right}");
    // Right aligned: the digit is in the last of the four characters, which end 3 px before the area
    assert!(right >= 2 + 3 + 3 * 6);
  }
}
