use embedded_graphics::{
  draw_target::DrawTarget,
  geometry::Point,
  pixelcolor::{Rgb565, RgbColor},
};

use super::Layout;
use crate::batteries::Batteries;
use crate::textual::Charge;
use crate::volttable::VoltTable;
use crate::Snapshot;

const WIDTH: u32 = 240;
const HEIGHT: u32 = 240;
const BACKGROUND: Rgb565 = Rgb565::BLACK;
/// Background of a value that changed since the last update
const HIGHLIGHT: Rgb565 = Rgb565::new(8, 16, 8);
/// Space between the blocks
const GAP: u32 = 5;

/// The first design: pack voltage, charge and energy above the four cells drawn as batteries,
/// above a table of the ports, all at the bottom of the screen
pub struct Classic {
  charge: Charge,
  batteries: Batteries,
  table: VoltTable,
  drawn_static: bool,
}

impl Layout for Classic {
  fn new() -> Self {
    let table_top = HEIGHT - VoltTable::height();
    let batteries_top = table_top - GAP - Batteries::height();
    let charge_top = batteries_top - GAP - Charge::height();
    let at = |y: u32| Point::new(0, y as i32);
    Self {
      charge: Charge::new(at(charge_top), WIDTH, BACKGROUND, HIGHLIGHT),
      batteries: Batteries::new(at(batteries_top), WIDTH, BACKGROUND, HIGHLIGHT),
      table: VoltTable::new(at(table_top), WIDTH, BACKGROUND, HIGHLIGHT),
      drawn_static: false,
    }
  }

  fn update<D: DrawTarget<Color = Rgb565>>(&mut self, s: &Snapshot, target: &mut D) -> Result<(), D::Error> {
    if !self.drawn_static {
      target.clear(BACKGROUND)?;
      self.batteries.draw_static(target)?;
      self.table.draw_static(target)?;
      self.drawn_static = true;
    }

    self.charge.update(s.pack_volts, s.charge, s.remaining_wh, target)?;
    self.batteries.update(&s.cell_volts, target)?;
    let port = |i: usize| (Some(s.ports[i].volts), s.ports[i].amps, s.ports[i].watts());
    // The main ports' own voltage isn't measured
    let main = (None, s.main_amps, s.main_watts());
    self.table.update([port(0), port(1), port(2), main], target)
  }
}
