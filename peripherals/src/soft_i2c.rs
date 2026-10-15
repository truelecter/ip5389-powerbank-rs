//! Bit-banged I2C/SMBus master that waits out clock stretching.
//!
//! The BQ4050 stretches the clock for hundreds of microseconds. The AT32F403A I2C peripheral
//! does not wait for it reliably: reads come back shifted by a bit, with `0xFF` high bytes,
//! timeouts and arbitration losses. Driving the lines in software and waiting for SCL to
//! actually go high fixes that.

use embedded_hal::{
  delay::DelayNs,
  digital::{InputPin, OutputPin},
  i2c::{ErrorKind, ErrorType, I2c, NoAcknowledgeSource, Operation, SevenBitAddress},
};

/// SMBus clock low timeout: a slave may stretch the clock this long
const STRETCH_TIMEOUT_US: u32 = 35_000;
/// Clock stretching is polled in steps this long
const STRETCH_POLL_NS: u32 = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
  /// Another device drove SDA low while the master sent a high bit
  ArbitrationLoss,
  NoAcknowledge(NoAcknowledgeSource),
  /// A slave held SCL low longer than the SMBus timeout
  Timeout,
  /// A slave still holds SDA or SCL low after nine clock pulses
  BusHeld,
  /// A pin could not be driven or read
  Pin,
}

impl embedded_hal::i2c::Error for Error {
  fn kind(&self) -> ErrorKind {
    match *self {
      Error::ArbitrationLoss => ErrorKind::ArbitrationLoss,
      Error::NoAcknowledge(source) => ErrorKind::NoAcknowledge(source),
      Error::BusHeld => ErrorKind::Bus,
      Error::Timeout | Error::Pin => ErrorKind::Other,
    }
  }
}

/// Frees a bus that a slave still holds, for example because the master was reset in the
/// middle of a read: clocks SCL until the slave releases SDA, at most nine times (a byte and
/// its acknowledge bit), then sends a STOP. Works on the bare pins, so it can run before the
/// pins are handed to a hardware I2C peripheral.
pub fn recover_bus<SCL, SDA, D>(scl: &mut SCL, sda: &mut SDA, delay: &mut D, frequency_hz: u32) -> Result<(), Error>
where
  SCL: OutputPin + InputPin,
  SDA: OutputPin + InputPin,
  D: DelayNs,
{
  let half_ns = 500_000_000 / frequency_hz;
  sda.set_high().map_err(|_| Error::Pin)?;
  scl.set_high().map_err(|_| Error::Pin)?;
  delay.delay_ns(half_ns);
  for _ in 0..9 {
    if sda.is_high().map_err(|_| Error::Pin)? {
      break;
    }
    scl.set_low().map_err(|_| Error::Pin)?;
    delay.delay_ns(half_ns);
    scl.set_high().map_err(|_| Error::Pin)?;
    delay.delay_ns(half_ns);
  }
  if !sda.is_high().map_err(|_| Error::Pin)? || !scl.is_high().map_err(|_| Error::Pin)? {
    return Err(Error::BusHeld);
  }
  // STOP: SDA rises while SCL is high
  scl.set_low().map_err(|_| Error::Pin)?;
  delay.delay_ns(half_ns);
  sda.set_low().map_err(|_| Error::Pin)?;
  delay.delay_ns(half_ns);
  scl.set_high().map_err(|_| Error::Pin)?;
  delay.delay_ns(half_ns);
  sda.set_high().map_err(|_| Error::Pin)?;
  delay.delay_ns(half_ns);
  Ok(())
}

/// I2C master on two open-drain pins with external pull-ups
pub struct SoftI2c<SCL, SDA, D> {
  scl: SCL,
  sda: SDA,
  delay: D,
  half_period_ns: u32,
}

impl<SCL, SDA, D> SoftI2c<SCL, SDA, D>
where
  SCL: OutputPin + InputPin,
  SDA: OutputPin + InputPin,
  D: DelayNs,
{
  /// `frequency_hz` is the nominal bus clock; pin and delay overhead make the real one lower
  pub fn new(scl: SCL, sda: SDA, delay: D, frequency_hz: u32) -> Self {
    Self { scl, sda, delay, half_period_ns: 500_000_000 / frequency_hz }
  }

  pub fn release(self) -> (SCL, SDA, D) {
    (self.scl, self.sda, self.delay)
  }

  /// Frees the bus if a slave still holds it; see [`recover_bus`]
  pub fn recover(&mut self) -> Result<(), Error> {
    recover_bus(&mut self.scl, &mut self.sda, &mut self.delay, 500_000_000 / self.half_period_ns)
  }

  fn half(&mut self) {
    self.delay.delay_ns(self.half_period_ns);
  }

  fn sda(&mut self, high: bool) -> Result<(), Error> {
    if high { self.sda.set_high() } else { self.sda.set_low() }.map_err(|_| Error::Pin)
  }

  fn sda_is_high(&mut self) -> Result<bool, Error> {
    self.sda.is_high().map_err(|_| Error::Pin)
  }

  fn scl_low(&mut self) -> Result<(), Error> {
    self.scl.set_low().map_err(|_| Error::Pin)
  }

  /// Releases SCL and waits until the slaves release it too
  fn scl_high(&mut self) -> Result<(), Error> {
    self.scl.set_high().map_err(|_| Error::Pin)?;
    let mut waited_ns = 0;
    while !self.scl.is_high().map_err(|_| Error::Pin)? {
      if waited_ns >= STRETCH_TIMEOUT_US * 1000 {
        return Err(Error::Timeout);
      }
      self.delay.delay_ns(STRETCH_POLL_NS);
      waited_ns += STRETCH_POLL_NS;
    }
    Ok(())
  }

  /// START, or repeated START in the middle of a transaction. Leaves SCL low.
  fn start(&mut self) -> Result<(), Error> {
    self.sda(true)?;
    self.half();
    self.scl_high()?;
    self.half();
    self.sda(false)?;
    self.half();
    self.scl_low()
  }

  fn stop(&mut self) -> Result<(), Error> {
    self.sda(false)?;
    self.half();
    self.scl_high()?;
    self.half();
    self.sda(true)?;
    self.half();
    Ok(())
  }

  fn write_bit(&mut self, high: bool) -> Result<(), Error> {
    self.sda(high)?;
    self.half();
    self.scl_high()?;
    self.half();
    let lost = high && !self.sda_is_high()?;
    self.scl_low()?;
    if lost { Err(Error::ArbitrationLoss) } else { Ok(()) }
  }

  fn read_bit(&mut self) -> Result<bool, Error> {
    self.sda(true)?;
    self.half();
    self.scl_high()?;
    self.half();
    let bit = self.sda_is_high()?;
    self.scl_low()?;
    Ok(bit)
  }

  /// Sends a byte and returns whether the slave acknowledged it
  fn write_byte(&mut self, byte: u8) -> Result<bool, Error> {
    for i in (0..8).rev() {
      self.write_bit(byte >> i & 1 == 1)?;
    }
    Ok(!self.read_bit()?)
  }

  fn read_byte(&mut self, ack: bool) -> Result<u8, Error> {
    let mut byte = 0;
    for _ in 0..8 {
      byte = byte << 1 | self.read_bit()? as u8;
    }
    self.write_bit(!ack)?;
    Ok(byte)
  }

  fn operations(&mut self, address: u8, operations: &mut [Operation<'_>]) -> Result<(), Error> {
    let mut reading = None;
    for i in 0..operations.len() {
      let read = matches!(operations[i], Operation::Read(_));
      // Each change of direction needs a (repeated) START and the address again
      if reading != Some(read) {
        self.start()?;
        if !self.write_byte(address << 1 | read as u8)? {
          return Err(Error::NoAcknowledge(NoAcknowledgeSource::Address));
        }
        reading = Some(read);
      }
      // Reads acknowledge every byte except the last one before a STOP or a direction change
      let last_read = !matches!(operations.get(i + 1), Some(Operation::Read(_)));
      match &mut operations[i] {
        Operation::Write(bytes) => {
          for &byte in bytes.iter() {
            if !self.write_byte(byte)? {
              return Err(Error::NoAcknowledge(NoAcknowledgeSource::Data));
            }
          }
        }
        Operation::Read(buffer) => {
          let len = buffer.len();
          for (j, byte) in buffer.iter_mut().enumerate() {
            *byte = self.read_byte(!(last_read && j + 1 == len))?;
          }
        }
      }
    }
    Ok(())
  }
}

impl<SCL, SDA, D> ErrorType for SoftI2c<SCL, SDA, D> {
  type Error = Error;
}

impl<SCL, SDA, D> I2c<SevenBitAddress> for SoftI2c<SCL, SDA, D>
where
  SCL: OutputPin + InputPin,
  SDA: OutputPin + InputPin,
  D: DelayNs,
{
  fn transaction(&mut self, address: u8, operations: &mut [Operation<'_>]) -> Result<(), Error> {
    let result = self.operations(address, operations);
    // Free the bus whatever happened; a failed transaction reports its own error first
    let stopped = self.stop();
    result.and(stopped)
  }
}

#[cfg(test)]
mod tests {
  extern crate std;

  use super::*;
  use core::convert::Infallible;
  use std::{cell::RefCell, rc::Rc};

  /// Two open-drain lines and a slave that holds SDA low for a number of SCL rising edges
  #[derive(Default)]
  struct Bus {
    master_scl_low: bool,
    master_sda_low: bool,
    /// SCL rising edges the slave still needs before it lets go of SDA; `None` holds forever
    slave_holds_for: Option<u32>,
    clocks: u32,
    stops: u32,
  }

  impl Bus {
    fn sda(&self) -> bool {
      !self.master_sda_low && self.slave_holds_for == Some(0)
    }
  }

  struct Scl(Rc<RefCell<Bus>>);
  struct Sda(Rc<RefCell<Bus>>);

  impl embedded_hal::digital::ErrorType for Scl {
    type Error = Infallible;
  }
  impl embedded_hal::digital::ErrorType for Sda {
    type Error = Infallible;
  }

  impl OutputPin for Scl {
    fn set_low(&mut self) -> Result<(), Infallible> {
      self.0.borrow_mut().master_scl_low = true;
      Ok(())
    }
    fn set_high(&mut self) -> Result<(), Infallible> {
      let mut bus = self.0.borrow_mut();
      if bus.master_scl_low {
        bus.clocks += 1;
        if let Some(n) = bus.slave_holds_for.as_mut() {
          *n = n.saturating_sub(1);
        }
      }
      bus.master_scl_low = false;
      Ok(())
    }
  }
  impl InputPin for Scl {
    fn is_high(&mut self) -> Result<bool, Infallible> {
      Ok(!self.0.borrow().master_scl_low)
    }
    fn is_low(&mut self) -> Result<bool, Infallible> {
      Ok(self.0.borrow().master_scl_low)
    }
  }

  impl OutputPin for Sda {
    fn set_low(&mut self) -> Result<(), Infallible> {
      self.0.borrow_mut().master_sda_low = true;
      Ok(())
    }
    fn set_high(&mut self) -> Result<(), Infallible> {
      let mut bus = self.0.borrow_mut();
      if bus.master_sda_low && !bus.master_scl_low {
        bus.stops += 1;
      }
      bus.master_sda_low = false;
      Ok(())
    }
  }
  impl InputPin for Sda {
    fn is_high(&mut self) -> Result<bool, Infallible> {
      Ok(self.0.borrow().sda())
    }
    fn is_low(&mut self) -> Result<bool, Infallible> {
      Ok(!self.0.borrow().sda())
    }
  }

  struct NoDelay;
  impl DelayNs for NoDelay {
    fn delay_ns(&mut self, _: u32) {}
  }

  fn recover(slave_holds_for: Option<u32>) -> (Result<(), Error>, Bus) {
    let bus = Rc::new(RefCell::new(Bus { slave_holds_for, ..Bus::default() }));
    let result = recover_bus(&mut Scl(bus.clone()), &mut Sda(bus.clone()), &mut NoDelay, 100_000);
    (result, Rc::try_unwrap(bus).ok().unwrap().into_inner())
  }

  #[test]
  fn clocks_until_the_slave_lets_go_then_stops() {
    let (result, bus) = recover(Some(3));
    assert_eq!(result, Ok(()));
    // Three pulses free SDA, the fourth rising edge belongs to the STOP
    assert_eq!(bus.clocks, 4);
    assert_eq!(bus.stops, 1);
    assert!(!bus.master_scl_low && !bus.master_sda_low);
  }

  #[test]
  fn idle_bus_only_gets_a_stop() {
    let (result, bus) = recover(Some(0));
    assert_eq!(result, Ok(()));
    assert_eq!(bus.clocks, 1);
    assert_eq!(bus.stops, 1);
  }

  #[test]
  fn gives_up_after_nine_clocks() {
    let (result, bus) = recover(None);
    assert_eq!(result, Err(Error::BusHeld));
    assert_eq!(bus.clocks, 9);
    assert_eq!(bus.stops, 0);
  }
}
