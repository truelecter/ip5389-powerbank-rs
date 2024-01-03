use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};

use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
};

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
    let output_settings = OutputSettingsBuilder::new()
        .build();

    let table = VoltTable::new(Point::zero(), 240);

    table.draw_initial(&mut display)?;
    table.draw(&mut display)?;

    Window::new("Hello, element spacing!", &output_settings).show_static(&display);
    Ok(())
}