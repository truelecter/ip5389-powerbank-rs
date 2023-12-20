#![no_std]
#![no_main]

use core::cell::RefCell;

use arrform::{arrform, ArrForm};
use cortex_m::interrupt::Mutex;
use display_interface_spi::SPIInterface;
use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    mono_font::{ascii::{FONT_9X18_BOLD}, MonoTextStyle},
    text::Text,
};

use mipidsi::Builder;

use ina3221::INA3221;
// use mipidsi::Builder;
use panic_rtt_target as _;
use rtt_target::{rprintln, rtt_init_print};

use stm32f1xx_hal::{
    pac::{Peripherals, TIM2},
    prelude::*,
    spi::{Spi, NoMiso}, i2c::{BlockingI2c, Mode}, timer::CounterMs,
};

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

    let clocks = rcc.cfgr
        .hclk(72.MHz())
        .sysclk(72.MHz())
        .freeze(&mut flash.acr);

    // let mut delay = dp.TIM1.delay_us(&clocks);
    let mut delay = cp.SYST.delay(&clocks);

    let mut afio = dp.AFIO.constrain();
    let mut gpioa = dp.GPIOA.split();

    // SPI1
    let sck = gpioa.pa5.into_alternate_push_pull(&mut gpioa.crl);
    let mosi = gpioa.pa7.into_alternate_push_pull(&mut gpioa.crl);
    let cs = gpioa.pa3.into_push_pull_output(&mut gpioa.crl);

    let spi = Spi::spi1(
        dp.SPI1,
        (sck, NoMiso, mosi),
        &mut afio.mapr,
        embedded_hal::spi::MODE_3,
        16.MHz(),
        clocks,
    );

    let rst = gpioa.pa1.into_push_pull_output_with_state(&mut gpioa.crl, stm32f1xx_hal::gpio::PinState::High);
    let dc = gpioa.pa2.into_push_pull_output_with_state(&mut gpioa.crl, stm32f1xx_hal::gpio::PinState::High);

    let di = SPIInterface::new(spi, dc, cs);

    let mut display = Builder::st7789(di)
        .with_display_size(240, 240)
        .with_orientation(mipidsi::Orientation::Portrait(false))
        .with_invert_colors(mipidsi::ColorInversion::Inverted)
        .with_color_order(mipidsi::ColorOrder::Rgb)
        .init(&mut delay, Some(rst))
        .unwrap();

    // Clear the display initially
    display.clear(Rgb565::BLACK).unwrap();

    rprintln!("Display Init");

    // INA PB6 PB7
    let mut gpiob = dp.GPIOB.split();

    let pb6 = gpiob.pb6.into_alternate_open_drain(&mut gpiob.crl);
    let pb7 = gpiob.pb7.into_alternate_open_drain(&mut gpiob.crl);

    let i2c = BlockingI2c::i2c1(
        dp.I2C1,
        (pb6, pb7),
        &mut afio.mapr,
        Mode::Standard { frequency: 100.kHz() },
        clocks,
        20,
        2,
        20,
        20,
    );

    // BQ4050 PB10, PB11
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

    let mut bq4050 = bq4050::BQ4050::new(i2c2);
    let mut ina = INA3221::new(i2c, ina3221::AddressPin::Gnd);

    ina.reset().unwrap();
    ina.enable_channel(ina3221::Channel::Ch1, true).unwrap();
    ina.enable_channel(ina3221::Channel::Ch2, true).unwrap();
    ina.enable_channel(ina3221::Channel::Ch3, true).unwrap();

    let text_fill = MonoTextStyle::new(&FONT_9X18_BOLD, Rgb565::WHITE);
    let text_clear = MonoTextStyle::new(&FONT_9X18_BOLD, Rgb565::WHITE);

    let mut toClear = Text::new(&"", Point::new(50, 50), text_clear);

    // let clear_style = PrimitiveStyleBuilder::new()
    //     .stroke_color(Rgb565::RED)
    //     .stroke_width(3)
    //     .fill_color(Rgb565::GREEN)
    //     .build();

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

                toClear.draw(&mut display).unwrap();

                let _ =
                    Text::new(arrform!(64, "Temp: {:.2}", sn).as_str(), Point::new(50, 50), text_fill)
                    .draw(&mut display).unwrap();
            },
            Err(e) => rprintln!("{:#?}", e),
        };
    }
}

