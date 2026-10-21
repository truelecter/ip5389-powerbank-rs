//! Two clock profiles: 8 MHz straight from HSI while idle, 48 MHz from the PLL while drawing.
//! At 240 MHz the chip draws about 8 mA more from the battery than at 8 MHz, even sleeping in
//! WFI between interrupts.
//!
//! Peripherals keep the dividers they were set up with, so their clocks follow the profile:
//!
//! - I2C1 (INA3221) and the bit-banged SMBus run only in the idle profile, so set them up for it.
//! - SPI1 (display) runs in the draw profile: set up at idle for 4 MHz, it runs at 24 MHz.
//! - TIM1 (backlight PWM) runs in both; only its frequency changes, not the duty cycle.
//! - The RTC runs from LSI and doesn't depend on either.

use core::sync::atomic::{AtomicU32, Ordering};

use cortex_m::peripheral::DWT;
use embedded_hal::delay::DelayNs;
use stm32f1xx_hal::{
  flash::ACR,
  prelude::*,
  rcc::{self, Rcc},
};

pub const IDLE_HZ: u32 = 8_000_000;
pub const DRAW_HZ: u32 = 48_000_000;

/// Current core clock, for [`CycleDelay`]
static CORE_HZ: AtomicU32 = AtomicU32::new(IDLE_HZ);

/// HSI, no PLL, every bus at 8 MHz
pub fn idle_config() -> rcc::Config {
  rcc::Config::hsi()
}

/// HSI/2 x 12 = 48 MHz, every bus at 48 MHz
fn draw_config() -> rcc::Config {
  rcc::Config::hsi()
    .sysclk(DRAW_HZ.Hz())
    .hclk(DRAW_HZ.Hz())
    .pclk1(DRAW_HZ.Hz())
    .pclk2(DRAW_HZ.Hz())
}

pub struct Clock {
  rcc: Rcc,
  acr: ACR,
}

impl Clock {
  /// Takes the RCC set up with [`idle_config`]
  pub fn new(rcc: Rcc, acr: ACR) -> Self {
    CORE_HZ.store(rcc.clocks.sysclk().raw(), Ordering::Relaxed);
    Self { rcc, acr }
  }

  /// Runs `f` at the draw clock and returns to the idle clock
  pub fn fast<R>(&mut self, f: impl FnOnce() -> R) -> R {
    self.switch(draw_config());
    let result = f();
    self.switch(idle_config());
    result
  }

  fn switch(&mut self, config: rcc::Config) {
    self.rcc.reconfigure(config, &mut self.acr);
    CORE_HZ.store(self.rcc.clocks.sysclk().raw(), Ordering::Relaxed);
  }
}

/// Busy-wait delay on the DWT cycle counter that follows the clock profile. The cycle counter
/// must be enabled.
pub struct CycleDelay;

impl DelayNs for CycleDelay {
  fn delay_ns(&mut self, ns: u32) {
    let hz = CORE_HZ.load(Ordering::Relaxed) as u64;
    let cycles = (ns as u64 * hz).div_ceil(1_000_000_000) as u32;
    let start = DWT::cycle_count();
    while DWT::cycle_count().wrapping_sub(start) < cycles {}
  }
}
