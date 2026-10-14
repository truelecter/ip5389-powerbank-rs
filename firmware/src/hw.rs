#![no_std]
#![no_main]
use cortex_m::peripheral::DWT;
use display_interface_spi::SPIInterface;

use embedded_graphics::{pixelcolor::Rgb565, prelude::*};

use graphics::volttable::VoltTable;
use ina3221::INA3221;
use panic_rtt_target as _;
use rtt_target::{rprintln, rtt_init_print};

use stm32f1xx_hal::{
  i2c::{BlockingI2c, Mode, DutyCycle},
  pac::Peripherals,
  prelude::*,
  spi::{NoMiso, Spi},
};

use mipidsi::Builder;
use peripherals::{self, bq4050};

#[cortex_m_rt::entry]
fn main() -> ! {
  rtt_init_print!();
  rprintln!("Hello, world!");

  let cp = cortex_m::Peripherals::take().unwrap();
  let dp = Peripherals::take().unwrap();

  let mut flash = dp.FLASH.constrain();
  let rcc = dp.RCC.constrain();

  let clocks = rcc
    .cfgr
    .hclk(72.MHz())
    .sysclk(72.MHz())
    .freeze(&mut flash.acr);

  unsafe {
    let demcr = 0xE000EDFC as *mut u32;
    let old = demcr.read_volatile();
    let new = old | 0x01000000;
    // cp.DWT.enable_cycle_counter();
    demcr.write_volatile(new);
  }

  let mut delay = cp.SYST.delay(&clocks);

  let mut afio = dp.AFIO.constrain();
  let mut gpioa = dp.GPIOA.split();
  let mut gpiob = dp.GPIOB.split();

  rprintln!("dwt {}", DWT::cycle_count());

  // SPI1
  let sck = gpioa.pa5.into_alternate_push_pull(&mut gpioa.crl);
  let mosi = gpioa.pa7.into_alternate_push_pull(&mut gpioa.crl);
  let cs = gpioa.pa3.into_push_pull_output(&mut gpioa.crl);

  let spi = Spi::spi1(
    dp.SPI1,
    (sck, NoMiso, mosi),
    &mut afio.mapr,
    embedded_hal::spi::MODE_3,
    48.MHz(),
    clocks,
  );

  rprintln!("dwt {}", DWT::cycle_count());

  // let bl = gpioa.pa8.into_push_pull_output(&mut gpioa.crh);
  let rst = gpioa
    .pa1
    .into_push_pull_output_with_state(&mut gpioa.crl, stm32f1xx_hal::gpio::PinState::High);
  let dc = gpioa
    .pa2
    .into_push_pull_output_with_state(&mut gpioa.crl, stm32f1xx_hal::gpio::PinState::High);

  let di = SPIInterface::new(spi, dc, cs);
  let mut display = Builder::st7789(di)
    .with_display_size(240, 240)
    .with_orientation(mipidsi::Orientation::Portrait(false))
    .with_invert_colors(mipidsi::ColorInversion::Inverted)
    .with_color_order(mipidsi::ColorOrder::Rgb)
    .init(&mut delay, Some(rst))
    .unwrap();

  rprintln!("Display Init");

  display.clear(Rgb565::RED).unwrap();

  let table = VoltTable::new(Point::zero(), 240);

  table.draw_static(&mut display).unwrap();
  table.draw(&mut display).unwrap();

  rprintln!("BQ4050 init");
  // look into https://github.com/cs2dsb/i2c_hung_fix.rs/blob/master/src/lib.rs for freeze fix
  let pb10 = gpiob.pb10.into_alternate_open_drain(&mut gpiob.crh);
  let pb11 = gpiob.pb11.into_alternate_open_drain(&mut gpiob.crh);

  let i2c2 = BlockingI2c::i2c2(
    dp.I2C2,
    (pb10, pb11),
    Mode::Standard {
      frequency: 100.kHz(),
    },
    clocks,
    100_000,
    5,
    100_000,
    100_000,
  );

  let mut bq4050 = bq4050::BQ4050::new(i2c2);
  rprintln!("BQ4050 init finished");

  let pb6 = gpiob.pb6.into_alternate_open_drain(&mut gpiob.crl);
  let pb7 = gpiob.pb7.into_alternate_open_drain(&mut gpiob.crl);

  let i2c = BlockingI2c::i2c1(
    dp.I2C1,
    (pb6, pb7),
    &mut afio.mapr,
    // Mode::Standard {
    //   frequency: 100.kHz(),
    // },
    Mode::Fast { frequency: 400_i32.kHz(), duty_cycle: DutyCycle::Ratio2to1 },
    clocks,
    50_000,
    5,
    20_000,
    100_000,
  );

  let mut ina3221 = INA3221::new(i2c, ina3221::AddressPin::Gnd);

  ina3221.reset().unwrap();
  ina3221.enable_channel(ina3221::Channel::Ch1, true).unwrap();
  ina3221.enable_channel(ina3221::Channel::Ch2, true).unwrap();
  ina3221.enable_channel(ina3221::Channel::Ch3, true).unwrap();

  rprintln!("INA3221 init finished");

  loop {
    match ina3221.bus_voltage(ina3221::Channel::Ch1) {
      Ok(v) => rprintln!("b1 {}", v),
      Err(e) => rprintln!("{:#?}", e),
    };

    match ina3221.bus_voltage(ina3221::Channel::Ch2) {
      Ok(v) => rprintln!("b2 {}", v),
      Err(e) => rprintln!("{:#?}", e),
    };

    match ina3221.bus_voltage(ina3221::Channel::Ch3) {
      Ok(v) => rprintln!("b3 {}", v),
      Err(e) => rprintln!("{:#?}", e),
    };

    match ina3221.shunt_voltage(ina3221::Channel::Ch1) {
      Ok(v) => rprintln!("s1 {}", v),
      Err(e) => rprintln!("{:#?}", e),
    };

    match ina3221.shunt_voltage(ina3221::Channel::Ch2) {
      Ok(v) => rprintln!("s2 {}", v),
      Err(e) => rprintln!("{:#?}", e),
    };

    match ina3221.shunt_voltage(ina3221::Channel::Ch3) {
      Ok(v) => rprintln!("s3 {}", v),
      Err(e) => rprintln!("{:#?}", e),
    };

    // match bq4050.get_temperature() {
    //   Ok(temp) => rprintln!("temp {}", temp),
    //   Err(e) => rprintln!("{:#?}", e),
    // };

    match bq4050.get_da_status_2() {
      Ok(temp) => {
        rprintln!("temp1 {}", temp.ts1_temperature);
        rprintln!("temp2 {}", temp.ts2_temperature);
        rprintln!("temp3 {}", temp.ts3_temperature);
        rprintln!("temp4 {}", temp.ts4_temperature);
        rprintln!("cell_temp {}", temp.cell_temperature);
        rprintln!("fet_temp {}", temp.fet_temperature);
        rprintln!("int_temp 0b{:b} => {}", Into::<u16>::into(temp.int_temperature), Into::<u16>::into(temp.int_temperature));
      },
      Err(e) => rprintln!("{:#?}", e),
    };
    delay.delay_ms(50 as u16);

    match bq4050.get_da_status_1() {
      Ok(temp) => {
        rprintln!("cell_voltage_1 {}", temp.cell_voltage_1);
        rprintln!("cell_voltage_2 {}", temp.cell_voltage_2);
        rprintln!("cell_voltage_3 {}", temp.cell_voltage_3);
        rprintln!("cell_voltage_4 {}", temp.cell_voltage_4);
        rprintln!("bat_voltage {}", temp.bat_voltage);
        rprintln!("pack_voltage {}", temp.pack_voltage);
        rprintln!("cell_current_1 {}", temp.cell_current_1);
        rprintln!("cell_current_2 {}", temp.cell_current_2);
        rprintln!("cell_current_3 {}", temp.cell_current_3);
        rprintln!("cell_current_4 {}", temp.cell_current_4);
        rprintln!("cell_power_1 {}", temp.cell_power_1);
        rprintln!("cell_power_2 {}", temp.cell_power_2);
        rprintln!("cell_power_3 {}", temp.cell_power_3);
        rprintln!("cell_power_4 {}", temp.cell_power_4);
        rprintln!("power {}", temp.power);
        rprintln!("average_power {}", temp.average_power);
      },
      Err(e) => rprintln!("{:#?}", e),
    };
    // match bq4050.get_cycle_count() {
    //   Ok(count) => {
    //     rprintln!("cycles 0b{:b} => {}", count, count);
    //   },
    //   Err(e) => rprintln!("{:#?}", e),
    // };

    delay.delay_ms(2000 as u16);
  }
}
