use embedded_graphics_simulator::{OutputSettingsBuilder, SimulatorDisplay, Window};

use embedded_graphics::{pixelcolor::Rgb565, prelude::*};

use embedded_layout::align::{horizontal, vertical, Align};
use embedded_layout::View;

use graphics::batteries::Batteries;
use graphics::volttable::VoltTable;
use graphics::textual::Charge;

fn main() -> Result<(), core::convert::Infallible> {
  let mut display: SimulatorDisplay<Rgb565> = SimulatorDisplay::new(Size::new(240, 240));
  let output_settings = OutputSettingsBuilder::new().build();

  let mut table = VoltTable::new(Point::zero(), 240).align_to(
    &display.bounding_box(),
    horizontal::Left,
    vertical::Bottom,
  );

  table.set_dc_values(16.4, 2.1, 16.4  * 2.1);

  table.draw_static(&mut display)?;
  table.draw(&mut display)?;

  let mut batteries = Batteries::new(Point::zero(), 240)
    .align_to(&table, horizontal::Center, vertical::BottomToTop)
    .translate(Point::new(0, -5));

  batteries
    .set_voltage1(3.48)
    .draw_static(&mut display)?.draw(&mut display)?;

  let mut charge = Charge::new(Point::zero())
    .align_to(&batteries, horizontal::Center, vertical::BottomToTop)
    .translate(Point::new(0, -5));

  charge
    .set_voltage(13.37)
    .set_percents(89.0)
    .set_power(9.35)
    .draw_static(&mut display)?
    .draw(&mut display)?;

  Window::new("Powerbank display demo", &output_settings).show_static(&display);
  Ok(())
}
