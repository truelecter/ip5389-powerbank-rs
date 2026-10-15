//! Texas Instruments INA3221 triple-channel shunt and bus voltage monitor

use embedded_hal::i2c::I2c;

#[derive(Clone, Copy, Debug)]
pub enum Error<I2cError> {
  I2c(I2cError),
}

impl<I2cError> From<I2cError> for Error<I2cError> {
  fn from(e: I2cError) -> Self {
    Error::I2c(e)
  }
}

/// Pin the A0 address input is tied to
#[derive(Clone, Copy, Debug)]
pub enum AddressPin {
  Gnd,
  Vs,
  Sda,
  Scl,
}

impl AddressPin {
  fn addr(self) -> u8 {
    match self {
      AddressPin::Gnd => 0x40,
      AddressPin::Vs => 0x41,
      AddressPin::Sda => 0x42,
      AddressPin::Scl => 0x43,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
  Ch1,
  Ch2,
  Ch3,
}

impl Channel {
  fn index(self) -> u8 {
    match self {
      Channel::Ch1 => 0,
      Channel::Ch2 => 1,
      Channel::Ch3 => 2,
    }
  }

  /// Shunt voltage register, the bus voltage register follows it
  fn shunt_reg(self) -> u8 {
    REG_CH1_SHUNT + 2 * self.index()
  }

  /// Channel enable bit in the configuration register
  fn enable_bit(self) -> u16 {
    1 << (14 - self.index())
  }
}

const REG_CONFIG: u8 = 0x00;
const REG_CH1_SHUNT: u8 = 0x01;

const CONFIG_RESET: u16 = 1 << 15;

/// Shunt voltage LSB, volts
const SHUNT_LSB: f32 = 40e-6;
/// Bus voltage LSB, volts
const BUS_LSB: f32 = 8e-3;

pub struct INA3221<I2C> {
  i2c: I2C,
  address: u8,
}

impl<I2C, I2cError> INA3221<I2C>
where
  I2C: I2c<Error = I2cError>,
{
  pub fn new(i2c: I2C, address: AddressPin) -> Self {
    INA3221 {
      i2c,
      address: address.addr(),
    }
  }

  fn read_reg(&mut self, reg: u8) -> Result<u16, Error<I2cError>> {
    let mut buffer = [0u8; 2];
    self.i2c.write_read(self.address, &[reg], &mut buffer)?;
    Ok(u16::from_be_bytes(buffer))
  }

  fn write_reg(&mut self, reg: u8, value: u16) -> Result<(), Error<I2cError>> {
    let [msb, lsb] = value.to_be_bytes();
    self.i2c.write(self.address, &[reg, msb, lsb])?;
    Ok(())
  }

  /// Reads a measurement register: a signed value in bits 15..3
  fn read_measurement(&mut self, reg: u8) -> Result<i16, Error<I2cError>> {
    Ok((self.read_reg(reg)? as i16) >> 3)
  }

  /// Resets all registers to their power-on values
  pub fn reset(&mut self) -> Result<(), Error<I2cError>> {
    self.write_reg(REG_CONFIG, CONFIG_RESET)
  }

  pub fn enable_channel(&mut self, channel: Channel, enabled: bool) -> Result<(), Error<I2cError>> {
    let config = self.read_reg(REG_CONFIG)?;
    let config = if enabled {
      config | channel.enable_bit()
    } else {
      config & !channel.enable_bit()
    };
    self.write_reg(REG_CONFIG, config)
  }

  /// Voltage across the shunt resistor, volts
  pub fn shunt_voltage(&mut self, channel: Channel) -> Result<f32, Error<I2cError>> {
    Ok(self.read_measurement(channel.shunt_reg())? as f32 * SHUNT_LSB)
  }

  /// Bus voltage to ground, volts
  pub fn bus_voltage(&mut self, channel: Channel) -> Result<f32, Error<I2cError>> {
    Ok(self.read_measurement(channel.shunt_reg() + 1)? as f32 * BUS_LSB)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use embedded_hal::i2c::{ErrorKind, ErrorType, Operation};

  /// Answers reads with `response`, records the last write
  struct FakeBus {
    response: [u8; 2],
    written: [u8; 3],
    pointer: u8,
  }

  impl FakeBus {
    fn new(response: u16) -> Self {
      FakeBus {
        response: response.to_be_bytes(),
        written: [0; 3],
        pointer: 0,
      }
    }
  }

  impl ErrorType for FakeBus {
    type Error = ErrorKind;
  }

  impl I2c for FakeBus {
    fn transaction(&mut self, address: u8, operations: &mut [Operation<'_>]) -> Result<(), ErrorKind> {
      assert_eq!(address, 0x40);
      for operation in operations {
        match operation {
          Operation::Write(bytes) => {
            self.pointer = bytes[0];
            if bytes.len() == 3 {
              self.written.copy_from_slice(bytes);
            }
          }
          Operation::Read(buffer) => buffer.copy_from_slice(&self.response),
        }
      }
      Ok(())
    }
  }

  #[test]
  fn register_write_is_one_transaction_msb_first() {
    let mut ina = INA3221::new(FakeBus::new(0x7127), AddressPin::Gnd);
    ina.enable_channel(Channel::Ch2, false).unwrap();
    // channel 2 enable is bit 13
    assert_eq!(ina.i2c.written, [REG_CONFIG, 0x51, 0x27]);
  }

  #[test]
  fn shunt_voltage_is_signed() {
    // -100 LSB in bits 15..3
    let mut ina = INA3221::new(FakeBus::new((-100i16 << 3) as u16), AddressPin::Gnd);
    let volts = ina.shunt_voltage(Channel::Ch3).unwrap();
    assert_eq!(ina.i2c.pointer, 0x05);
    assert!((volts - -100.0 * 40e-6).abs() < 1e-9);
  }

  #[test]
  fn bus_voltage_reads_the_register_after_the_shunt() {
    // 12 V = 1500 LSB
    let mut ina = INA3221::new(FakeBus::new(1500 << 3), AddressPin::Gnd);
    let volts = ina.bus_voltage(Channel::Ch2).unwrap();
    assert_eq!(ina.i2c.pointer, 0x04);
    assert!((volts - 12.0).abs() < 1e-6);
  }
}
