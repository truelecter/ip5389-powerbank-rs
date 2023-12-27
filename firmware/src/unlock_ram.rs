#![no_std]
#![no_main]

use core::ptr::{write_volatile, read_volatile};

use panic_rtt_target as _;
use rtt_target::{rprintln, rtt_init_print};

use stm32f1xx_hal as _;
// use stm32f1xx_hal::{
//     pac::Peripherals,
//     prelude::*,
// };

const FLASH_UNLOCK_KEY1: u32 = 0x4567_0123;
const FLASH_UNLOCK_KEY2: u32 = 0xCDEF_89AB;

const PERIPH_BASE: u32 = 0x4000_0000;
const AHBPERIPH_BASE: u32 = PERIPH_BASE + 0x2_0000;
const FLASH_REG_BASE: u32 = AHBPERIPH_BASE + 0x2000;

// 0x4002_2004
// 0x4002_2044

// 0x4002_2008

const FLASH_USD_UNLOCK: *mut u32 = (FLASH_REG_BASE + 0x08) as *mut u32;
const FLASH_UNLOCK: *mut u32 = (FLASH_REG_BASE + 0x04) as *mut u32;
const FLASH_UNLOCK2: *mut u32 = (FLASH_REG_BASE + 0x44) as *mut u32;
const FLASH_CTRL: *mut u32 = (FLASH_REG_BASE + 0x10) as *mut u32;
const FLASH_STS: *mut u32 = (FLASH_REG_BASE + 0x0C) as *mut u32;

const F: *mut u32 = 0x1FFF_F810 as *mut u32;

#[cortex_m_rt::pre_init]
unsafe fn before_main() {
  if read_volatile(F) & 0xFF == 0xFE {
    return;
  }

  write_volatile(FLASH_UNLOCK, FLASH_UNLOCK_KEY1);
  write_volatile(FLASH_UNLOCK, FLASH_UNLOCK_KEY2);

  write_volatile(FLASH_UNLOCK2, FLASH_UNLOCK_KEY1);
  write_volatile(FLASH_UNLOCK2, FLASH_UNLOCK_KEY2);

  write_volatile(FLASH_USD_UNLOCK, FLASH_UNLOCK_KEY1);
  write_volatile(FLASH_USD_UNLOCK, FLASH_UNLOCK_KEY2);


  // write_volatile(FLASH_CTRL, read_volatile(FLASH_CTRL) | 0b0000_0011_0000);

  loop {
    let a = read_volatile(FLASH_CTRL) & 0b0010_0000_0000;

    if a > 0 {
      break;
    }
  }

  write_volatile(FLASH_CTRL, read_volatile(FLASH_CTRL) | 0b0000_0000_1000);

  write_volatile(F, read_volatile(F) | 0xFE);

  let f_res = read_volatile(FLASH_STS);

  let mut counter:u32 = 1_000_000;

  loop {
    counter = counter - 1;
    if counter == 0 {
      break;
    }
  }

  write_volatile(FLASH_CTRL, read_volatile(FLASH_CTRL) & 0xFFFFFFF7);

  if f_res & 0b0011_0101 == 0 {
    cortex_m::peripheral::SCB::sys_reset();
  }
}

// usd_unlock Offset 0x08
#[cortex_m_rt::entry]
fn main() -> ! {
  rtt_init_print!();
  rprintln!("Hello, worl!");

  // let cp = cortex_m::Peripherals::take().unwrap();
  // let dp = Peripherals::take().unwrap();

  // let mut flash = dp.FLASH.constrain();
  // let rcc = dp.RCC.constrain();

  // let clocks = rcc.cfgr
  //     .hclk(72.MHz())
  //     .sysclk(72.MHz())
  //     .freeze(&mut flash.acr);

  // let mut delay = cp.SYST.delay(&clocks);

  unsafe {
    let f_res = read_volatile(F);

    if f_res & 0xFF == 0xFE {
      rprintln!("OK");

    } else {
      rprintln!("NOK, {:b}", f_res);
    }
  }

  loop { }
}
