#![no_std]
#![no_main]
use cortex_m::peripheral::DWT;
use embedded_hal::delay::DelayNs;
use embedded_hal_bus::spi::ExclusiveDevice;

use embedded_graphics::{pixelcolor::Rgb565, prelude::*};

use graphics::volttable::VoltTable;
use ina3221::INA3221;
use panic_rtt_target as _;
use rtt_target::{rprintln, rtt_init_print};

use stm32f1xx_hal::{
  i2c::{BlockingI2c, DutyCycle, Mode},
  pac::{Peripherals, SPI1},
  prelude::*,
  rcc,
  spi::SpiExt,
};

use mipidsi::{
  interface::SpiInterface,
  models::ST7789,
  options::{ColorInversion, ColorOrder, Orientation},
  Builder,
};
use peripherals::{bq4050, ina3221, soft_i2c::SoftI2c};

#[cortex_m_rt::entry]
fn main() -> ! {
  rtt_init_print!();
  rprintln!("Hello, world!");

  let cp = cortex_m::Peripherals::take().unwrap();
  let dp = Peripherals::take().unwrap();

  let mut flash = dp.FLASH.constrain();

  // HICK/2 * 60 = 240 MHz, APB1 and APB2 at their 120 MHz maximum
  let mut rcc = dp.RCC.freeze(
    rcc::Config::hsi()
      .sysclk(240.MHz())
      .hclk(240.MHz())
      .pclk1(120.MHz())
      .pclk2(120.MHz()),
    &mut flash.acr,
  );

  unsafe {
    let demcr = 0xE000EDFC as *mut u32;
    let old = demcr.read_volatile();
    let new = old | 0x01000000;
    // cp.DWT.enable_cycle_counter();
    demcr.write_volatile(new);
  }

  let mut delay = cp.SYST.delay(&rcc.clocks);

  let mut gpioa = dp.GPIOA.split(&mut rcc);
  let mut gpiob = dp.GPIOB.split(&mut rcc);

  rprintln!("dwt {}", DWT::cycle_count());

  // SPI1
  let sck = gpioa.pa5.into_alternate_push_pull(&mut gpioa.crl);
  let mosi = gpioa.pa7.into_alternate_push_pull(&mut gpioa.crl);
  let cs = gpioa.pa3.into_push_pull_output(&mut gpioa.crl);

  // APB2 120 MHz / 4
  let spi = dp.SPI1.spi(
    (Some(sck), SPI1::NoMiso, Some(mosi)),
    embedded_hal::spi::MODE_3.into(),
    30.MHz(),
    &mut rcc,
  );

  rprintln!("dwt {}", DWT::cycle_count());

  // let bl = gpioa.pa8.into_push_pull_output(&mut gpioa.crh);
  let rst = gpioa
    .pa1
    .into_push_pull_output_with_state(&mut gpioa.crl, stm32f1xx_hal::gpio::PinState::High);
  let dc = gpioa
    .pa2
    .into_push_pull_output_with_state(&mut gpioa.crl, stm32f1xx_hal::gpio::PinState::High);

  let spi = ExclusiveDevice::new_no_delay(spi, cs).unwrap();
  let mut display_buffer = [0u8; 512];
  let di = SpiInterface::new(spi, dc, &mut display_buffer);
  let mut display = Builder::new(ST7789, di)
    .display_size(240, 240)
    .orientation(Orientation::new())
    .invert_colors(ColorInversion::Inverted)
    .color_order(ColorOrder::Rgb)
    .reset_pin(rst)
    .init(&mut delay)
    .unwrap();

  rprintln!("Display Init");

  display.clear(Rgb565::RED).unwrap();

  let mut table = VoltTable::new(Point::zero(), 240, Rgb565::BLACK, Rgb565::BLACK);

  table.draw_static(&mut display).unwrap();
  table.update([(Some(0.0), 0.0, 0.0); 4], &mut display).unwrap();

  rprintln!("BQ4050 init");
  // Bit-banged: the I2C peripheral garbles reads while the BQ4050 stretches the clock
  let scl = gpiob
    .pb10
    .into_open_drain_output_with_state(&mut gpiob.crh, stm32f1xx_hal::gpio::PinState::High);
  let sda = gpiob
    .pb11
    .into_open_drain_output_with_state(&mut gpiob.crh, stm32f1xx_hal::gpio::PinState::High);
  let smbus = SoftI2c::new(scl, sda, dp.TIM3.delay_us(&mut rcc), 100_000);

  let mut bq4050 = bq4050::BQ4050::new(smbus);
  rprintln!("BQ4050 init finished");

  let i2c = BlockingI2c::new(
    dp.I2C1,
    (gpiob.pb6, gpiob.pb7),
    // Mode::Standard {
    //   frequency: 100.kHz(),
    // },
    Mode::Fast { frequency: 400.kHz(), duty_cycle: DutyCycle::Ratio2to1 },
    &mut rcc,
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
      Ok(v) => { rprintln!("b1 {}", v); }
      Err(e) => { rprintln!("{:#?}", e); }
    };

    match ina3221.bus_voltage(ina3221::Channel::Ch2) {
      Ok(v) => { rprintln!("b2 {}", v); }
      Err(e) => { rprintln!("{:#?}", e); }
    };

    match ina3221.bus_voltage(ina3221::Channel::Ch3) {
      Ok(v) => { rprintln!("b3 {}", v); }
      Err(e) => { rprintln!("{:#?}", e); }
    };

    match ina3221.shunt_voltage(ina3221::Channel::Ch1) {
      Ok(v) => { rprintln!("s1 {}", v); }
      Err(e) => { rprintln!("{:#?}", e); }
    };

    match ina3221.shunt_voltage(ina3221::Channel::Ch2) {
      Ok(v) => { rprintln!("s2 {}", v); }
      Err(e) => { rprintln!("{:#?}", e); }
    };

    match ina3221.shunt_voltage(ina3221::Channel::Ch3) {
      Ok(v) => { rprintln!("s3 {}", v); }
      Err(e) => { rprintln!("{:#?}", e); }
    };

    match bq4050.get_temperature() {
      Ok(temp) => { rprintln!("temp {}", temp); }
      Err(e) => { rprintln!("temp {:?}", e); }
    };
    match bq4050.get_voltage() {
      Ok(v) => { rprintln!("voltage {} mV", v); }
      Err(e) => { rprintln!("voltage {:?}", e); }
    };
    match bq4050.get_current() {
      Ok(v) => { rprintln!("current {} mA", v); }
      Err(e) => { rprintln!("current {:?}", e); }
    };
    match bq4050.get_relative_state_of_charge() {
      Ok(v) => { rprintln!("rsoc {} %", v); }
      Err(e) => { rprintln!("rsoc {:?}", e); }
    };
    match (bq4050.get_cell_voltage_1(), bq4050.get_cell_voltage_2(), bq4050.get_cell_voltage_3(), bq4050.get_cell_voltage_4()) {
      (Ok(a), Ok(b), Ok(c), Ok(d)) => { rprintln!("cells {} {} {} {} mV", a, b, c, d); }
      e => { rprintln!("cells {:?}", (e.0.err(), e.1.err(), e.2.err(), e.3.err())); }
    };

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
      Err(e) => { rprintln!("{:#?}", e); }
    };
    DelayNs::delay_ms(&mut delay, 50);

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
      Err(e) => { rprintln!("{:#?}", e); }
    };
    // match bq4050.get_cycle_count() {
    //   Ok(count) => {
    //     rprintln!("cycles 0b{:b} => {}", count, count);
    //   },
    //   Err(e) => rprintln!("{:#?}", e),
    // };

    DelayNs::delay_ms(&mut delay, 2000);
  }
}
