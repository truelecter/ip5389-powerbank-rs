#![no_std]
#![no_main]

use core::{cell::RefCell, ptr::null};

use arrform::{arrform, ArrForm};
use cortex_m::interrupt::Mutex;
use display_interface_spi::SPIInterface;
use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, Primitive, PrimitiveStyle, Triangle, Rectangle, StyledDrawable, Styled, PrimitiveStyleBuilder}, mono_font::{ascii::FONT_10X20, MonoTextStyle}, text::Text,
};

use mipidsi::Builder;

use ina3221::INA3221;
// use mipidsi::Builder;
use st7789v::{self, ColorFormat};
use panic_rtt_target as _;
use rtt_target::{rprintln, rtt_init_print};

use embedded_hal::spi::{MODE_3, MODE_2};

use stm32f1xx_hal::{
    pac::{Peripherals, TIM2},
    prelude::*,
    spi::{Spi, NoMiso}, i2c::{BlockingI2c, Mode, DutyCycle, I2c}, timer::CounterMs,
};

// struct MyNoPin;
// use core::convert::Infallible;
// impl embedded_hal::digital::v2::OutputPin for MyNoPin {
//     type Error = Infallible;

//     fn set_high(&mut self) -> Result<(), Self::Error> {
//         Ok(())
//     }

//     fn set_low(&mut self) -> Result<(), Self::Error> {
//         Ok(())
//     }
// }

mod bq4050;

static G_TIM: Mutex<RefCell<Option<CounterMs<TIM2>>>> = Mutex::new(RefCell::new(None));

#[cortex_m_rt::entry]
fn main() -> ! {
    rtt_init_print!();
    rprintln!("Hello, world!");

    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = Peripherals::take().unwrap();

    let mut flash = dp.FLASH.constrain();
    let rcc = dp.RCC.constrain();

    // let clocks = rcc.cfgr
    //     .hclk(48.MHz())
    //     .sysclk(240.MHz())
    //     .pclk2(120.MHz())
    //     .pclk1(120.MHz())
    //     .freeze(&mut flash.acr);

    let clocks = rcc.cfgr
        .hclk(48.MHz())
        .sysclk(72.MHz())
        .pclk1(36.MHz())
        .pclk2(72.MHz())
        .freeze(&mut flash.acr);

    let mut delay = dp.TIM2.delay_us(&clocks);
    let mut delay2 = cp.SYST.delay(&clocks);

    let mut afio = dp.AFIO.constrain();
    let mut gpioa = dp.GPIOA.split();

    // SPI1
    let sck = gpioa.pa5.into_alternate_push_pull(&mut gpioa.crl);
    // let miso = gpioa.pa6;
    let mosi = gpioa.pa7.into_alternate_push_pull(&mut gpioa.crl);
    let cs = gpioa.pa3.into_push_pull_output(&mut gpioa.crl);

    let spi = Spi::spi1(
        dp.SPI1,
        (sck, NoMiso, mosi),
        &mut afio.mapr,
        MODE_3,
        4.MHz(),
        clocks,
    );

    let rst = gpioa.pa1.into_push_pull_output_with_state(&mut gpioa.crl, stm32f1xx_hal::gpio::PinState::High);
    let dc = gpioa.pa2.into_push_pull_output_with_state(&mut gpioa.crl, stm32f1xx_hal::gpio::PinState::High);

    let di = SPIInterface::new(spi, dc, cs);

    let mut display = Builder::st7789(di)
        // width and height are switched on purpose because of the orientation
        .with_display_size(240, 240)
        // this orientation applies for the Display HAT Mini by Pimoroni
        .with_orientation(mipidsi::Orientation::Landscape(true))
        // .with_invert_colors(mipidsi::ColorInversion::Inverted)
        .init(&mut delay, Some(rst))
        .unwrap();

    let colors = [Rgb565::RED, Rgb565::GREEN, Rgb565::BLUE];

    // Clear the display initially
    display.clear(colors[0]).unwrap();

    // let mut d2 = st7789v::ST7789V::with_cs(
    //     spi, cs, dc, rst,
    // ).unwrap();

    // d2.init(&mut delay).unwrap();
    // d2.color_mode(ColorFormat::RGB262K_CI18Bit, &mut delay).unwrap();

    rprintln!("Display Init");

    // INA PB6 PB7
    let mut gpiob = dp.GPIOB.split();

    let pb6 = gpiob.pb6.into_alternate_open_drain(&mut gpiob.crl);
    let pb7 = gpiob.pb7.into_alternate_open_drain(&mut gpiob.crl);

    let i2c = BlockingI2c::i2c1(
        dp.I2C1,
        (pb6, pb7),
        &mut afio.mapr,
        Mode::Fast {
            // frequency: 400.kHz(),
            // duty_cycle: DutyCycle::Ratio16to9,
            frequency: 100.kHz(),
            duty_cycle: DutyCycle::Ratio2to1,
        },
        clocks,
        20,
        2,
        20,
        20,
    );

    // BQ4050 PB10, PB11
    // let pb8 = gpiob.pb8.into_alternate_open_drain(&mut gpiob.crh);
    // let pb9 = gpiob.pb9.into_alternate_open_drain(&mut gpiob.crh);
    let pb10 = gpiob.pb10.into_alternate_open_drain(&mut gpiob.crh);
    let pb11 = gpiob.pb11.into_alternate_open_drain(&mut gpiob.crh);

    let i2c2 = BlockingI2c::i2c2(
        dp.I2C2,
        (pb10, pb11),
        // &mut afio.mapr,
        Mode::Standard {
            frequency: 100.kHz(),
        },
        clocks,
        20,
        1,
        20,
        20,
    );

    // delay.delay_ms(10000u32);

    // for addr in 0x1..0x7F {
    // let addr = 0x0b;
    // rprintln!("trying {:X}", addr);

    // match i2c2.write(addr, &[0;0]) {
    //     Ok(_) => rprintln!("!!!!!!!!PRESENT {:X}!!!!!!!!!!!!!", addr),
    //     Err(_) => {},
    // };
    // }

    let mut bq4050 = bq4050::BQ4050::new(i2c2);
    let mut ina = INA3221::new(i2c, ina3221::AddressPin::Gnd);

    ina.reset().unwrap();
    ina.enable_channel(ina3221::Channel::Ch1, true).unwrap();
    ina.enable_channel(ina3221::Channel::Ch2, true).unwrap();
    ina.enable_channel(ina3221::Channel::Ch3, true).unwrap();

    let text_style = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);

    let mut toClear: Rectangle = Default::default();
    let clear_style = PrimitiveStyleBuilder::new()
        .stroke_color(Rgb565::RED)
        .stroke_width(3)
        .fill_color(Rgb565::GREEN)
        .build();

    loop {
        rprintln!("Namalyovano");

        let volt1 = ina.shunt_voltage(ina3221::Channel::Ch1).unwrap();
        let volt2 = ina.shunt_voltage(ina3221::Channel::Ch2).unwrap();
        let volt3 = ina.shunt_voltage(ina3221::Channel::Ch3).unwrap();

        let bus_voltage1 = ina.bus_voltage(ina3221::Channel::Ch1).unwrap();
        let bus_voltage2 = ina.bus_voltage(ina3221::Channel::Ch2).unwrap();
        let bus_voltage3 = ina.bus_voltage(ina3221::Channel::Ch3).unwrap();

        rprintln!(
            "Bus1: {:.2}\n Volt1: {:.2}",
            bus_voltage1, volt1
        );

        rprintln!(
            "Bus2: {:.2}\n Volt2: {:.2}",
            bus_voltage2, volt2
        );

        rprintln!(
            "Bus3: {:.2}\n Volt3: {:.2}",
            bus_voltage3, volt3
        );

        match bq4050.get_temperature() {
            Ok(sn) => {
                rprintln!("BQ Temp: {:.2}", sn);

                toClear.draw_styled(&clear_style, &mut display).unwrap();

                let text = arrform!(64, "Temp: {:.2}", sn);
                let right = Text::new(&text.as_str(), Point::new(50, 50), text_style);

                toClear = right.bounding_box();

                right.draw(&mut display).unwrap();
            },
            Err(e) => rprintln!("{:#?}", e),
        };

        // delay.delay_ms(2000u32);
    }
}



fn draw_smiley<T: DrawTarget<Color = Rgb565>>(display: &mut T) -> Result<(), T::Error> {
    // Draw the left eye as a circle located at (50, 100), with a diameter of 40, filled with white
    Circle::new(Point::new(50, 100), 40)
        .into_styled(PrimitiveStyle::with_fill(Rgb565::WHITE))
        .draw(display)?;

    // Draw the right eye as a circle located at (50, 200), with a diameter of 40, filled with white
    Circle::new(Point::new(50, 200), 40)
        .into_styled(PrimitiveStyle::with_fill(Rgb565::WHITE))
        .draw(display)?;

    // Draw an upside down red triangle to represent a smiling mouth
    Triangle::new(
        Point::new(130, 140),
        Point::new(130, 200),
        Point::new(160, 170),
    )
    .into_styled(PrimitiveStyle::with_fill(Rgb565::RED))
    .draw(display)?;

    // Cover the top part of the mouth with a black triangle so it looks closed instead of open
    Triangle::new(
        Point::new(130, 150),
        Point::new(130, 190),
        Point::new(150, 170),
    )
    .into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK))
    .draw(display)?;

    Ok(())
}