//! Survey of the MCU pins the firmware doesn't use: which ones something on the board drives.
//!
//! Every pin is read floating, then with the internal pull-up, then with the pull-down
//! (about 40 kΩ), and left floating again, as after reset. A pin that reads the same with
//! both pulls is driven, or pulled by a resistor stronger than the internal one: it is
//! connected to something. A pin that follows the pulls is unconnected, or goes to a
//! high-impedance input. Pins never drive anything.
//!
//! PB4 is left out: on battery the whole board goes dark when it gets the GPIO pull-up, and
//! when it is driven low. It reads low with either pull. Left as after reset (JTAG's NJTRST)
//! or floating, nothing happens; with a charger connected nothing happens either.
//!
//! Found on battery, charger unplugged: PA6, PB2 (BOOT1) driven low; PB12, PB13 driven
//! high; every other pin follows the pulls, and none of them changes the board's power.
//!
//! Prints a table over RTT, then a line whenever a pin's class changes, twice a second:
//! plug and unplug things meanwhile to see which pins follow them.

#![no_std]
#![no_main]

// Only the idle profile and `CycleDelay`
#[allow(dead_code)]
mod clock;

use embedded_hal::delay::DelayNs;
use panic_rtt_target as _;
use rtt_target::{rprintln, rtt_init_print};

use stm32f1xx_hal::{pac, prelude::*};

use clock::CycleDelay;

/// Pins of the 48-pin package the firmware leaves alone: not the display, the button, the
/// backlight, the two I2C buses or SWD
const PINS: &[(usize, u8)] = &[
  (0, 4),
  (0, 6),
  (0, 9),
  (0, 10),
  (0, 11),
  (0, 12),
  (0, 15),
  (1, 0),
  (1, 1),
  (1, 2),
  (1, 3),
  // PB4 turns the power off: a weak pull-up is enough
  (1, 5),
  (1, 8),
  (1, 9),
  (1, 12),
  (1, 13),
  (1, 14),
  (1, 15),
  (2, 13),
  (2, 14),
  (2, 15),
  (3, 0),
  (3, 1),
];
const PORT_NAMES: [char; 4] = ['A', 'B', 'C', 'D'];

/// CNF/MODE nibbles
const FLOATING: u32 = 0b0100;
const PULLED: u32 = 0b1000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
  /// Reads high with either pull
  High,
  /// Reads low with either pull
  Low,
  /// Follows the pulls
  Open,
  /// High with the pull-down and low with the pull-up: shouldn't happen
  Odd,
}

impl Class {
  fn name(self) -> &'static str {
    match self {
      Class::High => "driven high",
      Class::Low => "driven low",
      Class::Open => "follows pulls",
      Class::Odd => "odd",
    }
  }
}

fn port(index: usize) -> &'static pac::gpioa::RegisterBlock {
  // SAFETY: the ports' register blocks; the HAL parts that own them are dropped
  unsafe {
    match index {
      0 => &*pac::GPIOA::ptr(),
      1 => &*pac::GPIOB::ptr(),
      2 => &*pac::GPIOC::ptr(),
      _ => &*pac::GPIOD::ptr(),
    }
  }
}

fn set_mode(index: usize, pin: u8, nibble: u32) {
  let gpio = port(index);
  let shift = (pin as u32 % 8) * 4;
  // SAFETY: changes only this pin's nibble
  if pin < 8 {
    gpio.crl().modify(|r, w| unsafe { w.bits((r.bits() & !(0xf << shift)) | (nibble << shift)) });
  } else {
    gpio.crh().modify(|r, w| unsafe { w.bits((r.bits() & !(0xf << shift)) | (nibble << shift)) });
  }
}

fn read(index: usize, pin: u8) -> bool {
  port(index).idr().read().bits() & (1 << pin) != 0
}

/// Floating level and class of one pin; leaves it floating
fn probe(index: usize, pin: u8) -> (bool, Class) {
  let mut delay = CycleDelay;
  let floating = read(index, pin);
  // ODR selects the pull: 1 up, 0 down. BSRR sets or resets one bit.
  let gpio = port(index);
  gpio.bsrr().write(|w| unsafe { w.bits(1 << pin) });
  set_mode(index, pin, PULLED);
  delay.delay_ms(2);
  let up = read(index, pin);
  gpio.bsrr().write(|w| unsafe { w.bits(1 << (pin + 16)) });
  delay.delay_ms(2);
  let down = read(index, pin);
  set_mode(index, pin, FLOATING);
  let class = match (up, down) {
    (true, true) => Class::High,
    (false, false) => Class::Low,
    (true, false) => Class::Open,
    (false, true) => Class::Odd,
  };
  (floating, class)
}

#[cortex_m_rt::entry]
fn main() -> ! {
  let mut cp = cortex_m::Peripherals::take().unwrap();
  let dp = pac::Peripherals::take().unwrap();
  let mut flash = dp.FLASH.constrain();
  let mut rcc = dp.RCC.freeze(clock::idle_config(), &mut flash.acr);
  cp.DCB.enable_trace();
  cp.DWT.enable_cycle_counter();
  rtt_init_print!(rtt_target::ChannelMode::NoBlockSkip, 4096);

  // Clocks the ports
  let _afio = dp.AFIO.constrain(&mut rcc);
  let _ = dp.GPIOA.split(&mut rcc);
  let _ = dp.GPIOB.split(&mut rcc);
  let _ = dp.GPIOC.split(&mut rcc);
  let _ = dp.GPIOD.split(&mut rcc);
  // PA15 and PB3 leave JTAG, SWD stays; PD0 and PD1 (HSE) become GPIO
  // SAFETY: AFIO MAPR, SWJ_CFG (bits 24-26) and PD01_REMAP (bit 15)
  unsafe { (*pac::AFIO::ptr()).mapr().write(|w| w.bits(0b010 << 24 | 1 << 15)) };

  if HUNT {
    hunt();
  }

  let mut classes = [Class::Open; PINS.len()];
  rprintln!("pin  floating  pulls");
  for (i, &(index, pin)) in PINS.iter().enumerate() {
    let (floating, class) = probe(index, pin);
    classes[i] = class;
    rprintln!("P{}{:<2}  {}         {}", PORT_NAMES[index], pin, floating as u8, class.name());
  }

  let mut delay = CycleDelay;
  let mut half_seconds = 0u32;
  rprintln!("watching");
  loop {
    delay.delay_ms(500);
    half_seconds += 1;
    for (i, &(index, pin)) in PINS.iter().enumerate() {
      let (_, class) = probe(index, pin);
      if class != classes[i] {
        let (s, tenths) = (half_seconds / 2, (half_seconds % 2) * 5);
        rprintln!("{}.{}s P{}{}: {} -> {}", s, tenths, PORT_NAMES[index], pin, classes[i].name(), class.name());
        classes[i] = class;
      }
    }
  }
}

/// Finding a pin that cuts the power, on battery: each pull is announced in the log 4 s before
/// it is applied for 0.1 s, so when the board goes dark the last line read names the pin.
/// Read the log continuously from the host meanwhile.
const HUNT: bool = false;

fn hunt() -> ! {
  let mut delay = CycleDelay;
  loop {
    for &(index, pin) in PINS {
      for (up, name) in [(true, "up"), (false, "down")] {
        rprintln!("next P{}{} pull-{}", PORT_NAMES[index], pin, name);
        delay.delay_ms(4000);
        let bit = if up { 1 << pin } else { 1 << (pin + 16) };
        // SAFETY: BSRR sets or resets this pin's ODR bit only, which picks its pull
        port(index).bsrr().write(|w| unsafe { w.bits(bit) });
        set_mode(index, pin, PULLED);
        delay.delay_ms(100);
        let level = read(index, pin);
        set_mode(index, pin, FLOATING);
        rprintln!("  survived, read {}", level as u8);
      }
    }
  }
}
