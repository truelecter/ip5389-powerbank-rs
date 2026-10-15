// #![deny(unsafe_code)]
// #![deny(warnings)]
#![no_main]
#![no_std]

use panic_rtt_target as _;

use rtic::app;
use rtt_target::{rprintln, rtt_init_print};

use stm32f1xx_hal::{
  gpio::gpioa::PA0,
  gpio::{Edge, ExtiPin, Input, OpenDrain, Output, Pin, PinState, PullDown},
  i2c::{BlockingI2c, Mode},
  pac::{I2C1, SPI1, TIM2, TIM4},
  prelude::*,
  rcc,
  spi::{Spi, SpiExt},
  timer::{Channel, Counter, Event, Timer},
};

/// System clock, the AT32F403A maximum
const SYSCLK_HZ: u32 = 240_000_000;
/// Tick rate of the data and redraw timers. Low enough for a 16-bit prescaler at 240 MHz,
/// high enough for a 16-bit counter to reach a second.
const TIMER_HZ: u32 = 10_000;
/// INA3221 I2C start, address and per-byte timeout
const I2C_TIMEOUT_US: u32 = 35_000;
/// BQ4050 SMBus clock, bit-banged: the I2C peripheral garbles reads while the BQ4050
/// stretches the clock
const SMBUS_HZ: u32 = 100_000;
/// INA3221 shunt resistors, channels 1 to 3: the `R005` (5 mΩ) resistors in the output paths.
/// Checked on USB2 charging a phone: 36.7 W measured here for 33 W at the port (about 90 %).
const SHUNT_OHMS: [f32; 3] = [0.005; 3];
/// Shunt reading with no current, per volt on the bus. It grows with the bus voltage (0.04 mV
/// at 2.5 V, 0.28 mV at 16.4 V), so it most likely is the INA3221 input bias current through
/// the board's input filter resistors. Uncorrected, an idle module shows 56–64 mA; with a
/// phone charging on USB2 the battery current measured by the BQ4050 left no room for that.
const SHUNT_OFFSET_PER_BUS_VOLT: [f32; 3] = [17.0e-6, 19.5e-6, 17.0e-6];
/// Port on each INA3221 channel: left and right USB-C modules and the DC port, each checked
/// with a load
const PORT_NAMES: [&str; 3] = ["USB1", "USB2", "DC"];

// Display st7789(v?)
use embedded_hal_bus::spi::{ExclusiveDevice, NoDelay};
use embedded_graphics::{
  draw_target::DrawTarget,
  geometry::Point,
  mono_font::ascii::FONT_9X18_BOLD,
  pixelcolor::{Rgb565, RgbColor, WebColors},
};
use graphics::textual::TextField;
use mipidsi::{
  interface::SpiInterface,
  models::ST7789,
  options::{ColorInversion, ColorOrder, Orientation},
  Builder, Display,
};

use peripherals::ina3221;
use peripherals::ina3221::INA3221;

use peripherals::bq4050;
use peripherals::bq4050::BQ4050;
use peripherals::soft_i2c::{self, SoftI2c};

use cortex_m::delay::Delay;

/// Runs before RAM is initialized. Replaces the deprecated `#[cortex_m_rt::pre_init]`.
#[unsafe(export_name = "__pre_init")]
unsafe extern "C" fn preinit() {
  // TODO - disable baclkight
}

#[app(device = stm32f1xx_hal::pac, peripherals = true)]
mod app {
  use super::*;

  pub struct InaValues {
    bus: [f32; 3],
    volt: [f32; 3],
  }

  pub struct BqValues {
    temp: f32,
  }

  /// Screen background, also the background of every text field
  const BACKGROUND: Rgb565 = Rgb565::BLACK;
  /// Characters per text line: 26 fit the display width
  const LINE: usize = 25;

  /// What the redraw task shows. Each field redraws only the characters that changed.
  pub struct Screen {
    temp: TextField<LINE>,
    bus: [TextField<LINE>; 3],
    /// Time since boot, changes every redraw: shows the firmware is alive
    uptime: TextField<LINE>,
    /// Redraws since boot; the redraw timer fires once a second
    seconds: u32,
  }

  impl Screen {
    fn new() -> Self {
      let line = |row: i32| {
        TextField::new(Point::new(12, 20 + row * 24), &FONT_9X18_BOLD, Rgb565::WHITE, BACKGROUND)
      };
      Self {
        temp: line(0),
        bus: [line(1), line(2), line(3)],
        uptime: TextField::new(Point::new(12, 200), &FONT_9X18_BOLD, Rgb565::CSS_GRAY, BACKGROUND),
        seconds: 0,
      }
    }
  }

  #[shared]
  struct Shared {
    ina: InaValues,
    bq: BqValues,
  }

  #[local]
  struct Local {
    data_timer: Counter<TIM4, TIMER_HZ>,
    bq4050: BQ4050<SoftI2c<Pin<'B', 10, Output<OpenDrain>>, Pin<'B', 11, Output<OpenDrain>>, Delay>>,
    ina3221: INA3221<BlockingI2c<I2C1>>,

    button: PA0<Input<PullDown>>,

    display: Display<
      SpiInterface<
        'static,
        ExclusiveDevice<Spi<SPI1, u8>, Pin<'A', 3, Output>, NoDelay>,
        Pin<'A', 2, Output>,
      >,
      ST7789,
      Pin<'A', 1, Output>,
    >,
    redraw_timer: Counter<TIM2, TIMER_HZ>,
    screen: Screen,
  }

  #[init(local = [display_buffer: [u8; 512] = [0; 512]])]
  fn init(mut cx: init::Context) -> (Shared, Local) {
    let mut flash = cx.device.FLASH.constrain();

    // HICK/2 * 60 = 240 MHz, APB1 and APB2 at their 120 MHz maximum
    let mut rcc = cx.device.RCC.freeze(
      rcc::Config::hsi()
        .sysclk(SYSCLK_HZ.Hz())
        .hclk(SYSCLK_HZ.Hz())
        .pclk1(120.MHz())
        .pclk2(120.MHz()),
      &mut flash.acr,
    );
    let clocks = rcc.clocks;

    // BlockingI2c measures its timeouts with the DWT cycle counter
    cx.core.DCB.enable_trace();
    cx.core.DWT.enable_cycle_counter();

    rtt_init_print!();
    rprintln!(
      "init: sysclk {} hclk {} pclk1 {} pclk2 {}",
      clocks.sysclk(),
      clocks.hclk(),
      clocks.pclk1(),
      clocks.pclk2()
    );

    let mut delay = cx.device.TIM3.delay_us(&mut rcc);

    let mut afio = cx.device.AFIO.constrain(&mut rcc);
    let mut gpioa = cx.device.GPIOA.split(&mut rcc);
    let mut gpiob = cx.device.GPIOB.split(&mut rcc);

    // Display initialization
    // PA8 - led light. Low is off
    let backlight = gpioa.pa8.into_alternate_push_pull(&mut gpioa.crh);
    let mut backlight_pwm =
      Timer::new(cx.device.TIM1, &mut rcc).pwm_hz(backlight, &mut afio.mapr, 1.kHz());

    let max = backlight_pwm.get_max_duty();
    backlight_pwm.enable(Channel::C1);
    backlight_pwm.set_duty(Channel::C1, max / 2);

    let sck = gpioa.pa5.into_alternate_push_pull(&mut gpioa.crl);
    let mosi = gpioa.pa7.into_alternate_push_pull(&mut gpioa.crl);
    let cs = gpioa.pa3.into_push_pull_output(&mut gpioa.crl);
    let rst = gpioa
      .pa1
      .into_push_pull_output_with_state(&mut gpioa.crl, PinState::High);
    let dc = gpioa
      .pa2
      .into_push_pull_output_with_state(&mut gpioa.crl, PinState::High);

    // APB2 120 MHz / 2, the fastest SPI1 clock. The ST7789 accepts writes up to about 62 MHz.
    let spi = cx.device.SPI1.spi(
      (Some(sck), SPI1::NoMiso, Some(mosi)),
      embedded_hal::spi::MODE_3.into(),
      60.MHz(),
      &mut rcc,
    );

    let spi = ExclusiveDevice::new_no_delay(spi, cs).unwrap();
    let di = SpiInterface::new(spi, dc, cx.local.display_buffer);

    rprintln!("Display init");
    let mut display = Builder::new(ST7789, di)
      .display_size(240, 240)
      .orientation(Orientation::new())
      .invert_colors(ColorInversion::Inverted)
      .color_order(ColorOrder::Rgb)
      .reset_pin(rst)
      .init(&mut delay)
      .unwrap();

    // Clear the display initially
    display.clear(BACKGROUND).unwrap();

    rprintln!("Display init finished");

    rprintln!("INA3221 init");

    // A reset in the middle of a read can leave the INA3221 holding SDA low: clock it free
    // on the bare pins before the I2C peripheral takes them
    let mut scl = gpiob.pb6.into_open_drain_output_with_state(&mut gpiob.crl, PinState::High);
    let mut sda = gpiob.pb7.into_open_drain_output_with_state(&mut gpiob.crl, PinState::High);
    if let Err(e) = soft_i2c::recover_bus(&mut scl, &mut sda, &mut delay, 100_000) {
      rprintln!("INA3221 bus recovery: {:?}", e);
    }
    let i2c = BlockingI2c::new(
      cx.device.I2C1,
      (scl.into_floating_input(&mut gpiob.crl), sda.into_floating_input(&mut gpiob.crl)),
      Mode::Standard {
        frequency: 100.kHz(),
      },
      &mut rcc,
      I2C_TIMEOUT_US,
      5,
      I2C_TIMEOUT_US,
      I2C_TIMEOUT_US,
    );

    let mut ina3221 = INA3221::new(i2c, ina3221::AddressPin::Gnd);

    // On failure the data task keeps reporting read errors, and the display keeps working
    let setup = ina3221.reset().and_then(|()| {
      [ina3221::Channel::Ch1, ina3221::Channel::Ch2, ina3221::Channel::Ch3]
        .into_iter()
        .try_for_each(|channel| ina3221.enable_channel(channel, true))
    });
    if let Err(e) = setup {
      rprintln!("INA3221 setup: {:?}", e);
    }

    rprintln!("INA3221 init finished");

    rprintln!("BQ4050 init");
    let scl = gpiob.pb10.into_open_drain_output_with_state(&mut gpiob.crh, PinState::High);
    let sda = gpiob.pb11.into_open_drain_output_with_state(&mut gpiob.crh, PinState::High);
    let mut smbus = SoftI2c::new(scl, sda, Delay::new(cx.core.SYST, SYSCLK_HZ), SMBUS_HZ);
    if let Err(e) = smbus.recover() {
      rprintln!("BQ4050 bus recovery: {:?}", e);
    }

    let bq4050 = bq4050::BQ4050::new(smbus);
    rprintln!("BQ4050 init finished");

    let mut data_timer = cx.device.TIM4.counter::<TIMER_HZ>(&mut rcc);
    data_timer.start(1.secs()).unwrap();
    data_timer.listen(Event::Update);

    let mut redraw_timer = cx.device.TIM2.counter::<TIMER_HZ>(&mut rcc);
    redraw_timer.start(1.secs()).unwrap();
    redraw_timer.listen(Event::Update);

    let mut button = gpioa.pa0.into_pull_down_input(&mut gpioa.crl);
    button.make_interrupt_source(&mut afio);
    button.enable_interrupt(&mut cx.device.EXTI);
    button.trigger_on_edge(&mut cx.device.EXTI, Edge::Rising);

    (
      Shared {
        ina: InaValues {
          bus: [0.0; 3],
          volt: [0.0; 3],
        },
        bq: BqValues { temp: 0.0 },
      },
      Local {
        bq4050,
        ina3221,
        data_timer,
        redraw_timer,
        button,
        display,
        screen: Screen::new(),
      },
    )
  }

  #[idle()]
  fn idle(_: idle::Context) -> ! {
    loop {
      // Now Wait For Interrupt is used instead of a busy-wait loop
      // to allow MCU to sleep between interrupts
      // https://developer.arm.com/documentation/ddi0406/c/Application-Level-Architecture/Instruction-Details/Alphabetical-list-of-instructions/WFI
      cortex_m::asm::wfi()
    }
  }

  #[task(binds = TIM4, shared = [ina, bq], local = [bq4050, ina3221, data_timer])]
  fn data_timer_update(cx: data_timer_update::Context) {
    let ina3221 = cx.local.ina3221;
    let bq4050 = cx.local.bq4050;
    let timer = cx.local.data_timer;

    (cx.shared.bq, cx.shared.ina).lock(|bq, ina| {
      match ina3221.bus_voltage(ina3221::Channel::Ch1) {
        Ok(v) => ina.bus[0] = v,
        Err(e) => { rprintln!("{:#?}", e); }
      };

      match ina3221.bus_voltage(ina3221::Channel::Ch2) {
        Ok(v) => ina.bus[1] = v,
        Err(e) => { rprintln!("{:#?}", e); }
      };

      match ina3221.bus_voltage(ina3221::Channel::Ch3) {
        Ok(v) => ina.bus[2] = v,
        Err(e) => { rprintln!("{:#?}", e); }
      };

      match ina3221.shunt_voltage(ina3221::Channel::Ch1) {
        Ok(v) => ina.volt[0] = v,
        Err(e) => { rprintln!("{:#?}", e); }
      };

      match ina3221.shunt_voltage(ina3221::Channel::Ch2) {
        Ok(v) => ina.volt[1] = v,
        Err(e) => { rprintln!("{:#?}", e); }
      };

      match ina3221.shunt_voltage(ina3221::Channel::Ch3) {
        Ok(v) => ina.volt[2] = v,
        Err(e) => { rprintln!("{:#?}", e); }
      };

      match bq4050.get_temperature() {
        Ok(temp) => bq.temp = temp,
        Err(e) => { rprintln!("{:#?}", e); }
      };
    });

    let int = timer.get_interrupt();
    timer.clear_interrupt(int);
  }

  #[derive(Default)]
  struct DrawData {
    pack_temp: f32,
    bus: [f32; 3],
    volt: [f32; 3],
  }

  #[task(priority = 2, binds = TIM2, shared = [ina, bq], local = [display, redraw_timer, screen])]
  fn redraw_timer_update(cx: redraw_timer_update::Context) {
    let display = cx.local.display;
    let screen = cx.local.screen;
    let mut draw_data: DrawData = Default::default();
    let timer = cx.local.redraw_timer;

    (cx.shared.bq, cx.shared.ina).lock(|bq, ina| {
      for i in 0..=2 {
        draw_data.bus[i] = ina.bus[i];
        draw_data.volt[i] = ina.volt[i];
      }

      draw_data.pack_temp = bq.temp;
    });

    let start = cortex_m::peripheral::DWT::cycle_count();
    let mut buf = [0u8; LINE];
    let mut chars = 0;

    let text = format_no_std::show(&mut buf, format_args!("Temp {:>9.2} C", draw_data.pack_temp));
    chars += screen.temp.update(text.unwrap_or("Temp ?"), display).unwrap();

    for (i, field) in screen.bus.iter_mut().enumerate() {
      let amps = (draw_data.volt[i] - SHUNT_OFFSET_PER_BUS_VOLT[i] * draw_data.bus[i]) / SHUNT_OHMS[i];
      let text = format_no_std::show(
        &mut buf,
        format_args!(
          "{:<4}{:>6.2}V{:>7.3}A{:>5.1}W",
          PORT_NAMES[i],
          draw_data.bus[i],
          amps,
          draw_data.bus[i] * amps
        ),
      );
      chars += field.update(text.unwrap_or("?"), display).unwrap();
    }

    let s = screen.seconds;
    let text = format_no_std::show(&mut buf, format_args!("Up {}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60));
    chars += screen.uptime.update(text.unwrap_or("Up ?"), display).unwrap();
    screen.seconds = s.wrapping_add(1);

    let us = cortex_m::peripheral::DWT::cycle_count().wrapping_sub(start) / (SYSCLK_HZ / 1_000_000);
    rprintln!("redraw: {} chars in {} us", chars, us);

    let int = timer.get_interrupt();
    timer.clear_interrupt(int);
  }

  #[task(binds = EXTI0, local = [button])]
  fn button_click(ctx: button_click::Context) {
    rprintln!("Button click");
    // Reset if helf for long
    // cortex_m::peripheral::SCB::sys_reset();
    ctx.local.button.clear_interrupt_pending_bit();
  }
}
