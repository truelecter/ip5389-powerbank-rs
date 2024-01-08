use embedded_graphics_simulator::{OutputSettingsBuilder, SimulatorDisplay, Window};

use embedded_graphics::{pixelcolor::Rgb565, prelude::*};

use embedded_layout::View;
use embedded_layout::align::{Align, horizontal, vertical};
use graphics::batteries::Batteries;
use graphics::volttable::VoltTable;

/*
+------+---------+---------+-------+
| Port | Voltage | Current | Power |
+------+---------+---------+-------+
| USB  |
+------+---------+---------+-------+
| EXT1 |
+------+---------+---------+-------+
| EXT2 |
+------+---------+---------+-------+
| DC   |
+------+---------+---------+-------+
*/

fn main() -> Result<(), core::convert::Infallible> {
  let mut display: SimulatorDisplay<Rgb565> = SimulatorDisplay::new(Size::new(240, 240));
  let output_settings = OutputSettingsBuilder::new().build();

  let table = VoltTable::new(Point::zero(), 240)
    .align_to(&display.bounding_box(), horizontal::Left, vertical::Bottom);

  table.draw_static(&mut display)?;
  table.draw(&mut display)?;

  Batteries::new(Point::zero(), 240)
    .align_to(&table, horizontal::Center, vertical::BottomToTop)
    .translate(Point::new(0, -5))
    .set_voltage1(3.48)
    .draw_static(&mut display)?
    .draw(&mut display)?;

  Window::new("Powerbank display demo", &output_settings).show_static(&display);
  Ok(())
}
