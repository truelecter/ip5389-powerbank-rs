//! Saving a measured capacity to the gauge's data flash.

use embedded_hal::delay::DelayNs;
use embedded_hal::i2c::I2c;

use peripherals::bq4050::{operation, BQ4050};

/// Data flash: Design Capacity mAh, followed by Design Capacity cWh (TRM table 14-1)
const DESIGN_CAPACITY: u16 = 0x444d;
/// Data flash: Learned Full Charge Capacity, mAh
const LEARNED_CAPACITY: u16 = 0x4100;
/// MAC DeviceReset(): the gauge reloads the capacities from data flash
const DEVICE_RESET: u16 = 0x0041;
/// Measured capacities outside this range are not saved
const SANE_MAH: core::ops::RangeInclusive<u16> = 1000..=20000;

/// Writes `mah` as the learned and the design capacity, scaling the design energy the same
/// way, checks both by reading them back and resets the gauge so it uses them
pub fn save<I: I2c>(gauge: &mut BQ4050<I>, mah: u16, delay: &mut impl DelayNs) -> Result<(), &'static str> {
  if !SANE_MAH.contains(&mah) {
    return Err("capacity out of range");
  }
  let status = retry(delay, || gauge.get_operation_status()).map_err(|_| "gauge not answering")?;
  if (status >> operation::SEC_SHIFT) & operation::SEC_MASK == operation::SEALED {
    return Err("gauge sealed");
  }

  let mut row = [0u8; 32];
  retry(delay, || gauge.read_data_flash(DESIGN_CAPACITY, &mut row)).map_err(|_| "design read failed")?;
  let old_mah = u16::from_le_bytes([row[0], row[1]]);
  let old_cwh = u16::from_le_bytes([row[2], row[3]]);
  let cwh = if old_mah == 0 { mah as u32 * 144 / 100 } else { old_cwh as u32 * mah as u32 / old_mah as u32 } as u16;

  let [m0, m1] = mah.to_le_bytes();
  let [c0, c1] = cwh.to_le_bytes();
  write_checked(gauge, delay, DESIGN_CAPACITY, &[m0, m1, c0, c1]).map_err(|_| "design write failed")?;
  write_checked(gauge, delay, LEARNED_CAPACITY, &[m0, m1]).map_err(|_| "learned write failed")?;
  retry(delay, || gauge.mac_command(DEVICE_RESET)).map_err(|_| "reset failed")
}

/// Writes `data` at `address` and reads it back
fn write_checked<I: I2c>(gauge: &mut BQ4050<I>, delay: &mut impl DelayNs, address: u16, data: &[u8]) -> Result<(), ()> {
  retry(delay, || gauge.write_data_flash(address, data)).map_err(|_| ())?;
  // Page erase and programming take up to 40 ms
  delay.delay_ms(100);
  let mut row = [0u8; 32];
  retry(delay, || gauge.read_data_flash(address, &mut row)).map_err(|_| ())?;
  if &row[..data.len()] == data {
    Ok(())
  } else {
    Err(())
  }
}

/// The gauge NACKs while it writes its data flash: try up to five times, 50 ms apart
fn retry<T, E>(delay: &mut impl DelayNs, mut f: impl FnMut() -> Result<T, E>) -> Result<T, E> {
  let mut result = f();
  for _ in 0..4 {
    if result.is_ok() {
      break;
    }
    delay.delay_ms(50);
    result = f();
  }
  result
}
