//! The firmware shared by every layout: measures once a second, decides the screen level and
//! hands the readings to the layout. Each binary picks the layout as `crate::Screen`.

use core::fmt::Debug;
use core::sync::atomic::{AtomicU8, Ordering};

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

use crate::clock::{self, Clock, CycleDelay};
use crate::{capacity, store};

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
/// Backlight duty cycle in percent. Measured from the battery: about 6 mA at 50 %.
const BACKLIGHT_BRIGHT: u8 = 50;
const BACKLIGHT_DIM: u8 = 10;
/// BatteryMode() bit: capacities in 10 mWh instead of mAh
const CAPM: u16 = 1 << 15;
/// The gauge's "no estimate" time
const NO_TIME: u16 = 0xFFFF;

// Display st7789(v?)
use embedded_hal::pwm::SetDutyCycle;
use embedded_hal_bus::spi::{ExclusiveDevice, NoDelay};
use graphics::layouts::Layout;
use graphics::battery::{self, Action, BatteryPage, Details};
use graphics::{Snapshot, PORT_NAMES};
use mipidsi::{
  interface::SpiInterface,
  models::ST7789,
  options::{ColorInversion, ColorOrder, Orientation},
  Builder, Display,
};

use peripherals::bq4050::{gauging, BQ4050};
use peripherals::ina3221::{self, INA3221};
use peripherals::soft_i2c::{self, SoftI2c};

use power::button::{Button, Press};
use power::auto_off::{self, AutoOff};
use power::learning::{GaugeState, Measurement, Phase};
use power::{main_port_a, Level, Readings, ScreenPower};

/// Button events since the last update, set by the button interrupt: [`DOWN`], [`SHORT`],
/// [`LONG`]
static BUTTON: AtomicU8 = AtomicU8::new(0);
/// The button went down: wakes the screen right away
const DOWN: u8 = 1;
/// A press ended, by its length
const SHORT: u8 = 2;
const LONG: u8 = 4;
/// RTC prescaler for 1 Hz from the 40 kHz LSI
const RTC_DIVIDER: u32 = 40_000;

/// What the display shows
#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
  /// The firmware's layout
  Main,
  /// Battery details and capacity measurement, opened by a short press
  Battery,
}

/// Switches the board off while it runs on battery; with a charger connected it stays on until
/// the charger goes away. PB4 must otherwise stay as after reset, JTAG's NJTRST: on battery
/// the board went dark both when PB4 got the GPIO pull-up and when it was driven low. What
/// the pin controls on the board is not known yet.
fn power_off() {
  // SAFETY: GPIOB pin 4 only, and AFIO SWJ_CFG; SWD stays enabled
  unsafe {
    let gpiob = &*pac::GPIOB::ptr();
    gpiob.bsrr().write(|w| w.bits(1 << (4 + 16)));
    gpiob.crl().modify(|r, w| w.bits((r.bits() & !(0xf << 16)) | (0b0010 << 16)));
    // JTAG without NJTRST: PB4 becomes a GPIO
    let afio = &*pac::AFIO::ptr();
    afio.mapr().modify(|r, w| w.bits((r.bits() & !(0b111 << 24)) | (0b001 << 24)));
  }
}

/// Milliseconds from the RTC counter and its prescaler divider: the same clock as the 1 Hz
/// tick, independent of the clock profile. Wraps after 49 days.
fn rtc_millis() -> u32 {
  // SAFETY: read-only access to the RTC counter registers
  let rtc = unsafe { &*pac::RTC::ptr() };
  loop {
    let low = rtc.cntl().read().bits();
    let divider = (rtc.divh().read().bits() << 16) | rtc.divl().read().bits();
    let high = rtc.cnth().read().bits();
    // Read again if the second ticked meanwhile
    if rtc.cntl().read().bits() == low {
      let seconds = (high << 16) | low;
      let fraction = (RTC_DIVIDER - 1).saturating_sub(divider) * 1000 / RTC_DIVIDER;
      return seconds.wrapping_mul(1000).wrapping_add(fraction);
    }
  }
}

/// Runs before RAM is initialized. Replaces the deprecated `#[cortex_m_rt::pre_init]`.
#[unsafe(export_name = "__pre_init")]
unsafe extern "C" fn preinit() {
  // TODO - disable baclkight
}

type Gauge = BQ4050<SoftI2c<Pin<'B', 10, Output<OpenDrain>>, Pin<'B', 11, Output<OpenDrain>>, CycleDelay>>;

/// The value of a reading, or `None` after logging why it failed
fn logged<T, E: Debug>(what: &str, result: Result<T, E>) -> Option<T> {
  if let Err(e) = &result {
    rprintln!("{}: {:?}", what, e);
  }
  result.ok()
}

#[app(device = stm32f1xx_hal::pac, peripherals = true)]
mod firmware {
  use super::*;

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
    bq4050: Gauge,
    /// The gauge reports capacities in 10 mWh, not mAh
    capacity_in_energy: bool,
    ina3221: INA3221<BlockingI2c<I2C1>>,
    display: St7789,
    backlight: PwmChannel<TIM1, 0>,
    screen: crate::Screen,
    /// The battery page
    battery: BatteryPage,
    page: Page,
    measurement: Measurement,
    details: Details,
    /// The latest readings. A failed read keeps the value from the update before.
    snapshot: Snapshot,
    power: ScreenPower,
    seconds: u32,
    level: Level,
    auto_off: AutoOff,
    /// Automatic power-off timeout, `None` when off; kept in flash
    auto_off_s: Option<u32>,
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
    let display = Builder::new(ST7789, di)
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
    let mut bq4050 = BQ4050::new(smbus);
    let mode = logged("BQ4050 battery mode", bq4050.get_battery_mode()).unwrap_or(0);
    let capacity_in_energy = mode & CAPM != 0;

    let auto_off_s = store::load().and_then(|i| auto_off::CHOICES.get(i as usize).copied()).flatten();
    rprintln!("auto off after {:?} s", auto_off_s);
    rprintln!("BQ4050 battery mode {:#06x}", mode);

    // 1 Hz tick from LSI, independent of the clock profile
    let mut bkp = cx.device.BKP.constrain(&mut cx.device.PWR, &mut rcc);
    let mut rtc = Rtc::new_lsi(cx.device.RTC, &mut bkp);
    rtc.listen_seconds();

    let mut button = gpioa.pa0.into_pull_down_input(&mut gpioa.crl);
    button.make_interrupt_source(&mut afio);
    button.enable_interrupt(&mut cx.device.EXTI);
    button.trigger_on_edge(&mut cx.device.EXTI, Edge::RisingFalling);

    let clock = Clock::new(rcc, flash.acr);
    rprintln!("init finished");
    // The first update draws the screen and lights the backlight
    BUTTON.store(DOWN, Ordering::Relaxed);
    rtic::pend(pac::Interrupt::RTC);

    (
      Shared {},
      Local {
        clock,
        rtc,
        bq4050,
        capacity_in_energy,
        ina3221,
        display,
        backlight,
        screen: crate::Screen::new(),
        battery: BatteryPage::new(),
        page: Page::Main,
        measurement: Measurement::new(),
        details: Details::default(),
        snapshot: Snapshot::default(),
        power: ScreenPower::new(),
        seconds: 0,
        level: Level::Off,
        auto_off: AutoOff::new(),
        auto_off_s,
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

  /// Once a second, and right after a button event: measure, decide the screen level, act on
  /// a press, redraw
  #[task(
    binds = RTC,
    local = [
      clock, rtc, bq4050, capacity_in_energy, ina3221, display, backlight, screen, battery, page,
      measurement, details, snapshot, power, seconds, level, auto_off, auto_off_s,
      // The press started on a bright screen: on a dim or dark one it only wakes it
      press_on_bright: bool = false,
    ]
  )]
  fn update(cx: update::Context) {
    // SAFETY: read-only access to the RTC flags; the second flag is cleared through the HAL
    let tick = unsafe { (*pac::RTC::ptr()).crl().read().secf().bit_is_set() };
    if tick {
      cx.local.rtc.clear_second_flag();
      *cx.local.seconds += 1;
    }
    let seconds = *cx.local.seconds;
    let events = BUTTON.swap(0, Ordering::Relaxed);
    if events & DOWN != 0 {
      *cx.local.press_on_bright = *cx.local.level == Level::Bright;
    }
    let press = match events {
      e if e & LONG != 0 => Some(Press::Long),
      e if e & SHORT != 0 => Some(Press::Short),
      _ => None,
    };

    let (s, details) = (cx.local.snapshot, cx.local.details);
    measure(s, details, cx.local.ina3221, cx.local.bq4050, *cx.local.capacity_in_energy);
    s.uptime_s = seconds;

    let measurement = cx.local.measurement;
    let before = measurement.phase();
    let gauge = GaugeState {
      full: details.gauging & gauging::FC != 0,
      empty: details.gauging & gauging::EDV0 != 0,
      pack_a: s.pack_amps,
    };
    measurement.update(&gauge, tick as u32);
    if measurement.phase() != before {
      rprintln!("{}s: measurement {:?} -> {:?}", seconds, before, measurement.phase());
    }
    s.status = battery::status(measurement.phase(), details.gauging);

    let readings = Readings { ports_a: s.ports.map(|port| port.amps), pack_a: s.pack_amps };
    s.main_amps = main_port_a(&readings);
    let level = cx.local.power.update(&readings, events != 0, tick as u32);
    if level != *cx.local.level {
      // The pack current still shows the previous level: the gauge averages over a second
      rprintln!("{}s: screen {:?} -> {:?}, pack {} mA", seconds, cx.local.level, level, (s.pack_amps * 1000.0) as i32);
      *cx.local.level = level;
    }

    // A measurement lives in RAM: never switch off during one
    let busy = !matches!(measurement.phase(), Phase::Idle | Phase::Interrupted);
    if cx.local.auto_off.update(&readings, events != 0, busy, tick as u32, *cx.local.auto_off_s) {
      rprintln!("{}s: auto off", seconds);
      power_off();
    }

    let (page, screen, battery) = (cx.local.page, cx.local.screen, cx.local.battery);
    if let (Some(press), true) = (press, *cx.local.press_on_bright) {
      match *page {
        Page::Main if press == Press::Short => {
          *page = Page::Battery;
          *battery = BatteryPage::new();
        }
        Page::Main => {}
        Page::Battery => match battery.press(press, measurement.phase()) {
          Some(Action::StartMeasuring) => {
            measurement.start();
            battery.set_message("");
          }
          Some(Action::StopMeasuring) => measurement.stop(),
          Some(Action::Save(mah)) => {
            let message = match capacity::save(cx.local.bq4050, mah, &mut CycleDelay) {
              Ok(()) => {
                measurement.stop();
                "Saved, gauge reset"
              }
              Err(e) => e,
            };
            rprintln!("{}s: save {} mAh: {}", seconds, mah, message);
            battery.set_message(message);
          }
          Some(Action::PowerOff) => {
            rprintln!("{}s: power off", seconds);
            battery.set_message("Off: unplug the charger");
            power_off();
          }
          Some(Action::AutoOff) => {
            let next = auto_off::next_choice(*cx.local.auto_off_s);
            let index = auto_off::CHOICES.iter().position(|&c| c == next).unwrap_or(0);
            let saved = store::save(index as u16).is_ok();
            rprintln!("{}s: auto off after {:?} s, saved: {}", seconds, next, saved);
            *cx.local.auto_off_s = next;
            battery.set_message(if saved { "" } else { "Setting not saved" });
          }
          Some(Action::GaugeShutdown) => {
            let sent = logged("BQ4050 shutdown", cx.local.bq4050.shutdown()).is_some();
            rprintln!("{}s: gauge shutdown sent: {}", seconds, sent);
            battery.set_message(if sent { "Gauge shutdown sent" } else { "Gauge shutdown failed" });
          }
          Some(Action::Back) => {
            *page = Page::Main;
            *screen = crate::Screen::new();
          }
          None => {}
        },
      }
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
      // The next wake shows the main layout again
      if *page == Page::Battery {
        *page = Page::Main;
        *screen = crate::Screen::new();
      }
      return;
    }
    if display.is_sleeping() {
      // The panel keeps its frame memory while asleep, so the layout stays valid
      display.wake(&mut CycleDelay).ok();
    }

    match *page {
      Page::Main => {
        cx.local.clock.fast(|| screen.update(s, display)).ok();
      }
      Page::Battery => {
        measure_details(details, cx.local.bq4050, seconds);
        let phase = measurement.phase();
        let auto_off_s = *cx.local.auto_off_s;
        cx.local.clock.fast(|| battery.update(s, details, phase, auto_off_s, display)).ok();
      }
    }
    // After drawing: the first update replaces whatever the panel showed at power-on
    cx.local.backlight.set_duty_cycle_percent(duty).ok();
  }

  /// Gauge values only the battery page shows
  fn measure_details(d: &mut Details, bq4050: &mut Gauge, seconds: u32) {
    if let Some(mah) = logged("BQ4050 full charge capacity", bq4050.get_full_charge_capacity()) {
      d.full_mah = mah;
    }
    if let Some(mah) = logged("BQ4050 design capacity", bq4050.get_design_capacity()) {
      d.design_mah = mah;
    }
    if let Some(percents) = logged("BQ4050 max error", bq4050.get_max_error()) {
      d.max_error = percents;
    }
    if let Some(status) = logged("BQ4050 operation status", bq4050.get_operation_status()) {
      if status != d.operation {
        rprintln!("{}s: OperationStatus {:#010x}", seconds, status);
      }
      d.operation = status;
    }
  }

  /// Updates `s`, and the gauge values `d` needs every update, with every reading that succeeds
  fn measure(s: &mut Snapshot, d: &mut Details, ina3221: &mut INA3221<BlockingI2c<I2C1>>, bq4050: &mut Gauge, capacity_in_energy: bool) {
    let channels = [ina3221::Channel::Ch1, ina3221::Channel::Ch2, ina3221::Channel::Ch3];
    for (i, channel) in channels.into_iter().enumerate() {
      match (ina3221.bus_voltage(channel), ina3221.shunt_voltage(channel)) {
        (Ok(bus), Ok(shunt)) => {
          s.ports[i].volts = bus;
          s.ports[i].amps = (shunt - SHUNT_OFFSET_PER_BUS_VOLT[i] * bus) / SHUNT_OHMS[i];
        }
        (Err(e), _) | (_, Err(e)) => {
          rprintln!("INA3221 {}: {:?}", PORT_NAMES[i], e);
        }
      }
    }

    if let Some(mv) = logged("BQ4050 voltage", bq4050.get_voltage()) {
      s.pack_volts = mv as f32 / 1000.0;
    }
    if let Some(ma) = logged("BQ4050 current", bq4050.get_current()) {
      s.pack_amps = ma as f32 / 1000.0;
    }
    if let Some(temp) = logged("BQ4050 temperature", bq4050.get_temperature()) {
      s.pack_temp = temp;
    }
    if let Some(status) = logged("BQ4050 DAStatus2", bq4050.get_da_status_2()) {
      s.gauge_temp = *status.int_temperature as f32 / 10.0 - 273.15;
    }
    let cells = [
      bq4050.get_cell_voltage_1(),
      bq4050.get_cell_voltage_2(),
      bq4050.get_cell_voltage_3(),
      bq4050.get_cell_voltage_4(),
    ];
    for (volts, mv) in s.cell_volts.iter_mut().zip(cells) {
      if let Some(mv) = logged("BQ4050 cell voltage", mv) {
        *volts = mv as f32 / 1000.0;
      }
    }
    if let Some(percents) = logged("BQ4050 state of charge", bq4050.get_relative_state_of_charge()) {
      s.charge = percents.min(100) as u8;
    }
    if let Some(percents) = logged("BQ4050 state of health", bq4050.get_soh()) {
      s.health = (percents & 0xFF).min(100) as u8;
    }
    if let Some(cycles) = logged("BQ4050 cycle count", bq4050.get_cycle_count()) {
      s.cycles = cycles;
    }
    if let Some(capacity) = logged("BQ4050 remaining capacity", bq4050.get_remaining_capacity()) {
      d.remaining_mah = capacity;
      s.remaining_wh = if capacity_in_energy {
        capacity as f32 / 100.0
      } else {
        capacity as f32 * s.pack_volts / 1000.0
      };
    }
    if let Some(flags) = logged("BQ4050 gauging status", bq4050.get_gauging_status()) {
      d.gauging = flags;
    }
    if s.pack_amps == 0.0 {
      s.minutes_left = None;
    } else {
      let minutes = if s.pack_amps < 0.0 {
        logged("BQ4050 time to empty", bq4050.get_run_time_to_empty())
      } else {
        logged("BQ4050 time to full", bq4050.get_average_time_to_full())
      };
      if let Some(minutes) = minutes {
        s.minutes_left = Some(minutes).filter(|&m| m != NO_TIME);
      }
    }
  }

  /// Both edges of the button: a press wakes the screen right away, its release tells short
  /// from long
  #[task(binds = EXTI0, priority = 2, local = [button, debounce: Button = Button::new()])]
  fn button_edge(cx: button_edge::Context) {
    cx.local.button.clear_interrupt_pending_bit();
    let down = cx.local.button.is_high();
    let event = match cx.local.debounce.edge(down, rtc_millis()) {
      _ if down => DOWN,
      Some(Press::Short) => SHORT,
      Some(Press::Long) => LONG,
      None => return,
    };
    BUTTON.fetch_or(event, Ordering::Relaxed);
    rtic::pend(pac::Interrupt::RTC);
  }
}
