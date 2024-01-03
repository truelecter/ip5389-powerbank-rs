use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};

use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*, primitives::{Rectangle, StyledDrawable, PrimitiveStyleBuilder},
};

use embedded_layout::prelude::*;

use graphics::volttable::VoltTableEntry;

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
        // .theme()
        .build();

    // Create styles used by the drawing operations.
    // let thin_stroke = PrimitiveStyle::with_stroke(Rgb565::BLUE, 1);
    // let thick_stroke = PrimitiveStyle::with_stroke(Rgb565::YELLOW, 3);
    // let fill = PrimitiveStyle::with_fill(Rgb565::RED);
    // let text_style = MonoTextStyle::new(&FONT_10X20, Rgb565::CSS_MINT_CREAM);

    let mut r1 = VoltTableEntry::new(
        Point::new(0, 0),
        240,
        0.1, 0.2, 0.3,
        Rgb565::CYAN,
        Rgb565::CYAN,
        "EXT1"
    );

    r1.draw_initial(&mut display)?;
    r1.update_values(0.2, 0.3, 0.4);
    r1.draw(&mut display)?;

    let r2 = VoltTableEntry::new(
        Point::new(0, 0),
        240,
        0.1, 0.2, -0.3,
        Rgb565::CYAN,
        Rgb565::CYAN,
        "EXT2"
    )
        .align_to(&r1, horizontal::Center, vertical::TopToBottom);

    r2.draw_initial(&mut display)?;
    r2.draw(&mut display)?;

    Rectangle::new(Point::zero(), Size::new(236, 10))
        .align_to(&r2, horizontal::NoAlignment, vertical::TopToBottom)
        .draw_styled(
            &PrimitiveStyleBuilder::new().fill_color(Rgb565::BLUE).build(),
            &mut display
        )?;

    Window::new("Hello, element spacing!", &output_settings).show_static(&display);
    Ok(())
}