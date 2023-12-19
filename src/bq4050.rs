#![no_std]
use byteorder::{ByteOrder, LittleEndian};
use embedded_hal::blocking::i2c::{WriteRead, Write, Read};

/// I2C address
#[derive(Copy, Clone)]
pub enum Address {
    /// Device address
    Dev = 0x0B,
    /// Register of MAC
    Mac = 0x44,
}

// Each methods refers the corresponding section and command from bq4050 Technical Reference https://www.ti.com/lit/ug/sluuaq3a/sluuaq3a.pdf
// Read word commands
#[derive(Copy, Clone)]
pub enum Cmd {
    TemperatureReg = 0x08,
    VoltageReg = 0x09,
    CurrentReg = 0x0A,
    AverageCurrentReg = 0x0B,
    MaxErrorReg = 0x0C,
    RelativeSocReg = 0x0D,
    AbsoluteSocReg = 0x0E,
    RemainingCapacityReg = 0x0F,
    FullChargeCapacityReg = 0x10,
    ChargingCurrentReg = 0x14,
    ChargingVoltageReg = 0x15,
    BatteryStatusReg = 0x16,
    CycleCountReg = 0x17,
    CellVoltage4Reg = 0x3C,
    CellVoltage3Reg = 0x3D,
    CellVoltage2Reg = 0x3E,
    CellVoltage1Reg = 0x3F,
    SohReg = 0x4F,
    SerialNumberReg = 0x1C,
}

// Read word
#[derive(Copy, Clone)]
pub enum CmdBlock {
    DEVICENAMEReg = 0x21,
}

#[derive(Clone, Copy, Debug)]
pub enum Error<I2cError> {
    I2cError(I2cError),
}

pub struct BQ4050<I2C> {
    i2c: I2C,
}

impl<I2C, I2cError> BQ4050<I2C>
where
    I2C: WriteRead<Error = I2cError> + Write<Error = I2cError> + Read<Error = I2cError>
{
    pub fn new(i2c: I2C) -> BQ4050<I2C> {
        BQ4050 { i2c: i2c }
    }

    // 13.2 0x01 RemainingCapacityAlarm()
    // This read/write word function sets a low capacity alarm threshold for the cell stack.
    // Protocol - Word
    // Unit - If BatteryMode()[CAPM]= 0, then the data reports in mAh.
    // Unit - If BatteryMode()[CAPM]= 1, then the data reports in 10 mWh.

    // 13.3 0x02 RemainingTimeAlarm()
    // This read/write word function sets a low remaining time-to-fully discharge alarm threshold for the cell stack.
    // Protocol - Word
    // Unit - min

    // 13.4 0x03 BatteryMode()
    // This read/write word function sets various battery operating mode options.
    // Protocol - Word
    // TODO: describe battery modes

    // 13.5 0x04 AtRate()
    // This read/write word function sets the value used in calculating AtRateTimeToFull() and AtRateTimeToEmpty().
    // Protocol - Word
    // Unit - If BatteryMode()[CAPM]= 0, then the data reports in mAh.
    // Unit - If BatteryMode()[CAPM]= 1, then the data reports in 10 mWh.

    // 13.6 0x05 AtRateTimeToFull()
    // This word read function returns the remaining time-to-fully charge the battery stack.
    // Protocol - Word
    // Unit - min

    // 13.7 0x06 AtRateTimeToEmpty()
    // This word read function returns the remaining time-to-fully discharge the battery stack.
    // Protocol - Word
    // Unit - min

    // 13.8 0x07 AtRateOK()
    // This read-word function returns a Boolean value that indicates whether the battery can deliver AtRate() for at least 10s.
    // Protocol - Word
    // ---
    // 13.9 0x08 Temperature()
    // This read-word function returns the temperature in units 0.1°K.
    // Protocol - Word
    // Unit - 0.1°K
    pub fn get_temperature(&mut self) -> Result<f32, Error<I2cError>> {
        let mut buffer = [0u8; 2];
        self.int_rw(
            Address::Dev as u8,
            &[Cmd::TemperatureReg as u8],
            &mut buffer,
        )?;
        // self.i2c.write_read(
        //     Address::Dev as u8,
        //     &[Cmd::TemperatureReg as u8],
        //     &mut buffer,
        // )?;
        Ok(convert_temperature(LittleEndian::read_u16(&buffer[0..2])))
    }

    fn int_rw(
        &mut self,
        address: u8,
        bytes: &[u8],
        buffer: &mut [u8]
    ) -> Result<(), Error<I2cError>> {
        self.i2c.write(address, bytes)?;
        self.i2c.read(address, buffer)?;

        Ok(())
    }

    // 13.10 0x09 Voltage()
    // This read-word function returns the sum of the measured cell voltages.
    // Protocol - Word
    // Unit - mV
    pub fn get_voltage(&mut self) -> Result<u16, Error<I2cError>> {
        let mut buffer = [0u8; 2];
        self.i2c
            .write_read(Address::Dev as u8, &[Cmd::VoltageReg as u8], &mut buffer)?;
        Ok(LittleEndian::read_u16(&buffer[0..2]))
    }
    // 13.11 0x0A Current()
    // This read-word function returns the measured current from the coulomb counter.
    // If the input to the device exceeds the maximum value,the value is clamped at the maximum and does not roll over.
    // Protocol - Word
    // Unit - mA
    pub fn get_current(&mut self) -> Result<u16, Error<I2cError>> {
        let mut buffer = [0u8; 2];
        self.i2c
            .write_read(Address::Dev as u8, &[Cmd::CurrentReg as u8], &mut buffer)?;
        Ok(LittleEndian::read_u16(&buffer[0..2]))
    }
    // 13.12 0x0B AverageCurrent()
    // Protocol - Word
    // Unit - mA
    pub fn get_average_current(&mut self) -> Result<u16, Error<I2cError>> {
        let mut buffer = [0u8; 2];
        self.i2c.write_read(
            Address::Dev as u8,
            &[Cmd::AverageCurrentReg as u8],
            &mut buffer,
        )?;
        Ok(LittleEndian::read_u16(&buffer[0..2]))
    }
    // 13.13 0x0C MaxError()
    // This read-word function returns the expected margin of error, in %, in the state-of-charge calculation with a rangeof 1 to 100%.
    // Protocol - Word
    // Unit - %
    pub fn get_max_error(&mut self) -> Result<u16, Error<I2cError>> {
        let mut buffer = [0u8; 2];
        self.i2c
            .write_read(Address::Dev as u8, &[Cmd::MaxErrorReg as u8], &mut buffer)?;
        Ok(LittleEndian::read_u16(&buffer[0..2]))
    }

    // 13.14 0x0D RelativeStateOfCharge()
    // This read-word function returns the predicted remaining battery capacity as a percentage of FullChargeCapacity().
    // Protocol - Word
    // Unit - %
    pub fn get_relative_state_of_charge(&mut self) -> Result<u16, Error<I2cError>> {
        let mut buffer = [0u8; 2];
        self.i2c.write_read(
            Address::Dev as u8,
            &[Cmd::RelativeSocReg as u8],
            &mut buffer,
        )?;
        Ok(LittleEndian::read_u16(&buffer[0..2]))
    }

    // 13.15 0x0E AbsoluteStateOfCharge()
    // This read-word function returns the predicted remaining battery capacity as a percentage.
    // Protocol - Word
    // Unit - %
    pub fn get_absolute_state_of_charge(&mut self) -> Result<u16, Error<I2cError>> {
        let mut buffer = [0u8; 2];
        self.i2c.write_read(
            Address::Dev as u8,
            &[Cmd::AbsoluteSocReg as u8],
            &mut buffer,
        )?;
        Ok(LittleEndian::read_u16(&buffer[0..2]))
    }

    // 13.16 0x0F RemainingCapacity()
    // This read-word function returns the predicted remaining battery capacity.
    // Protocol - Word
    // Unit - If BatteryMode()[CAPM]= 0, then the data reports in mAh.
    // Unit - If BatteryMode()[CAPM]= 1, then the data reports in 10 mWh.
    // ---
    // 13.17 0x10 FullChargeCapacity()
    // This read-word function returns the predicted battery capacity when fully charged.
    // The value returned will not be updated during charging.
    // Protocol - Word
    // Unit - If BatteryMode()[CAPM]= 0, then the data reports in mAh.
    // Unit - If BatteryMode()[CAPM]= 1, then the data reports in 10 mWh.
    // ---
    // 13.18 0x11 RunTimeToEmpty()
    // This read-word function returns the predicted remaining battery capacity based on the present rate of discharge.
    // Protocol - Word
    // Unit - min
    // 65535 = Battery is not being discharged.
    // ---
    // 13.19 0x12 AverageTimeToEmpty()
    // This read-word function returns the predicted remaining battery capacity based on AverageCurrent().
    // Protocol - Word
    // Unit - min
    // 65535 = Battery is not being discharged.
    // ---
    // 13.20 0x13 AverageTimeToFull()
    // This read-word function returns the predicted time-to-full charge based on AverageCurrent().
    // Protocol - Word
    // Unit - min
    // 65535 = Battery is not being discharged.
    // ---
    // 13.21 0x14 ChargingCurrent()
    // This read-word function returns the desired charging current.
    // Protocol - Word
    // Unit - mA
    // 65535 = Battery is not being discharged.
    // ---
    // 13.22 0x15 ChargingVoltage()
    // This read-word function returns the desired charging voltage.
    // Protocol - Word
    // Unit - mV
    // 65535 = Battery is not being discharged.
    // ---

    // 13.23 0x16 BatteryStatus()
    // This read-word function returns various battery status information.
    // Protocol - Word
    // TODO: describe battery modes
    // ---

    // 13.24 0x17 CycleCount()
    // This read-word function returns the number of discharge cycles the battery has experienced.
    // The default value is stored in the data flash value CycleCount, which is updated in runtime.
    // Protocol - Word
    // Unit - cycles
    // ---

    // 13.25 0x18 DesignCapacity()
    // This read-word function returns the theoretical pack capacity.
    // The default value is stored in the data flash value Design Capacity mAh or Design Capacityc Wh.
    // Protocol - Word
    // Unit - If BatteryMode()[CAPM]= 0, then the data reports in mAh.
    // Unit - If BatteryMode()[CAPM]= 1, then the data reports in 10 mWh.
    // ---

    // 13.26 0x19 DesignVoltage()
    // This read-word function returns the theoretical pack voltage.
    // The default value is stored in data flash value Design Voltage.
    // Protocol - Word
    // Unit - mV
    // TODO: describe versions
    // ---
    // 13.27 0x1A SpecificationInfo()
    // Protocol - Word
    // ---

    // 13.28 0x1B ManufacturerDate()
    // This read-word function returns the pack's manufacturer date.
    // Protocol - Word
    // ManufacturerDate() value in the following format: Day + Month*32+ (Year–1980)*512

    // 13.29 0x1C SerialNumber()
    // This read-word function returns the assigned pack serial number.
    // Protocol - Word
    pub fn get_serial_number(&mut self) -> Result<u16, Error<I2cError>> {
        let mut buffer = [0u8; 2];
        self.i2c.write_read(
            Address::Dev as u8,
            &[Cmd::SerialNumberReg as u8],
            &mut buffer,
        )?;
        Ok(LittleEndian::read_u16(&buffer[0..2]))
    }

    // 13.30 0x20 ManufacturerName()
    // This read-block function returns the pack manufacturer's name.
    // Protocol - Block
    // Unit - ASCII
    // ---
    // 13.31 0x21 DeviceName()
    // This read-block function returns the assigned pack name.
    // Protocol - Block
    // Unit - ASCII
    // ---
    // 13.32 0x22 DeviceChemistry()
    // This read-block function returns the battery chemistry used in the pack.
    // Protocol - Block
    // Unit - ASCII
    // ---

    // 13.33 0x23 ManufacturerData()
    // This read-block function returns ManufacturerInfo by default.
    // The command also returns a response to MAC command in order to maintain compatibility of the MAC system in bq30zxy family.
    // Protocol - Block
    // ---

    // 13.34 0x2F Authenticate()
    // This read/write block function provides SHA-1 authentication to send the challenge and read the response in the default mode.
    // It is also used to input a new authentication key when the MAC AuthenticationKey() is used.
    // Protocol - Block
    // ---

    // 13.35 0x3C CellVoltage4()
    // This read-word function returns the Cell 4 voltage.
    // Protocol - Word
    // Unit - mV
    pub fn get_cell_voltage_4(&mut self) -> Result<u16, Error<I2cError>> {
        let mut buffer = [0u8; 2];
        self.i2c.write_read(
            Address::Dev as u8,
            &[Cmd::CellVoltage4Reg as u8],
            &mut buffer,
        )?;
        Ok(LittleEndian::read_u16(&buffer[0..2]))
    }
    // 13.36 0x3D CellVoltage3()
    // This read-word function returns the Cell 3 voltage.
    // Protocol - Word
    // Unit - mV
    pub fn get_cell_voltage_3(&mut self) -> Result<u16, Error<I2cError>> {
        let mut buffer = [0u8; 2];
        self.i2c.write_read(
            Address::Dev as u8,
            &[Cmd::CellVoltage3Reg as u8],
            &mut buffer,
        )?;
        Ok(LittleEndian::read_u16(&buffer[0..2]))
    }
    // 13.37 0x3E CellVoltage2()
    // This read-word function returns the Cell 2 voltage.
    // Protocol - Word
    // Unit - mV
    pub fn get_cell_voltage_2(&mut self) -> Result<u16, Error<I2cError>> {
        let mut buffer = [0u8; 2];
        self.i2c.write_read(
            Address::Dev as u8,
            &[Cmd::CellVoltage2Reg as u8],
            &mut buffer,
        )?;
        Ok(LittleEndian::read_u16(&buffer[0..2]))
    }
    // 13.38 0x3F CellVoltage1()
    // This read-word function returns the Cell 1 voltage.
    // Protocol - Word
    // Unit - mV
    pub fn get_cell_voltage_1(&mut self) -> Result<u16, Error<I2cError>> {
        let mut buffer = [0u8; 2];
        self.i2c.write_read(
            Address::Dev as u8,
            &[Cmd::CellVoltage1Reg as u8],
            &mut buffer,
        )?;
        Ok(LittleEndian::read_u16(&buffer[0..2]))
    }
    // 13.39 0x4A BTPDischargeSet()
    // This read/write word command updates the BTP set threshold for discharge mode for the next BTP interrupt,
    // de-asserts the present BTP interrupt, and clears the OperationStatus()[BTP_INT] bit.
    // Format - SignedInt
    // Unit - mAh
    // ---
    // 13.40 0x4B BTPChargeSet()
    // This read/write word command updates the BTP set threshold for charge mode for the next BTP interrupt,
    // de-asserts the present BTP interrupt, and clears the OperationStatus()[BTP_INT] bit.
    // Format - SignedInt
    // Unit - mAh
    // ---

    // 13.41 0x4F State-of-Health(SoH)
    // This read word command returns the SoH information of the battery in percentage of design capacity and design energy.
    pub fn get_soh(&mut self) -> Result<u16, Error<I2cError>> {
        let mut buffer = [0u8; 2];
        self.i2c
            .write_read(Address::Dev as u8, &[Cmd::SohReg as u8], &mut buffer)?;
        Ok(LittleEndian::read_u16(&buffer[0..2]))
    }

    // 13.42 0x50 SafetyAlert
    // This command returns the SafetyAlert() flags. For a description of each bit flag, see the ManufacturerAccess()
    // version of the same command in Section 13.1.
    // Protocol - Block
    // NOTE: This command and commands 0x51 to 0x58 are not accessible in SEALED mode.
    // ---
    // 13.43 0x51 SafetyStatus
    // This command returns the SafetyStatus() flags. For a description of each bit flag, see the ManufacturerAccess()
    // version of the same command in Section 13.1.
    // Protocol - Block
    // ---
    // 13.44 0x52 PFAlert
    // This command returns the PFAlert() flags. For a description of each bit flag, see the ManufacturerAccess()
    // version of the same command in Section 13.1.
    // Protocol - Block
    // ---
    // 13.45 0x53 PFStatus
    // This command returns the PFStatus() flags. For a description of each bit flag, see the ManufacturerAccess()
    // version of the same command in Section 13.1.
    // Protocol - Block
    // ---
    // 13.46 0x54 OperationStatus
    // This command returns the OperationStatus() flags. For a description of each bit flag, see the ManufacturerAccess()
    // version of the same command in Section 13.1.
    // Protocol - Block
    // ---
    // 13.47 0x55 ChargingStatus
    // This command returns the ChargingStatus() flags. For a description of each bit flag, see the ManufacturerAccess()
    // version of the same command in Section 13.1.
    // Protocol - Block
    // ---
    // 13.48 0x56 GaugingStatus
    // This command returns the GaugingStatus() flags. For a description of each bit flag, see the ManufacturerAccess()
    // version of the same command in Section 13.1.
    // Protocol - Block
    // ---
    // 13.49 0x57 ManufacturingStatus
    // This command returns the ManufacturingStatus() flags. For a description of each bit flag, see the ManufacturerAccess()
    // version of the same command in Section 13.1.
    // Protocol - Block
    // ---
    // 13.50 0x58 AFE Register
    // This command returns a snapshotof the AFE register settings. For a description of each bit flag, see the ManufacturerAccess()
    // version of the same command in Section 13.1.
    // Protocol - Block
    // ---

    // 13.51 0x59 TURBO_POWER
    // TURBO_POWER reports the maximal peak power value, MAX_POWER. The gauge computes a new RAM value every second.
    // TURBO_POWER() is initialized to the result of the max power calculation at reset or power up.
    // Protocol - Word
    // Unit - cW
    // ---
    // 13.52 0x5A TURBO_FINAL
    // TURBO_FINAL sets Min Turbo Power, which represents the minimal TURBO BOOST mode power level during active operation (for example, non-SLEEP).
    // Protocol - Word
    // Unit - cW
    // ---
    // 13.53 0x5B TURBO_PACK_R
    // TURBO_PACK_R sets the PACK Resistance value of the battery pack serial resistance,
    // including resistance associated with FETs, traces, sense resistors, and so on TURBO_PACK_R() accesses to the data flash value Pack Resistance.
    // Protocol - Word
    // Unit - cΩ
    // ---
    // 13.54 0x5C TURBO_SYS_R
    // TURBO_SYS_R sets the System Resistance value of the system serial resistance along the path from battery to system power converter
    // input that includes FETs, traces, sense resistors, and so on TURBO_SYS_R() accesses to the data flash value System Resistance.
    // Protocol - Word
    // Unit - cΩ
    // ---
    // 13.55 0x5D TURBO_EDV
    // TURBO_EDV sets the Minimal Voltage at the system power converter input at which the system will still operate.
    // TURBO_EDV() is written to the data flash value TerminateVoltage.
    // Intended use is to write it once on first use to adjust for possible changes in system design from the time the battery pack was designed.
    // Protocol - Word
    // Unit - mV
    // ---
    // 13.56 0x5E TURBO_CURRENT
    // The gauge computes a maximal discharge current supported by the cell design for a C-rate discharge pulse for 10 ms.
    // This value is updated every 1s for the system to read.
    // Protocol - Word
    // Unit - mAh
    // NOTE:This computes a maximal discharge current supported by the cell design.
    // ---
    // 13.57 0x5F NoLoadRemCap()
    // This read-only word command returns the equivalen to fRemainingCapacity() under a no load condition.
    // For a description of returned data values, see theManufacturerAccess() version of same command in Section 13.1.
    // Protocol - UnsignedInt
    // Unit - mAh
    // ---
    // 13.58 0x60 LifetimeDataBlock1
    // This command returns the first block of LifetimeData.
    // For a description of returned data values, see theManufacturerAccess() version of the same command in Section 13.1.
    // NOTE: This command and commands 0x61 to 0x78 are not accessible in SEALED mode.
    // Protocol - Block
    // ---
    // 13.59 0x61 LifetimeDataBlock2
    // This command returns the second block of LifetimeData.
    // For a description of returned data values, see the ManufacturerAccess() version of the same command in Section 13.1.
    // Protocol - Block
    // ---
    // 13.60 0x62 LifetimeDataBlock3
    // This command returns the third block of LifetimeData.
    // For a description of returned data values, see theManufacturerAccess() version of the same command in Section 13.1.
    // Protocol - Block
    // ---
    // 13.61 0x63 LifetimeDataBlock4
    // This command returns the third block of LifetimeData.
    // For a description of returned data values, see theManufacturerAccess() version of the same command in Section 13.1.
    // Protocol - Block
    // ---
    // 13.62 0x64 LifetimeDataBlock5
    // This command returns the third block of LifetimeData.
    // For a description of returned data values, see theManufacturerAccess() version of the same command in Section 13.1.
    // Protocol - Block
    // ---
    // 13.63 0x70 ManufacturerInfo
    // This command return smanufacturer information.
    // For a description of returned data values, see theManufacturerAccess() version of the same command in Section 13.1.
    // Protocol - Block
    // ---
    // 13.64 0x71 DAStatus1
    // This command returns the Cell Voltages, PackVoltage, Bat Voltage, Cell Currents, Cell Powers, Power,and AveragePower.
    // For a description of returned data values, see the ManufacturerAccess() versionoft he samecomm and in Section 13.1.
    // Protocol - Block
    // ---
    // 13.65 0x72 DAStatus2
    // This command returns the internal temp sensor, TS1, TS2, TS3, TS4, Cell Temp, and FET Temp.
    // For a description of returned data values, see theManufacturerAccess() version of the same command in Section 13.1.
    // Protocol - Block
    // ---
    // 13.66 0x73 GaugeStatus1
    // This commandinstructsthe deviceto return Impedance Track related gauging information.
    // For a description of returned data values, see theManufacturerAccess() version of the same command in Section 13.1.
    // Protocol - Block
    // ---
    // 13.67 0x74 GaugeStatus2
    // This commandinstructsthe deviceto return Impedance Track related gauging information.
    // For a description of returned data values, see theManufacturerAccess() version of the same command in Section 13.1.
    // Protocol - Block
    // ---
    // 13.68 0x75 GaugeStatus3
    // This commandinstructsthe deviceto return Impedance Track related gauging information.
    // For a description of returned data values, see theManufacturerAccess() version of the same command in Section 13.1.
    // Protocol - Block
    // ---
    // 13.69 0x76 CBStatus
    // This commandinstructsthe deviceto returncell balancetime information.
    // For a description of returned data values, see the ManufacturerAccess() version of the same command in Section 13.1.
    // Protocol - Block
    // ---
    // 13.70 0x77 State-of-Health
    // This commandinstructsthe deviceto returnthe state-of-healthfull chargecapacityand energy.
    // For a description of returned data values, see theManufacturerAccess() version of the same command in Section 13.1.
    // Protocol - Block
    // ---
    // 13.71 0x78 FilteredCapacity
    // This commandinstructsthe deviceto returnthe filteredcapacityand energyevenif[SMOOTH]= 0.
    // For a description of returned data values, see theManufacturerAccess() version of the same command in Section 13.1.
    // Protocol - Block
    // ---
}

fn convert_temperature(raw: u16) -> f32 {
    raw as f32 / 10.0 - 273.15
}

impl<E> From<E> for Error<E> {
    fn from(error: E) -> Self {
        Error::I2cError(error)
    }
}
