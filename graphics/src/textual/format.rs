use core::fmt::{self, Write};

/// Text of up to `N` bytes formatted without allocation; anything longer is cut
#[derive(Clone, Copy)]
pub struct Line<const N: usize> {
  bytes: [u8; N],
  len: usize,
}

impl<const N: usize> Line<N> {
  pub fn new() -> Self {
    Self { bytes: [0; N], len: 0 }
  }

  pub fn as_str(&self) -> &str {
    // `write_str` only cuts at character boundaries
    core::str::from_utf8(&self.bytes[..self.len]).unwrap_or("")
  }
}

impl<const N: usize> Default for Line<N> {
  fn default() -> Self {
    Self::new()
  }
}

impl<const N: usize> Write for Line<N> {
  fn write_str(&mut self, s: &str) -> fmt::Result {
    let mut end = s.len().min(N - self.len);
    while !s.is_char_boundary(end) {
      end -= 1;
    }
    self.bytes[self.len..self.len + end].copy_from_slice(&s.as_bytes()[..end]);
    self.len += end;
    if end < s.len() {
      Err(fmt::Error)
    } else {
      Ok(())
    }
  }
}

/// Formats into a [`Line`], cutting what doesn't fit
pub fn line<const N: usize>(args: fmt::Arguments) -> Line<N> {
  let mut line = Line::new();
  line.write_fmt(args).ok();
  line
}

/// `value` followed by `unit` in at most `N` characters, with as many decimals as fit, up to
/// `max_decimals`: `13.947V`, `-6.540A`, `-89.20W` in 7. A value that rounds to zero shows
/// without a minus sign. A value too big for `N` even without decimals is cut.
pub fn fixed<const N: usize>(value: f32, max_decimals: usize, unit: &str) -> Line<N> {
  let mut decimals = max_decimals;
  loop {
    let scale = (0..decimals).fold(1.0f32, |scale, _| scale * 10.0);
    let rounds_to_zero = value * scale < 0.5 && value * scale > -0.5;
    let value = if rounds_to_zero { 0.0 } else { value };

    let mut text = Line::<32>::new();
    let fitted = write!(text, "{value:.decimals$}{unit}").is_ok() && text.len <= N;
    if fitted || decimals == 0 {
      return line(format_args!("{}", text.as_str()));
    }
    decimals -= 1;
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn decimals_shrink_to_fit() {
    assert_eq!(fixed::<7>(13.947, 3, "V").as_str(), "13.947V");
    assert_eq!(fixed::<7>(-6.54, 3, "A").as_str(), "-6.540A");
    assert_eq!(fixed::<7>(-89.2, 3, "W").as_str(), "-89.20W");
    assert_eq!(fixed::<7>(123.4, 3, "W").as_str(), "123.40W");
    assert_eq!(fixed::<5>(4.181, 2, "").as_str(), "4.18");
  }

  #[test]
  fn rounding_up_a_digit_drops_a_decimal() {
    assert_eq!(fixed::<6>(9.9996, 3, "V").as_str(), "10.00V");
  }

  #[test]
  fn tiny_negative_values_show_as_zero() {
    assert_eq!(fixed::<7>(-0.0004, 3, "A").as_str(), "0.000A");
    assert_eq!(fixed::<7>(-0.0006, 3, "A").as_str(), "-0.001A");
  }

  #[test]
  fn too_big_values_are_cut() {
    assert_eq!(fixed::<4>(123456.0, 2, "W").as_str(), "1234");
  }
}
