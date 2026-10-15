//! Display throughput benchmark. Times the drawing paths the firmware uses with the DWT cycle
//! counter, at each SPI clock in `SPI_CLOCKS`, and prints the results over RTT.
#![no_std]
#![no_main]

use cortex_m::peripheral::DWT;
use embedded_graphics::{
  mono_font::{ascii::FONT_9X18_BOLD, MonoTextStyle, MonoTextStyleBuilder},
  pixelcolor::Rgb565,
  prelude::*,
  primitives::Rectangle,
  text::Text,
};
use embedded_hal::delay::DelayNs;
use embedded_hal_bus::spi::ExclusiveDevice;
use mipidsi::{
  interface::SpiInterface,
  models::ST7789,
  options::{ColorInversion, ColorOrder, Orientation},
  Builder,
};
use panic_rtt_target as _;
use rtt_target::{rprintln, rtt_init_print};
use stm32f1xx_hal::{
  gpio::PinState,
  pac::{self, Peripherals, SPI1},
  prelude::*,
  rcc,
  spi::SpiExt,
};

const SYSCLK_HZ: u32 = 240_000_000;
const PCLK2_HZ: u32 = 120_000_000;
const WIDTH: u16 = 240;
const HEIGHT: u16 = 240;
/// Bytes in one full RGB565 frame
const FRAME_BYTES: u32 = WIDTH as u32 * HEIGHT as u32 * 2;
/// Frames per measurement; the average is printed
const FRAMES: u32 = 10;
/// `(SPI1 BR divider bits, label)`: PCLK2 / 4 is what the firmware uses, PCLK2 / 2 is the
/// fastest the SPI can go.
const SPI_CLOCKS: [(u8, u32); 2] = [(0b001, PCLK2_HZ / 4), (0b000, PCLK2_HZ / 2)];

fn cycles_to_us(cycles: u32) -> u32 {
  cycles / (SYSCLK_HZ / 1_000_000)
}

/// Runs `f` `FRAMES` times and prints the average time per call and the resulting frame rate
fn bench<E: core::fmt::Debug>(name: &str, mut f: impl FnMut(u32) -> Result<(), E>) {
  let start = DWT::cycle_count();
  for frame in 0..FRAMES {
    f(frame).unwrap();
  }
  let us = cycles_to_us(DWT::cycle_count().wrapping_sub(start)) / FRAMES;
  let fps_x10 = 10_000_000 / us.max(1);
  rprintln!("  {:<28} {:>6} us  {:>4}.{} fps", name, us, fps_x10 / 10, fps_x10 % 10);
}

/// Changes the SPI1 baud rate divider. The peripheral must be idle, which it is between draws:
/// every SPI write waits for BSY to clear.
fn set_spi_divider(br: u8) {
  // SAFETY: only the BR and SPE bits of SPI1 change, while the bus is idle. The HAL keeps
  // no copy of the divider.
  let spi = unsafe { &*pac::SPI1::ptr() };
  spi.cr1().modify(|_, w| w.spe().clear_bit());
  spi.cr1().modify(|_, w| unsafe { w.br().bits(br) });
  spi.cr1().modify(|_, w| w.spe().set_bit());
}

#[cortex_m_rt::entry]
fn main() -> ! {
  rtt_init_print!();

  let mut cp = cortex_m::Peripherals::take().unwrap();
  let dp = Peripherals::take().unwrap();
  let mut flash = dp.FLASH.constrain();
  let mut rcc = dp.RCC.freeze(
    rcc::Config::hsi()
      .sysclk(SYSCLK_HZ.Hz())
      .hclk(SYSCLK_HZ.Hz())
      .pclk1(120.MHz())
      .pclk2(PCLK2_HZ.Hz()),
    &mut flash.acr,
  );
  cp.DCB.enable_trace();
  cp.DWT.enable_cycle_counter();
  let mut delay = cp.SYST.delay(&rcc.clocks);

  let mut gpioa = dp.GPIOA.split(&mut rcc);
  // Full backlight, so the drawing can be watched
  let _backlight = gpioa
    .pa8
    .into_push_pull_output_with_state(&mut gpioa.crh, PinState::High);
  let sck = gpioa.pa5.into_alternate_push_pull(&mut gpioa.crl);
  let mosi = gpioa.pa7.into_alternate_push_pull(&mut gpioa.crl);
  let cs = gpioa.pa3.into_push_pull_output(&mut gpioa.crl);
  let rst = gpioa
    .pa1
    .into_push_pull_output_with_state(&mut gpioa.crl, PinState::High);
  let dc = gpioa
    .pa2
    .into_push_pull_output_with_state(&mut gpioa.crl, PinState::High);

  let spi = dp.SPI1.spi(
    (Some(sck), SPI1::NoMiso, Some(mosi)),
    embedded_hal::spi::MODE_3.into(),
    (PCLK2_HZ / 4).Hz(),
    &mut rcc,
  );
  let spi = ExclusiveDevice::new_no_delay(spi, cs).unwrap();
  let mut buffer = [0u8; 512];
  let di = SpiInterface::new(spi, dc, &mut buffer);

  let start = DWT::cycle_count();
  let mut display = Builder::new(ST7789, di)
    .display_size(WIDTH, HEIGHT)
    .orientation(Orientation::new())
    .invert_colors(ColorInversion::Inverted)
    .color_order(ColorOrder::Rgb)
    .reset_pin(rst)
    .init(&mut delay)
    .unwrap();
  rprintln!(
    "fps: sysclk {} Hz, {}x{} display, {} frames per test",
    rcc.clocks.sysclk().raw(),
    WIDTH,
    HEIGHT,
    FRAMES
  );
  rprintln!("display init (reset and sleep-out delays) {} us", cycles_to_us(DWT::cycle_count().wrapping_sub(start)));

  let full = Rectangle::new(Point::zero(), Size::new(WIDTH as u32, HEIGHT as u32));
  let colors = [Rgb565::RED, Rgb565::GREEN, Rgb565::BLUE];
  let erase = MonoTextStyle::new(&FONT_9X18_BOLD, Rgb565::RED);
  let text = MonoTextStyle::new(&FONT_9X18_BOLD, Rgb565::WHITE);
  let text_bg = MonoTextStyleBuilder::new()
    .font(&FONT_9X18_BOLD)
    .text_color(Rgb565::WHITE)
    .background_color(Rgb565::RED)
    .build();
  let label = "Temp: 6276.85";

  loop {
    for (br, spi_hz) in SPI_CLOCKS {
      set_spi_divider(br);
      let ideal_us = FRAME_BYTES * 8 / (spi_hz / 1_000_000);
      rprintln!(
        "SPI {} MHz: one frame is {} bytes, {} us on the wire at best",
        spi_hz / 1_000_000,
        FRAME_BYTES,
        ideal_us
      );

      // `DrawTarget::clear` -> `fill_solid`: one window, the same pixel repeated
      bench("clear (fill_solid)", |i| display.clear(colors[(i % 3) as usize]));

      // `fill_contiguous`: one window, a color computed per pixel, as images and gradients do
      bench("fill_contiguous gradient", |i| {
        display.fill_contiguous(
          &full,
          (0..WIDTH as u32 * HEIGHT as u32).map(|p| Rgb565::new((p + i) as u8 & 0x1F, (p >> 4) as u8 & 0x3F, i as u8 & 0x1F)),
        )
      });

      // `draw_iter`: one window per pixel, the worst case
      bench("draw_iter 240x24 strip", |i| {
        display.draw_iter(
          Rectangle::new(Point::zero(), Size::new(WIDTH as u32, 24))
            .points()
            .map(|p| Pixel(p, colors[(i % 3) as usize])),
        )
      });

      // The firmware's redraw: erase the old text in the background color, draw the new one.
      // Without a background color, the font draws only the set pixels.
      display.clear(Rgb565::RED).unwrap();
      bench("text erase + draw", |_| {
        Text::new(label, Point::new(50, 50), erase).draw(&mut display)?;
        Text::new(label, Point::new(50, 50), text).draw(&mut display).map(|_| ())
      });

      // The same text with a background color: each glyph is a single window
      bench("text with background", |_| {
        Text::new(label, Point::new(50, 50), text_bg).draw(&mut display).map(|_| ())
      });
    }
    DelayNs::delay_ms(&mut delay, 3000);
  }
}
