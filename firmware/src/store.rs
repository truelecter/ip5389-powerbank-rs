//! Settings that survive power-off, in the last 2 KB page of the first flash bank. The
//! firmware ends far below it, and `memory.x` keeps the linker out of it.

use stm32f1xx_hal::pac;

/// The page: bank 1 ends at 512 KB, its pages are 2 KB
const PAGE: u32 = 0x0807_F800;
/// Marks a written page
const MAGIC: u16 = 0xA55A;
const KEY1: u32 = 0x4567_0123;
const KEY2: u32 = 0xCDEF_89AB;

/// The stored value, `None` while the page holds none
pub fn load() -> Option<u16> {
  // SAFETY: reads of the flash page
  let (magic, value) = unsafe { (core::ptr::read_volatile(PAGE as *const u16), core::ptr::read_volatile((PAGE + 2) as *const u16)) };
  (magic == MAGIC).then_some(value)
}

/// Erases the page and stores `value`. Stalls the core for the erase, some tens of ms.
pub fn save(value: u16) -> Result<(), ()> {
  // SAFETY: the flash controller of bank 1, used by nothing else; only the settings page
  // is erased and written
  let flash = unsafe { &*pac::FLASH::ptr() };
  let wait = || while flash.sr().read().bsy().bit_is_set() {};
  wait();
  flash.keyr().write(|w| unsafe { w.key().bits(KEY1) });
  flash.keyr().write(|w| unsafe { w.key().bits(KEY2) });

  flash.cr().modify(|_, w| w.per().set_bit());
  flash.ar().write(|w| unsafe { w.far().bits(PAGE) });
  flash.cr().modify(|_, w| w.strt().set_bit());
  cortex_m::asm::nop();
  wait();
  flash.cr().modify(|_, w| w.per().clear_bit());

  for (offset, half_word) in [(0, MAGIC), (2, value)] {
    flash.cr().modify(|_, w| w.pg().set_bit());
    // SAFETY: a half-word of the erased page
    unsafe { core::ptr::write_volatile((PAGE + offset) as *mut u16, half_word) };
    wait();
    flash.cr().modify(|_, w| w.pg().clear_bit());
  }
  let errors = flash.sr().read();
  let failed = errors.pgerr().bit_is_set() || errors.wrprterr().bit_is_set();
  // Clear the error and end-of-operation flags by writing ones
  flash.sr().write(|w| w.pgerr().bit(true).wrprterr().bit(true).eop().bit(true));
  flash.cr().modify(|_, w| w.lock().set_bit());

  if failed || load() != Some(value) {
    Err(())
  } else {
    Ok(())
  }
}
