use embedded_graphics::{
  draw_target::DrawTarget,
  geometry::Point,
  mono_font::ascii::FONT_9X18_BOLD,
  pixelcolor::{Rgb565, RgbColor, WebColors},
};

use super::Layout;
use crate::textual::format::line;
use crate::textual::TextField;
use crate::{Snapshot, PORT_NAMES};

/// Characters per line: 26 fit the display width
const LINE: usize = 25;
const BACKGROUND: Rgb565 = Rgb565::BLACK;

/// One reading per line of text
pub struct Text {
  temp: TextField<LINE>,
  pack: TextField<LINE>,
  ports: [TextField<LINE>; 3],
  main: TextField<LINE>,
  /// Time since boot: shows the firmware is alive
  uptime: TextField<LINE>,
  cleared: bool,
}

impl Layout for Text {
  fn new() -> Self {
    let row = |row: i32| TextField::new(Point::new(12, 20 + row * 24), &FONT_9X18_BOLD, Rgb565::WHITE, BACKGROUND);
    Self {
      temp: row(0),
      pack: row(1),
      ports: [row(2), row(3), row(4)],
      main: row(5),
      uptime: TextField::new(Point::new(12, 200), &FONT_9X18_BOLD, Rgb565::CSS_GRAY, BACKGROUND),
      cleared: false,
    }
  }

  fn update<D: DrawTarget<Color = Rgb565>>(&mut self, s: &Snapshot, target: &mut D) -> Result<(), D::Error> {
    if !self.cleared {
      target.clear(BACKGROUND)?;
      self.cleared = true;
    }

    self.temp.update(line::<LINE>(format_args!("Temp {:>9.2} C", s.pack_temp)).as_str(), target)?;
    let pack = line::<LINE>(format_args!("Pack{:>6.2}V{:>7.3}A", s.pack_volts, s.pack_amps));
    self.pack.update(pack.as_str(), target)?;

    for ((field, port), name) in self.ports.iter_mut().zip(&s.ports).zip(PORT_NAMES) {
      let text = line::<LINE>(format_args!("{:<4}{:>6.2}V{:>7.3}A{:>5.1}W", name, port.volts, port.amps, port.watts()));
      field.update(text.as_str(), target)?;
    }

    // Battery side: the charger chip's port voltage isn't measured
    let main = line::<LINE>(format_args!("Main{:>14.3}A{:>5.1}W", s.main_amps, s.main_watts()));
    self.main.update(main.as_str(), target)?;

    let t = s.uptime_s;
    let uptime = line::<LINE>(format_args!("Up {}:{:02}:{:02}", t / 3600, t / 60 % 60, t % 60));
    self.uptime.update(uptime.as_str(), target)?;
    Ok(())
  }
}
