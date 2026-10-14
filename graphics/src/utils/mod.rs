use core::{
  slice::{self},
  str,
  str::Chars,
};

use numtoa::NumToA;
use micromath::F32Ext;
use embedded_graphics::pixelcolor::{Rgb565, RgbColor};

pub fn float_to_fixed_with_suffix<const CHARS: usize>(f: f32, suffix: u8) -> [u8; CHARS] {
  let mut binding = ryu::Buffer::new();
  let converted = binding.format_finite(f);
  let chars: Chars<'_>;

  if converted.len() < CHARS - 1 {
    chars = converted.chars();
  } else {
    chars = converted.split_at(CHARS - 1).0.chars();
  }

  let mut res = [suffix; CHARS];

  let mut i = 0;

  for c in chars {
    res[i] = c as u8;

    i = i + 1;
  }

  for j in i..(CHARS - 1) {
    res[j] = b'0';
  }

  return res;
}

pub fn float_to_fixed_with_suffix2<const CHARS: usize>(f: f32, suffix: [u8; 2]) -> [u8; CHARS] {
  let mut binding = ryu::Buffer::new();
  let converted = binding.format_finite(f);
  let chars: Chars<'_>;

  if converted.len() < CHARS - 1 {
    chars = converted.chars();
  } else {
    chars = converted.split_at(CHARS - 1).0.chars();
  }

  let mut res = [0; CHARS];

  let mut i = 0;

  for c in chars {
    res[i] = c as u8;

    i = i + 1;
  }

  for j in i..(CHARS - 2) {
    res[j] = b'0';
  }

  res[CHARS - 2] = suffix[0];
  res[CHARS - 1] = suffix[1];

  return res;
}

pub fn float_to_fixed<const CHARS: usize>(f: f32) -> [u8; CHARS] {
  let mut binding = ryu::Buffer::new();
  let converted = binding.format_finite(f);
  let chars: Chars<'_>;

  if converted.len() < CHARS - 1 {
    chars = converted.chars();
  } else {
    chars = converted.split_at(CHARS - 1).0.chars();
  }

  let mut res = [b'0'; CHARS];

  let mut i = 0;

  for c in chars {
    res[i] = c as u8;

    i = i + 1;
  }

  return res;
}

#[allow(dead_code)]
pub fn float_to_fixed2<'a, const CHARS_PER_CELL: usize>(f: f32, suffix: u8) -> &'a str {
  let mut binding = ryu::Buffer::new();
  let converted = binding.format_finite(f);
  let chars: Chars<'_>;

  if converted.len() < CHARS_PER_CELL - 1 {
    chars = converted.chars();
  } else {
    chars = converted.split_at(CHARS_PER_CELL - 1).0.chars();
  }

  let mut res = [suffix; CHARS_PER_CELL];

  let mut i = 0;

  for c in chars {
    res[i] = c as u8;

    i = i + 1;
  }

  for j in i..(CHARS_PER_CELL - 1) {
    res[j] = b'0';
  }

  unsafe {
    let slice = slice::from_raw_parts(res.as_ptr() as *const u8, CHARS_PER_CELL);

    return str::from_utf8(slice).unwrap();
  }
}

pub fn darken(color: Rgb565, percentage: u32) -> Rgb565 {
  Rgb565::new(
    ((Into::<u32>::into(color.r()) * percentage / 100) & 0xFF)
      .try_into()
      .unwrap(),
    ((Into::<u32>::into(color.g()) * percentage / 100) & 0xFF)
      .try_into()
      .unwrap(),
    ((Into::<u32>::into(color.b()) * percentage / 100) & 0xFF)
      .try_into()
      .unwrap(),
  )
}

pub fn to_integer_percents<'a>(v: f32)  -> [u8; 4] {
  let mut buf = [b'0', b'0', b'0', b'%'];
  let mut p_buf = buf.split_at_mut(3).0;

  let mut percents = v.round();

  if percents > 255.0 {
    percents = 255.0
  }

  let percents = percents as u8;

  percents.numtoa(10, &mut p_buf);

  match percents {
    0..=9 => { p_buf[1] = b' '; p_buf[0] = b' '; }
    10..=99 => { p_buf[0] = b' '; }
    _ => {}
  }

  buf
}