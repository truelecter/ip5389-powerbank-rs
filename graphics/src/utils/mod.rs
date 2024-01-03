use core::{str::Chars, slice::{self}, str};

pub fn float_to_fixed<const CHARS_PER_CELL: usize>(f: f32, suffix: u8) -> [u8; CHARS_PER_CELL] {
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