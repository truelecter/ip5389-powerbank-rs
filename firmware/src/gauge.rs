//! Read-only report of the BQ4050 state that decides capacity learning: security mode, status
//! flags, capacities and the data flash settings of TRM chapter 6 (CEDV gauging). Prints over
//! RTT once and then sleeps.

#![no_std]
#![no_main]

// Only the idle profile and `CycleDelay`
#[allow(dead_code)]
mod clock;

use panic_rtt_target as _;
use rtt_target::{rprintln, rtt_init_print};

use stm32f1xx_hal::{gpio::PinState, pac, prelude::*};

use clock::CycleDelay;
use peripherals::bq4050::BQ4050;
use peripherals::soft_i2c::SoftI2c;

/// How to show a data flash value
#[derive(Clone, Copy)]
enum Kind {
  I2,
  U1,
  U2,
  H1,
  H2,
}

/// Data flash values that matter for learning, from TRM SLUUAQ3A table 14-1
const SETTINGS: &[(&str, u16, Kind)] = &[
  ("Learned Full Charge Capacity, mAh", 0x4100, Kind::I2),
  ("Cycle Count", 0x4140, Kind::U2),
  ("Mfg Status Init", 0x4340, Kind::H2),
  ("PF Fuse A", 0x4400, Kind::H1),
  ("PF Fuse B", 0x4401, Kind::H1),
  ("PF Fuse C", 0x4402, Kind::H1),
  ("PF Fuse D", 0x4403, Kind::H1),
  ("SBS Gauging Configuration", 0x4408, Kind::H1),
  ("Power Valid Update Voltage, mV", 0x4413, Kind::I2),
  ("Design Capacity, mAh", 0x444d, Kind::I2),
  ("Design Capacity, cWh", 0x444f, Kind::I2),
  ("Design Voltage, mV", 0x4451, Kind::I2),
  ("Cycle Count Percentage", 0x4453, Kind::U1),
  ("SOC Flag Config A", 0x4455, Kind::H2),
  ("Permanent Failure Enabled PF A", 0x44f5, Kind::H1),
  ("Permanent Failure Enabled PF B", 0x44f6, Kind::H1),
  ("Permanent Failure Enabled PF C", 0x44f7, Kind::H1),
  ("Permanent Failure Enabled PF D", 0x44f8, Kind::H1),
  ("Std Temp Charging Voltage, mV/cell", 0x4544, Kind::I2),
  ("Charge Term Taper Current, mA", 0x456c, Kind::I2),
  ("Charge Term Voltage, mV", 0x4570, Kind::I2),
  ("DA Configuration", 0x457b, Kind::H1),
  ("Dsg Current Threshold, mA", 0x4586, Kind::I2),
  ("Chg Current Threshold, mA", 0x4588, Kind::I2),
  ("Quit Current, mA", 0x458a, Kind::I2),
  ("CEDV Gauging Configuration", 0x458e, Kind::H2),
  ("EMF", 0x4590, Kind::U2),
  ("C0", 0x4592, Kind::U2),
  ("R0", 0x4594, Kind::U2),
  ("T0", 0x4596, Kind::U2),
  ("R1", 0x4598, Kind::U2),
  ("TC", 0x459a, Kind::U1),
  ("C1", 0x459b, Kind::U1),
  ("Age Factor", 0x459c, Kind::U1),
  ("Fixed EDV 0, mV", 0x459d, Kind::I2),
  ("Fixed EDV 1, mV", 0x45a0, Kind::I2),
  ("Fixed EDV 2, mV", 0x45a3, Kind::I2),
  ("Battery Low %, 0.01 %", 0x45ec, Kind::U2),
  ("Learning Low Temp, 0.1 C", 0x45f9, Kind::U1),
  ("OverLoad Current, mA", 0x4602, Kind::I2),
  ("Self Discharge Rate, 0.01 %/day", 0x4606, Kind::U1),
  ("Near Full, mAh", 0x4609, Kind::I2),
  ("CEDV Profile 1 Voltage 0 % DOD, mV", 0x45a6, Kind::I2),
  ("CEDV Profile 1 Voltage 10 % DOD, mV", 0x45a8, Kind::I2),
  ("CEDV Profile 1 Voltage 20 % DOD, mV", 0x45aa, Kind::I2),
  ("CEDV Profile 1 Voltage 30 % DOD, mV", 0x45ac, Kind::I2),
  ("CEDV Profile 1 Voltage 40 % DOD, mV", 0x45ae, Kind::I2),
  ("CEDV Profile 1 Voltage 50 % DOD, mV", 0x45b0, Kind::I2),
  ("CEDV Profile 1 Voltage 60 % DOD, mV", 0x45b2, Kind::I2),
  ("CEDV Profile 1 Voltage 70 % DOD, mV", 0x45b4, Kind::I2),
  ("CEDV Profile 1 Voltage 80 % DOD, mV", 0x45b6, Kind::I2),
  ("CEDV Profile 1 Voltage 90 % DOD, mV", 0x45b8, Kind::I2),
  ("CEDV Profile 1 Voltage 100 % DOD, mV", 0x45ba, Kind::I2),
  ("Chg Relax Time, s", 0x458d, Kind::U1),
];

/// MAC subcommands with flags: name, subcommand, data bytes
const STATUS: &[(&str, u16, usize)] = &[
  ("OperationStatus", 0x0054, 4),
  ("ChargingStatus", 0x0055, 3),
  ("GaugingStatus", 0x0056, 3),
  ("ManufacturingStatus", 0x0057, 2),
  ("SafetyStatus", 0x0051, 4),
  ("PFStatus", 0x0053, 4),
];

/// SBS words: name, command
const WORDS: &[(&str, u8)] = &[
  ("MaxError, %", 0x0c),
  ("RelativeStateOfCharge, %", 0x0d),
  ("RemainingCapacity", 0x0f),
  ("FullChargeCapacity", 0x10),
  ("ChargingCurrent, mA", 0x14),
  ("ChargingVoltage, mV", 0x15),
  ("BatteryStatus", 0x16),
  ("DesignCapacity", 0x18),
];

#[cortex_m_rt::entry]
fn main() -> ! {
  let mut cp = cortex_m::Peripherals::take().unwrap();
  let dp = pac::Peripherals::take().unwrap();
  let mut flash = dp.FLASH.constrain();
  let mut rcc = dp.RCC.freeze(clock::idle_config(), &mut flash.acr);
  cp.DCB.enable_trace();
  cp.DWT.enable_cycle_counter();
  // The whole report fits, so it can be read after the fact
  rtt_init_print!(rtt_target::ChannelMode::NoBlockSkip, 4096);

  let mut gpiob = dp.GPIOB.split(&mut rcc);
  let scl = gpiob.pb10.into_open_drain_output_with_state(&mut gpiob.crh, PinState::High);
  let sda = gpiob.pb11.into_open_drain_output_with_state(&mut gpiob.crh, PinState::High);
  let mut smbus = SoftI2c::new(scl, sda, CycleDelay, 100_000);
  smbus.recover().ok();
  let mut gauge = BQ4050::new(smbus);

  for &(name, command) in WORDS {
    match retry(|| gauge.read_sbs_word(command)) {
      Ok(value) => {
        rprintln!("{}: {} ({:#06x})", name, value, value);
      }
      Err(e) => {
        rprintln!("{}: {:?}", name, e);
      }
    }
  }
  for &(name, subcommand, len) in STATUS {
    let mut buffer = [0u8; 40];
    match retry(|| gauge.mac_read(subcommand, &mut buffer).map(|data| data.len())) {
      Ok(got) => {
        rprintln!("{}: {:02x?}", name, &buffer[3..3 + got.min(len)]);
      }
      Err(e) => {
        rprintln!("{}: {:?}", name, e);
      }
    }
  }
  for &(name, address, kind) in SETTINGS {
    let mut data = [0u8; 32];
    if let Err(e) = retry(|| gauge.read_data_flash(address, &mut data)) {
      rprintln!("{:#06x} {}: {:?}", address, name, e);
      continue;
    }
    let word = u16::from_le_bytes([data[0], data[1]]);
    match kind {
      Kind::I2 => {
        rprintln!("{:#06x} {}: {}", address, name, word as i16);
      }
      Kind::U2 => {
        rprintln!("{:#06x} {}: {}", address, name, word);
      }
      Kind::U1 => {
        rprintln!("{:#06x} {}: {}", address, name, data[0]);
      }
      Kind::H1 => {
        rprintln!("{:#06x} {}: {:#04x}", address, name, data[0]);
      }
      Kind::H2 => {
        rprintln!("{:#06x} {}: {:#06x}", address, name, word);
      }
    }
  }
  rprintln!("done");
  loop {
    cortex_m::asm::wfi();
  }
}

/// The gauge NACKs while it writes its data flash: try a few times, 50 ms apart
fn retry<T, E>(mut f: impl FnMut() -> Result<T, E>) -> Result<T, E> {
  use embedded_hal::delay::DelayNs;
  let mut result = f();
  for _ in 0..3 {
    if result.is_ok() {
      break;
    }
    CycleDelay.delay_ms(50);
    result = f();
  }
  result
}
