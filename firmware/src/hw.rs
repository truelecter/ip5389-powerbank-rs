#![no_std]
#![no_main]
use display_interface_spi::SPIInterface;

use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    mono_font::{ascii::FONT_9X18_BOLD, MonoTextStyle},
    text::Text,
};

use embedded_layout::align::{Align, horizontal, vertical};
use graphics::volttable::VoltTableRow;
use panic_rtt_target as _;
use rtt_target::{rprintln, rtt_init_print};

use stm32f1xx_hal::{
    pac::Peripherals,
    prelude::*,
    spi::{Spi, NoMiso},
};

use mipidsi::{Builder, Display, models::ST7789};
use peripherals;

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
      .pclk2(72.MHz())
      .freeze(&mut flash.acr);

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
      48.MHz(),
      clocks,
  );

  // let bl = gpioa.pa8.into_push_pull_output(&mut gpioa.crh);
  let rst = gpioa.pa1.into_push_pull_output_with_state(&mut gpioa.crl, stm32f1xx_hal::gpio::PinState::High);
  let dc = gpioa.pa2.into_push_pull_output_with_state(&mut gpioa.crl, stm32f1xx_hal::gpio::PinState::High);

  let di = SPIInterface::new(spi, dc, cs);

  rprintln!("Display init");
  let mut display = Builder::st7789(di)
      .with_display_size(240, 240)
      .with_orientation(mipidsi::Orientation::Portrait(false))
      .with_invert_colors(mipidsi::ColorInversion::Inverted)
      .with_color_order(mipidsi::ColorOrder::Rgb)
      .init(&mut delay, Some(rst))
      .unwrap();

  rprintln!("Display Init");

  display.clear(Rgb565::RED).unwrap();

  let cols = [Rgb565::YELLOW, Rgb565::BLUE, Rgb565::RED, Rgb565::GREEN];
  let mut counter: usize = 0;

  let r1 = VoltTableRow::new(
    Point::new(0, 0),
    240,
    Rgb565::CYAN,
    Rgb565::CYAN,
    "EXT1"
  );

  r1.draw(&mut display).unwrap();

  let r2 = VoltTableRow::new(
    Point::new(0, 0),
    240,
    Rgb565::CYAN,
    Rgb565::CYAN,
    "EXT2"
  )
    .align_to(&r1, horizontal::Center, vertical::TopToBottom);

  r2.draw(&mut display).unwrap();


  loop {
    // let _ = text_test.draw(&mut frame);
    // display.set_pixels(0, 0, W-1, H-1, unsafe { FBUFF }).unwrap();

    // counter = counter + 1;

    // if counter == 4 {
    //   counter = 0;
    // }

    // display.clear(cols[counter]).unwrap();
  }
}
