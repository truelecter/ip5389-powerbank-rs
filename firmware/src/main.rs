// #![deny(unsafe_code)]
// #![deny(warnings)]
#![no_main]
#![no_std]

mod clock;

use core::sync::atomic::{AtomicBool, Ordering};

use panic_rtt_target as _;

use rtic::app;
use rtt_target::{rprintln, rtt_init_print};

use stm32f1xx_hal::{
  gpio::gpioa::PA0,
  gpio::{Edge, ExtiPin, Input, OpenDrain, Output, Pin, PinState, PullDown},
  i2c::{BlockingI2c, Mode},
  pac::{self, I2C1, SPI1, TIM1},
  prelude::*,
  rtc::{Rtc, RtcClkLsi},
  spi::{Spi, SpiExt},
  timer::{PwmChannel, Timer},
};

use clock::{Clock, CycleDelay};

/// INA3221 I2C start, address and per-byte timeout
const I2C_TIMEOUT_US: u32 = 35_000;
/// BQ4050 SMBus clock, bit-banged: the I2C peripheral garbles reads while the BQ4050
/// stretches the clock
const SMBUS_HZ: u32 = 100_000;
/// Display SPI clock as set up at the idle clock; it runs at 24 MHz in the draw profile
const SPI_IDLE_HZ: u32 = 4_000_000;
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
/// Backlight duty cycle in percent. Measured from the battery: about 6 mA at 50 %.
const BACKLIGHT_BRIGHT: u8 = 50;
const BACKLIGHT_DIM: u8 = 10;

// Display st7789(v?)
use embedded_graphics::{
  draw_target::DrawTarget,
  geometry::Point,
  mono_font::ascii::FONT_9X18_BOLD,
  pixelcolor::{Rgb565, RgbColor, WebColors},
};
use embedded_hal::pwm::SetDutyCycle;
use embedded_hal_bus::spi::{ExclusiveDevice, NoDelay};
use graphics::textual::TextField;
use mipidsi::{
  interface::SpiInterface,
  models::ST7789,
  options::{ColorInversion, ColorOrder, Orientation},
  Builder, Display,
};

use peripherals::bq4050::BQ4050;
use peripherals::ina3221::{self, INA3221};
use peripherals::soft_i2c::{self, SoftI2c};

use power::{main_port_a, Level, Readings, ScreenPower};

/// Set by the button interrupt, taken by the next update
static BUTTON: AtomicBool = AtomicBool::new(false);

/// Runs before RAM is initialized. Replaces the deprecated `#[cortex_m_rt::pre_init]`.
#[unsafe(export_name = "__pre_init")]
unsafe extern "C" fn preinit() {
  // TODO - disable baclkight
}

#[app(device = stm32f1xx_hal::pac, peripherals = true)]
mod app {
  use super::*;

  /// One second of measurements
  #[derive(Default)]
  struct Measurements {
    bus_v: [f32; 3],
    port_a: [f32; 3],
    pack_v: f32,
    pack_a: f32,
    pack_temp: f32,
  }

  /// Screen background, also the background of every text field
  const BACKGROUND: Rgb565 = Rgb565::BLACK;
  /// Characters per text line: 26 fit the display width
  const LINE: usize = 25;

  /// What the update task shows. Each field redraws only the characters that changed.
  pub struct Screen {
    temp: TextField<LINE>,
    pack: TextField<LINE>,
    ports: [TextField<LINE>; 3],
    /// The charger chip's USB-C and USB-A ports, from the current balance
    main: TextField<LINE>,
    /// Time since boot: shows the firmware is alive
    uptime: TextField<LINE>,
  }

  impl Screen {
    fn new() -> Self {
      let line = |row: i32| {
        TextField::new(Point::new(12, 20 + row * 24), &FONT_9X18_BOLD, Rgb565::WHITE, BACKGROUND)
      };
      Self {
        temp: line(0),
        pack: line(1),
        ports: [line(2), line(3), line(4)],
        main: line(5),
        uptime: TextField::new(Point::new(12, 200), &FONT_9X18_BOLD, Rgb565::CSS_GRAY, BACKGROUND),
      }
    }
  }

  type St7789 = Display<
    SpiInterface<'static, ExclusiveDevice<Spi<SPI1, u8>, Pin<'A', 3, Output>, NoDelay>, Pin<'A', 2, Output>>,
    ST7789,
    Pin<'A', 1, Output>,
  >;

  #[shared]
  struct Shared {}

  #[local]
  struct Local {
    clock: Clock,
    rtc: Rtc<RtcClkLsi>,
    bq4050: BQ4050<SoftI2c<Pin<'B', 10, Output<OpenDrain>>, Pin<'B', 11, Output<OpenDrain>>, CycleDelay>>,
    ina3221: INA3221<BlockingI2c<I2C1>>,
    display: St7789,
    backlight: PwmChannel<TIM1, 0>,
    screen: Screen,
    power: ScreenPower,
    seconds: u32,
    level: Level,
    button: PA0<Input<PullDown>>,
  }

  #[init(local = [display_buffer: [u8; 512] = [0; 512]])]
  fn init(mut cx: init::Context) -> (Shared, Local) {
    let mut flash = cx.device.FLASH.constrain();
    let mut rcc = cx.device.RCC.freeze(clock::idle_config(), &mut flash.acr);

    // BlockingI2c and CycleDelay count DWT cycles
    cx.core.DCB.enable_trace();
    cx.core.DWT.enable_cycle_counter();

    rtt_init_print!();
    rprintln!("init: sysclk {} idle, {} while drawing", rcc.clocks.sysclk(), clock::DRAW_HZ);

    let mut delay = CycleDelay;
    let mut afio = cx.device.AFIO.constrain(&mut rcc);
    let mut gpioa = cx.device.GPIOA.split(&mut rcc);
    let mut gpiob = cx.device.GPIOB.split(&mut rcc);

    // PA8 - backlight, low is off
    let backlight = gpioa.pa8.into_alternate_push_pull(&mut gpioa.crh);
    let mut backlight = Timer::new(cx.device.TIM1, &mut rcc)
      .pwm_hz(backlight, &mut afio.mapr, 1.kHz())
      .split();
    backlight.set_duty_cycle_percent(0).ok();
    backlight.enable();

    let sck = gpioa.pa5.into_alternate_push_pull(&mut gpioa.crl);
    let mosi = gpioa.pa7.into_alternate_push_pull(&mut gpioa.crl);
    let cs = gpioa.pa3.into_push_pull_output(&mut gpioa.crl);
    let rst = gpioa.pa1.into_push_pull_output_with_state(&mut gpioa.crl, PinState::High);
    let dc = gpioa.pa2.into_push_pull_output_with_state(&mut gpioa.crl, PinState::High);
    let spi = cx.device.SPI1.spi(
      (Some(sck), SPI1::NoMiso, Some(mosi)),
      embedded_hal::spi::MODE_3.into(),
      SPI_IDLE_HZ.Hz(),
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
      Mode::Standard { frequency: 100.kHz() },
      &mut rcc,
      I2C_TIMEOUT_US,
      5,
      I2C_TIMEOUT_US,
      I2C_TIMEOUT_US,
    );
    let mut ina3221 = INA3221::new(i2c, ina3221::AddressPin::Gnd);
    // On failure the update task keeps reporting read errors, and the display keeps working
    let setup = ina3221.reset().and_then(|()| {
      [ina3221::Channel::Ch1, ina3221::Channel::Ch2, ina3221::Channel::Ch3]
        .into_iter()
        .try_for_each(|channel| ina3221.enable_channel(channel, true))
    });
    if let Err(e) = setup {
      rprintln!("INA3221 setup: {:?}", e);
    }

    rprintln!("BQ4050 init");
    let scl = gpiob.pb10.into_open_drain_output_with_state(&mut gpiob.crh, PinState::High);
    let sda = gpiob.pb11.into_open_drain_output_with_state(&mut gpiob.crh, PinState::High);
    let mut smbus = SoftI2c::new(scl, sda, CycleDelay, SMBUS_HZ);
    if let Err(e) = smbus.recover() {
      rprintln!("BQ4050 bus recovery: {:?}", e);
    }
    let bq4050 = BQ4050::new(smbus);

    // 1 Hz tick from LSI, independent of the clock profile
    let mut bkp = cx.device.BKP.constrain(&mut cx.device.PWR, &mut rcc);
    let mut rtc = Rtc::new_lsi(cx.device.RTC, &mut bkp);
    rtc.listen_seconds();

    let mut button = gpioa.pa0.into_pull_down_input(&mut gpioa.crl);
    button.make_interrupt_source(&mut afio);
    button.enable_interrupt(&mut cx.device.EXTI);
    button.trigger_on_edge(&mut cx.device.EXTI, Edge::Rising);

    let mut clock = Clock::new(rcc, flash.acr);
    clock.fast(|| display.clear(BACKGROUND)).unwrap();
    rprintln!("init finished");
    // The first update lights the backlight
    BUTTON.store(true, Ordering::Relaxed);
    rtic::pend(pac::Interrupt::RTC);

    (
      Shared {},
      Local {
        clock,
        rtc,
        bq4050,
        ina3221,
        display,
        backlight,
        screen: Screen::new(),
        power: ScreenPower::new(),
        seconds: 0,
        level: Level::Off,
        button,
      },
    )
  }

  #[idle()]
  fn idle(_: idle::Context) -> ! {
    loop {
      // Sleep until the next interrupt
      cortex_m::asm::wfi()
    }
  }

  /// Once a second, and right after a button press: measure, decide the screen level, redraw
  #[task(binds = RTC, local = [clock, rtc, bq4050, ina3221, display, backlight, screen, power, seconds, level])]
  fn update(cx: update::Context) {
    // SAFETY: read-only access to the RTC flags; the second flag is cleared through the HAL
    let tick = unsafe { (*pac::RTC::ptr()).crl().read().secf().bit_is_set() };
    if tick {
      cx.local.rtc.clear_second_flag();
      *cx.local.seconds += 1;
    }
    let button = BUTTON.swap(false, Ordering::Relaxed);

    let m = measure(cx.local.ina3221, cx.local.bq4050);
    let readings = Readings { ports_a: m.port_a, pack_a: m.pack_a };
    let level = cx.local.power.update(&readings, button, tick as u32);
    if level != *cx.local.level {
      // The pack current still shows the previous level: the gauge averages over a second
      rprintln!("{}s: screen {:?} -> {:?}, pack {} mA", cx.local.seconds, cx.local.level, level, (m.pack_a * 1000.0) as i32);
      *cx.local.level = level;
    }

    let display = cx.local.display;
    let duty = match level {
      Level::Bright => BACKLIGHT_BRIGHT,
      Level::Dim => BACKLIGHT_DIM,
      Level::Off => 0,
    };
    if level == Level::Off {
      cx.local.backlight.set_duty_cycle_percent(0).ok();
      if !display.is_sleeping() {
        display.sleep(&mut CycleDelay).ok();
      }
      return;
    }
    if display.is_sleeping() {
      // The panel keeps its frame memory while asleep, so the fields stay valid
      display.wake(&mut CycleDelay).ok();
    }
    cx.local.backlight.set_duty_cycle_percent(duty).ok();

    let screen = cx.local.screen;
    let seconds = *cx.local.seconds;
    cx.local.clock.fast(|| draw(screen, display, &m, seconds));
  }

  fn measure(
    ina3221: &mut INA3221<BlockingI2c<I2C1>>,
    bq4050: &mut BQ4050<SoftI2c<Pin<'B', 10, Output<OpenDrain>>, Pin<'B', 11, Output<OpenDrain>>, CycleDelay>>,
  ) -> Measurements {
    let mut m = Measurements::default();
    let channels = [ina3221::Channel::Ch1, ina3221::Channel::Ch2, ina3221::Channel::Ch3];
    for (i, channel) in channels.into_iter().enumerate() {
      match (ina3221.bus_voltage(channel), ina3221.shunt_voltage(channel)) {
        (Ok(bus), Ok(shunt)) => {
          m.bus_v[i] = bus;
          m.port_a[i] = (shunt - SHUNT_OFFSET_PER_BUS_VOLT[i] * bus) / SHUNT_OHMS[i];
        }
        (Err(e), _) | (_, Err(e)) => {
          rprintln!("INA3221 {}: {:?}", PORT_NAMES[i], e);
        }
      }
    }
    match bq4050.get_voltage() {
      Ok(mv) => m.pack_v = mv as f32 / 1000.0,
      Err(e) => {
        rprintln!("BQ4050 voltage: {:?}", e);
      }
    }
    match bq4050.get_current() {
      Ok(ma) => m.pack_a = ma as f32 / 1000.0,
      Err(e) => {
        rprintln!("BQ4050 current: {:?}", e);
      }
    }
    match bq4050.get_temperature() {
      Ok(temp) => m.pack_temp = temp,
      Err(e) => {
        rprintln!("BQ4050 temperature: {:?}", e);
      }
    }
    m
  }

  fn draw(screen: &mut Screen, display: &mut St7789, m: &Measurements, seconds: u32) {
    let mut buf = [0u8; LINE];

    let text = format_no_std::show(&mut buf, format_args!("Temp {:>9.2} C", m.pack_temp));
    screen.temp.update(text.unwrap_or("Temp ?"), display).ok();

    let text = format_no_std::show(&mut buf, format_args!("Pack{:>6.2}V{:>7.3}A", m.pack_v, m.pack_a));
    screen.pack.update(text.unwrap_or("Pack ?"), display).ok();

    for (i, field) in screen.ports.iter_mut().enumerate() {
      let text = format_no_std::show(
        &mut buf,
        format_args!("{:<4}{:>6.2}V{:>7.3}A{:>5.1}W", PORT_NAMES[i], m.bus_v[i], m.port_a[i], m.bus_v[i] * m.port_a[i]),
      );
      field.update(text.unwrap_or("?"), display).ok();
    }

    // Battery side: the charger chip's port voltage isn't measured
    let main_a = main_port_a(&Readings { ports_a: m.port_a, pack_a: m.pack_a });
    let text = format_no_std::show(
      &mut buf,
      format_args!("Main{:>14.3}A{:>5.1}W", main_a, m.pack_v * main_a),
    );
    screen.main.update(text.unwrap_or("Main ?"), display).ok();

    let text = format_no_std::show(
      &mut buf,
      format_args!("Up {}:{:02}:{:02}", seconds / 3600, seconds / 60 % 60, seconds % 60),
    );
    screen.uptime.update(text.unwrap_or("Up ?"), display).ok();
  }

  #[task(binds = EXTI0, priority = 2, local = [button])]
  fn button_press(cx: button_press::Context) {
    cx.local.button.clear_interrupt_pending_bit();
    BUTTON.store(true, Ordering::Relaxed);
    rtic::pend(pac::Interrupt::RTC);
  }
}
