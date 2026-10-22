//! Switches the AT32F403A SRAM size, shared by the `unlock_ram` and `lock_ram` tools.
//!
//! The SRAM size comes from the EOPB0 byte of the user system data (USD, the option bytes):
//! 0xFE selects 224 KB, the factory 0xFF 96 KB. The USD area can only be programmed after an
//! erase, so this backs up every USD byte, erases the area and programs it back with EOPB0
//! changed, following Artery's `flash_user_system_data_erase` and
//! `flash_user_system_data_program`. The new size applies after a reset.
//!
//! Don't power the board off while it runs: until FAP is programmed back (the first write
//! after the erase), the erased USD reads as flash access protection enabled.

use core::ptr::{read_volatile, write_volatile};

use rtt_target::rprintln;

const FLASH_UNLOCK_KEY1: u32 = 0x4567_0123;
const FLASH_UNLOCK_KEY2: u32 = 0xCDEF_89AB;

const FLASH_BASE: usize = 0x4002_2000;
const FLASH_UNLOCK: *mut u32 = (FLASH_BASE + 0x04) as *mut u32;
const FLASH_USD_UNLOCK: *mut u32 = (FLASH_BASE + 0x08) as *mut u32;
const FLASH_STS: *mut u32 = (FLASH_BASE + 0x0C) as *mut u32;
const FLASH_CTRL: *mut u32 = (FLASH_BASE + 0x10) as *mut u32;
const FLASH_USD: *const u32 = (FLASH_BASE + 0x1C) as *const u32;

// FLASH_STS
const STS_OBF: u32 = 1 << 0;
const STS_PRGMERR: u32 = 1 << 2;
const STS_EPPERR: u32 = 1 << 4;
const STS_ODF: u32 = 1 << 5;

// FLASH_CTRL
const CTRL_USDPRGM: u32 = 1 << 4;
const CTRL_USDERS: u32 = 1 << 5;
const CTRL_ERSTR: u32 = 1 << 6;
const CTRL_OPLK: u32 = 1 << 7;
const CTRL_USDULKS: u32 = 1 << 9;

// FLASH_USD
const USD_FAP: u32 = 1 << 1;

/// User system data: 24 halfwords, each a data byte and its complement in the high byte
const USD_BASE: usize = 0x1FFF_F800;
const USD_HALFWORDS: usize = 24;
/// Index of the FAP (flash access protection) halfword
const USD_FAP_INDEX: usize = 0;
/// Index of the EOPB0 halfword
const USD_EOPB0_INDEX: usize = 8;
/// Reserved halfword after EOPB0
const USD_RESERVED_INDEX: usize = 9;

/// FAP value that keeps flash access protection off
const FAP_RELIEVE_KEY: u8 = 0xA5;
/// Value of an erased USD byte
const USD_ERASED: u8 = 0xFF;

/// Polls of FLASH_STS before giving up on an operation. An erase takes milliseconds.
const OPERATION_TIMEOUT: u32 = 10_000_000;

/// EOPB0 values. Each tool binary uses one of them.
#[allow(dead_code)]
#[derive(Clone, Copy)]
#[repr(u8)]
pub enum SramSize {
  Kb96 = 0xFF,
  Kb224 = 0xFE,
}

impl SramSize {
  fn name(self) -> &'static str {
    match self {
      SramSize::Kb96 => "96 KB",
      SramSize::Kb224 => "224 KB",
    }
  }
}

#[derive(Debug)]
enum Error {
  Timeout,
  Program,
  WriteProtected,
}

fn usd_byte(index: usize) -> u8 {
  unsafe { read_volatile((USD_BASE as *const u16).add(index)) as u8 }
}

/// Waits for the running flash operation, then checks and clears its status flags
fn wait_for_operation() -> Result<(), Error> {
  let mut polls = OPERATION_TIMEOUT;
  while unsafe { read_volatile(FLASH_STS) } & STS_OBF != 0 {
    polls -= 1;
    if polls == 0 {
      return Err(Error::Timeout);
    }
  }

  let sts = unsafe { read_volatile(FLASH_STS) };
  unsafe { write_volatile(FLASH_STS, STS_PRGMERR | STS_EPPERR | STS_ODF) };

  if sts & STS_PRGMERR != 0 {
    Err(Error::Program)
  } else if sts & STS_EPPERR != 0 {
    Err(Error::WriteProtected)
  } else {
    Ok(())
  }
}

fn set_ctrl(bits: u32) {
  unsafe { write_volatile(FLASH_CTRL, read_volatile(FLASH_CTRL) | bits) };
}

fn clear_ctrl(bits: u32) {
  unsafe { write_volatile(FLASH_CTRL, read_volatile(FLASH_CTRL) & !bits) };
}

/// Erases the USD area and programs `usd` back into it
fn rewrite_usd(usd: &[u8; USD_HALFWORDS]) -> Result<(), Error> {
  unsafe {
    write_volatile(FLASH_UNLOCK, FLASH_UNLOCK_KEY1);
    write_volatile(FLASH_UNLOCK, FLASH_UNLOCK_KEY2);
    write_volatile(FLASH_USD_UNLOCK, FLASH_UNLOCK_KEY1);
    write_volatile(FLASH_USD_UNLOCK, FLASH_UNLOCK_KEY2);
  }

  let mut polls = OPERATION_TIMEOUT;
  while unsafe { read_volatile(FLASH_CTRL) } & CTRL_USDULKS == 0 {
    polls -= 1;
    if polls == 0 {
      return Err(Error::Timeout);
    }
  }

  set_ctrl(CTRL_USDERS);
  set_ctrl(CTRL_ERSTR);
  let erased = wait_for_operation();
  clear_ctrl(CTRL_USDERS);
  erased?;

  set_ctrl(CTRL_USDPRGM);
  let mut programmed = Ok(());
  for (index, &value) in usd.iter().enumerate() {
    // An erased byte already holds the value, the hardware writes the complement
    if value == USD_ERASED {
      continue;
    }
    unsafe { write_volatile((USD_BASE as *mut u16).add(index), value as u16) };
    programmed = wait_for_operation();
    if programmed.is_err() {
      break;
    }
  }
  clear_ctrl(CTRL_USDPRGM);

  set_ctrl(CTRL_OPLK);
  programmed
}

fn halt() -> ! {
  loop {
    cortex_m::asm::wfi();
  }
}

/// Sets the SRAM size and resets to apply it, or reports that it's already set and halts
pub fn switch_sram(size: SramSize) -> ! {
  let eopb0 = usd_byte(USD_EOPB0_INDEX);
  if eopb0 == size as u8 {
    rprintln!("EOPB0 is {:#04x}: {} SRAM", eopb0, size.name());
    halt();
  }

  rprintln!("EOPB0 is {:#04x}, switching to {} SRAM", eopb0, size.name());

  if unsafe { read_volatile(FLASH_USD) } & USD_FAP != 0 {
    rprintln!("Flash access protection is on, the USD can't be rewritten without a mass erase");
    halt();
  }

  let mut usd = [USD_ERASED; USD_HALFWORDS];
  for (index, byte) in usd.iter_mut().enumerate() {
    *byte = usd_byte(index);
  }
  usd[USD_FAP_INDEX] = FAP_RELIEVE_KEY;
  usd[USD_EOPB0_INDEX] = size as u8;
  usd[USD_RESERVED_INDEX] = USD_ERASED;

  match rewrite_usd(&usd) {
    Ok(()) => {
      rprintln!("Done, resetting");
      cortex_m::peripheral::SCB::sys_reset();
    }
    Err(e) => {
      rprintln!("Rewriting the USD failed: {:?}", e);
      halt();
    }
  }
}
