#![no_std]
#![no_main]
use display_interface_spi::SPIInterface;

use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    mono_font::{ascii::FONT_9X18_BOLD, MonoTextStyle},
    text::Text,
};

use panic_rtt_target as _;
use rtt_target::{rprintln, rtt_init_print};

use stm32f1xx_hal::{
    pac::Peripherals,
    prelude::*,
    spi::{Spi, NoMiso},
};

use peripherals;

const W: u16 = 240;
const H: u16 = 240;
const FBUFF_SIZE: usize = (W * H) as usize;
// static mut FB: Framebuffer::<Rgb565, RawU16, LittleEndian, 240, 240, {buffer_size::<Rgb565>(240, 240)}> =
  // Framebuffer::<Rgb565, _, LittleEndian, 240, 240, {buffer_size::<Rgb565>(240, 240)}>::new();

// #[link_section=".ccmram.CCMRAM"]
// static mut FBUFF: [Rgb565; FBUFF_SIZE] = [Rgb565::BLACK; FBUFF_SIZE];

struct GayBuffer {
  fbuff: *mut [Rgb565; FBUFF_SIZE],
}

impl<'a> IntoIterator for &'a GayBuffer {
  fn into_iter(self) -> Self::IntoIter {
    unsafe { (*self.fbuff).into_iter() }
  }

  type Item = Rgb565;

  type IntoIter = core::array::IntoIter<Rgb565, FBUFF_SIZE>;
}


#[cortex_m_rt::entry]
fn main() -> ! {
  rtt_init_print!();
  rprintln!("Hello, world!");

  // let gb = GayBuffer{
  //   fbuff: unsafe { &mut FBUFF },
  // };

  let cp = cortex_m::Peripherals::take().unwrap();
  let dp = Peripherals::take().unwrap();

  let mut flash = dp.FLASH.constrain();
  let rcc = dp.RCC.constrain();

  let clocks = rcc.cfgr
      .hclk(72.MHz())
      .sysclk(72.MHz())
      .pclk2(72.MHz())
      .freeze(&mut flash.acr);

  // let mut delay = dp.TIM1.delay_us(&clocks);
  let mut delay = cp.SYST.delay(&clocks);

  // delay.delay_ms(5000u16);

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

  let bl = gpioa.pa8.into_push_pull_output(&mut gpioa.crh);
  let rst = gpioa.pa1.into_push_pull_output_with_state(&mut gpioa.crl, stm32f1xx_hal::gpio::PinState::High);
  let dc = gpioa.pa2.into_push_pull_output_with_state(&mut gpioa.crl, stm32f1xx_hal::gpio::PinState::High);

  let di = SPIInterface::new(spi, dc, cs);

  let mut display = peripherals::display::ST7789::new(
    di,
    Some(rst),
    Some(bl),
  );
  // .with_display_size(240, 240)
  // .with_framebuffer_size(240, 240)
  // .with_orientation(mipidsi::Orientation::Portrait(false))
  // .with_invert_colors(mipidsi::ColorInversion::Inverted)
  // .with_color_order(mipidsi::ColorOrder::Rgb)
  display.init(&mut delay).unwrap();

  // let fbuff = unsafe { FBUFF };
  // let framebuffer = Fra

  // unsafe {
  //   rprintln!("{}", gb.fbuff[0].b());
  //   rprintln!("{}", gb.fbuff[FBUFF_SIZE - 1].b());

  //   gb.fbuff[0] = Rgb565::BLUE;
  //   gb.fbuff[FBUFF_SIZE - 1] = Rgb565::BLUE;

  //   rprintln!("{}", gb.fbuff[0].b());
  //   rprintln!("{}", gb.fbuff[FBUFF_SIZE - 1].b());
  // }

  // let mut frame = FrameBuf::new(unsafe { &mut FBUFF }, W as usize, H as usize);

  // let area = Rectangle::new(Point::new(0, 0), Size { width: W as u32, height: H as u32 });
  // Clear the display initially
  // display.clear(Rgb565::BLACK).unwrap();

  // display.set_pixels(sx, sy, ex, ey, colors);

  // rprintln!("Buffer Init");

  // display.set_pixels(0, 0, W-1, H-1, &gb).unwrap();
//   display.set_pixels(0, 0, 19, 19, [Rgb565::YELLOW; 20*20]).unwrap();
  // display.fill_contiguous(&area, &gb);
  // display.

  // display.fill_contiguous(&area, fbuff).unwrap();

  rprintln!("Display Init");

  // display.draw_iter();

  let _text_fill = MonoTextStyle::new(&FONT_9X18_BOLD, Rgb565::WHITE);
  let text_clear = MonoTextStyle::new(&FONT_9X18_BOLD, Rgb565::WHITE);

  let _text_test = Text::new("Testing", Point::new(50, 50), text_clear);

  // let clear_style = PrimitiveStyleBuilder::new()
  //     .stroke_color(Rgb565::RED)
  //     .stroke_width(3)
  //     .fill_color(Rgb565::GREEN)
  //     .build();

  let cols = [Rgb565::YELLOW, Rgb565::BLUE, Rgb565::RED, Rgb565::GREEN];
  let mut counter: usize = 0;

  loop {
    // let _ = text_test.draw(&mut frame);
    // display.set_pixels(0, 0, W-1, H-1, unsafe { FBUFF }).unwrap();

    counter = counter + 1;

    if counter == 4 {
      counter = 0;
    }

    // for i in 0..FBUFF_SIZE {
    //   unsafe {
    //     // (*gb.fbuff)[i] = cols[counter];
    //     // FBUFF[i] = cols[counter];
    //   }

    // }
    // let _ = frame.clear(cols[counter]);

    display.clear(cols[counter]).unwrap();

    // let _ = display.draw_iter(&frame);

    // delay.delay_ms(1000u16);
  }
}
