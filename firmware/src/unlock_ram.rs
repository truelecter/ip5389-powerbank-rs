//! One-shot tool that switches the AT32F403A to 224 KB of SRAM, see `usd.rs`.
//! `lock_ram` switches back to 96 KB, which draws less power.

#![no_std]
#![no_main]

use panic_rtt_target as _;
use rtt_target::rtt_init_print;

use stm32f1xx_hal as _;

mod usd;

#[cortex_m_rt::entry]
fn main() -> ! {
  rtt_init_print!();
  usd::switch_sram(usd::SramSize::Kb224)
}
