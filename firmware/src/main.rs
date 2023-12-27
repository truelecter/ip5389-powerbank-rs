// #![deny(unsafe_code)]
// #![deny(warnings)]
#![no_main]
#![no_std]

use panic_rtt_target as _;

use rtic::app;
use rtt_target::{rprintln, rtt_init_print};
use cortex_m_rt::pre_init;
use systick_monotonic::Systick;

use stm32f1xx_hal::{
  prelude::*,
  pac::{I2C2, I2C1, SPI1, TIM2, TIM4},
  timer::Timer,
  spi::{Spi, NoMiso, Spi1NoRemap},
  i2c::{BlockingI2c, Mode},
  timer::CounterMs,
  gpio::{Pin, Alternate, OpenDrain, PinState, Output, Input, PullDown,ExtiPin, Edge},
  gpio::gpioa::PA0,
};

// Display st7789(v?)
use embedded_graphics::{
  draw_target::DrawTarget,
  pixelcolor::{Rgb565, RgbColor},
  text::Text,
  geometry::Point,
  mono_font::{ascii::FONT_9X18_BOLD, MonoTextStyle},
  Drawable
};
use display_interface_spi::SPIInterface;
use mipidsi::{Builder, Display, models::ST7789};

use ina3221::INA3221;

use peripherals::bq4050::BQ4050;
use peripherals::bq4050;

#[pre_init]
unsafe fn preinit() -> () {
  // TODO - disable baclkight
}

#[app(device = stm32f1xx_hal::pac, peripherals = true)]
mod app {
  use super::*;

  static mut FMT_BUF: [u8; 64] = [0u8; 64];

  pub struct InaValues {
    bus: [f32; 3],
    volt: [f32; 3],
  }

  pub struct BqValues {
    temp: f32,
  }

  pub struct DisplayRedrawLocations {
    temp: Text<'static, MonoTextStyle<'static, Rgb565>>,
  }

  #[shared]
  struct Shared {
    ina: InaValues,
    bq: BqValues,
  }

  #[local]
  struct Local {
    data_timer: CounterMs<TIM4>,
    bq4050: BQ4050<BlockingI2c<I2C2, (Pin<'B', 10, Alternate<OpenDrain>>, Pin<'B', 11, Alternate<OpenDrain>>)>>,
    ina3221: INA3221<BlockingI2c<I2C1, (Pin<'B', 6, Alternate<OpenDrain>>, Pin<'B', 7, Alternate<OpenDrain>>)>>,

    button: PA0<Input<PullDown>>,

    display: Display<
      SPIInterface<
        Spi<SPI1, Spi1NoRemap, (Pin<'A', 5, Alternate>, NoMiso, Pin<'A', 7, Alternate>), u8>,
        Pin<'A', 2, Output>,
        Pin<'A', 3, Output>
      >,
      ST7789,
      Pin<'A', 1, Output>
    >,
    redraw_timer: CounterMs<TIM2>,
    fills: DisplayRedrawLocations,
  }

  #[monotonic(binds = SysTick, default = true)]
  type MonoTimer = Systick<1000>;

  #[init]
  fn init(mut cx: init::Context) -> (Shared, Local, init::Monotonics) {
    let mut flash = cx.device.FLASH.constrain();
    let rcc = cx.device.RCC.constrain();

    let mono = Systick::new(cx.core.SYST, 72_000_000);

    rtt_init_print!();
    rprintln!("init");

    let clocks = rcc
        .cfgr
        .hclk(72.MHz())
        .sysclk(72.MHz())
        .pclk2(72.MHz())
        .freeze(&mut flash.acr);

    let mut delay = cx.device.TIM3.delay_us(&clocks);

    let mut afio = cx.device.AFIO.constrain();
    let mut gpioa = cx.device.GPIOA.split();
    let mut gpiob = cx.device.GPIOB.split();

    // Display initialization
    // PA8 - led light. Low is off
    let backlight = gpioa.pa8.into_alternate_push_pull(&mut gpioa.crh);
    let mut backlight_pwm = Timer::new(cx.device.TIM1, &clocks)
      .pwm_hz(backlight, &mut afio.mapr, 1.kHz());

    let max = backlight_pwm.get_max_duty();
    backlight_pwm.enable(stm32f1xx_hal::timer::Channel::C1);
    backlight_pwm.set_duty(stm32f1xx_hal::timer::Channel::C1, max / 2);

    let sck = gpioa.pa5.into_alternate_push_pull(&mut gpioa.crl);
    let mosi = gpioa.pa7.into_alternate_push_pull(&mut gpioa.crl);
    let cs = gpioa.pa3.into_push_pull_output(&mut gpioa.crl);
    let rst = gpioa.pa1.into_push_pull_output_with_state(&mut gpioa.crl, PinState::High);
    let dc = gpioa.pa2.into_push_pull_output_with_state(&mut gpioa.crl, PinState::High);

    let spi = Spi::spi1(
      cx.device.SPI1,
      (sck, NoMiso, mosi),
      &mut afio.mapr,
      embedded_hal::spi::MODE_3,
      48.MHz(),
      clocks,
    );

    let di = SPIInterface::new(spi, dc, cs);

    rprintln!("Display init");
    let mut display = Builder::st7789(di)
        .with_display_size(240, 240)
        .with_orientation(mipidsi::Orientation::Portrait(false))
        .with_invert_colors(mipidsi::ColorInversion::Inverted)
        .with_color_order(mipidsi::ColorOrder::Rgb)
        .init(&mut delay, Some(rst))
        .unwrap();

    // Clear the display initially
    display.clear(Rgb565::RED).unwrap();

    rprintln!("Display init finished");

    // INA PB6 PB7
    let pb6 = gpiob.pb6.into_alternate_open_drain(&mut gpiob.crl);
    let pb7 = gpiob.pb7.into_alternate_open_drain(&mut gpiob.crl);

    rprintln!("INA3221 init");

    let i2c = BlockingI2c::i2c1(
      cx.device.I2C1,
      (pb6, pb7),
      &mut afio.mapr,
      Mode::Standard { frequency: 100.kHz() },
      clocks,
      100,
      5,
      20,
      20,
    );

    let mut ina3221 = INA3221::new(i2c, ina3221::AddressPin::Gnd);

    ina3221.reset().unwrap();
    ina3221.enable_channel(ina3221::Channel::Ch1, true).unwrap();
    ina3221.enable_channel(ina3221::Channel::Ch2, true).unwrap();
    ina3221.enable_channel(ina3221::Channel::Ch3, true).unwrap();

    rprintln!("INA3221 init finished");

    rprintln!("BQ4050 init");
    // look into https://github.com/cs2dsb/i2c_hung_fix.rs/blob/master/src/lib.rs for freeze fix
    let pb10 = gpiob.pb10.into_alternate_open_drain(&mut gpiob.crh);
    let pb11 = gpiob.pb11.into_alternate_open_drain(&mut gpiob.crh);

    let i2c2 = BlockingI2c::i2c2(
      cx.device.I2C2,
      (pb10, pb11),
      Mode::Standard {
        frequency: 100.kHz(),
      },
      clocks,
      20,
      1,
      20,
      20,
    );

    let bq4050 = bq4050::BQ4050::new(i2c2);
    rprintln!("BQ4050 init finished");

    let mut data_timer = cx.device.TIM4.counter_ms(&clocks);
    data_timer.start(1.secs()).unwrap();
    data_timer.listen(stm32f1xx_hal::timer::Event::Update);

    let mut redraw_timer = cx.device.TIM2.counter_ms(&clocks);
    redraw_timer.start(1.secs()).unwrap();
    redraw_timer.listen(stm32f1xx_hal::timer::Event::Update);

    let mut button = gpioa.pa0.into_pull_down_input(&mut gpioa.crl);
    button.make_interrupt_source(&mut afio);
    button.enable_interrupt(&mut cx.device.EXTI);
    button.trigger_on_edge(&mut cx.device.EXTI, Edge::Rising);

    (
      Shared {
        ina: InaValues {
          bus: [0.0;3],
          volt: [0.0;3],
        },
        bq: BqValues {
          temp: 0.0,
        },
      },
      Local {
        bq4050,
        ina3221,
        data_timer,
        redraw_timer,
        button,
        display,
        fills: DisplayRedrawLocations {
          temp: Text::new(
            "",
            Point::new(50, 50),
            MonoTextStyle::new(&FONT_9X18_BOLD, Rgb565::WHITE),
          ),
        },
      },
      init::Monotonics(mono),
    )
  }

  #[task(binds = TIM4, shared = [ina, bq], local = [bq4050, ina3221, data_timer])]
  fn data_timer_update(cx: data_timer_update::Context) {
    let ina3221 = cx.local.ina3221;
    let bq4050 = cx.local.bq4050;

    (cx.shared.bq, cx.shared.ina).lock(|bq, ina| {
      match ina3221.bus_voltage(ina3221::Channel::Ch1) {
        Ok(v) => ina.bus[0] = v,
        Err(e) => rprintln!("{:#?}", e),
      };

      match ina3221.bus_voltage(ina3221::Channel::Ch2) {
        Ok(v) => ina.bus[1] = v,
        Err(e) => rprintln!("{:#?}", e),
      };

      match ina3221.bus_voltage(ina3221::Channel::Ch3) {
        Ok(v) => ina.bus[2] = v,
        Err(e) => rprintln!("{:#?}", e),
      };

      match ina3221.shunt_voltage(ina3221::Channel::Ch1) {
        Ok(v) => ina.volt[0] = v,
        Err(e) => rprintln!("{:#?}", e),
      };

      match ina3221.shunt_voltage(ina3221::Channel::Ch2) {
        Ok(v) => ina.volt[1] = v,
        Err(e) => rprintln!("{:#?}", e),
      };

      match ina3221.shunt_voltage(ina3221::Channel::Ch3) {
        Ok(v) => ina.volt[2] = v,
        Err(e) => rprintln!("{:#?}", e),
      };

      match bq4050.get_temperature() {
        Ok(temp) => bq.temp = temp,
        Err(e) => rprintln!("{:#?}", e),
      };
    });

    cx.local.data_timer.clear_interrupt(stm32f1xx_hal::timer::Event::Update);
  }

  #[derive(Default)]
  struct DrawData {
    pack_temp: f32,
    bus: [f32; 3],
    volt: [f32; 3],
  }

  #[task(priority = 2, binds = TIM2, shared = [ina, bq], local = [display, redraw_timer, fills])]
  fn redraw_timer_update(cx: redraw_timer_update::Context) {
    let display = cx.local.display;
    let fills = cx.local.fills;
    let mut draw_data:DrawData = Default::default();

    (cx.shared.bq, cx.shared.ina).lock(|bq, ina| {
      for i in 0..=2 {
        draw_data.bus[i] = ina.bus[i];
        draw_data.volt[i] = ina.volt[i];
      }

      draw_data.pack_temp = bq.temp;
    });

    fills.temp.draw(display).unwrap();

    let buf = unsafe {&mut FMT_BUF };
    let v = format_no_std::show(
        buf,
        format_args!("Temp: {:.2}", draw_data.pack_temp),
    ).unwrap();

    Text::new(
      &v,
      Point::new(50, 50),
      MonoTextStyle::new(&FONT_9X18_BOLD, Rgb565::WHITE),
    )
    .draw(display).unwrap();

    fills.temp = Text::new(
      &v,
      Point::new(50, 50),
      MonoTextStyle::new(&FONT_9X18_BOLD, Rgb565::BLACK),
    );

    let int = cx.local.redraw_timer.get_interrupt();
    cx.local.redraw_timer.clear_interrupt(int);
  }

  #[task(binds = EXTI0, local = [button])]
  fn button_click(ctx: button_click::Context) {
    rprintln!("Button click");
    // Reset if helf for long
    // cortex_m::peripheral::SCB::sys_reset();
    ctx.local.button.clear_interrupt_pending_bit();
  }
}