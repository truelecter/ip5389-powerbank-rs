//! One-shot tool that switches the AT32F403A back to the factory 96 KB of SRAM, see `usd.rs`.
//! The extra 128 KB that `unlock_ram` enables draw more power.

#![no_std]
#![no_main]

use panic_rtt_target as _;
use rtt_target::rtt_init_print;

use stm32f1xx_hal as _;

mod usd;

#[cortex_m_rt::entry]
fn main() -> ! {
  rtt_init_print!();
  usd::switch_sram(usd::SramSize::Kb96)
}
