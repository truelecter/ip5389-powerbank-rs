/// Ports on the INA3221 channels, in channel order: the left and right USB-C modules and the
/// DC port
pub const PORT_NAMES: [&str; 3] = ["USB1", "USB2", "DC"];

/// One output port measured by the INA3221
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Port {
  pub volts: f32,
  /// Positive while powering a device
  pub amps: f32,
}

impl Port {
  pub fn watts(&self) -> f32 {
    self.volts * self.amps
  }
}

/// One second of readings: everything a layout can show
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Snapshot {
  /// In [`PORT_NAMES`] order
  pub ports: [Port; 3],
  /// The charger chip's USB-C and USB-A ports, from the current balance on the battery side:
  /// positive while powering a device, negative while a charger charges the pack
  pub main_amps: f32,
  pub pack_volts: f32,
  /// Positive while charging
  pub pack_amps: f32,
  /// Cells 1 to 4
  pub cell_volts: [f32; 4],
  /// State of charge, %
  pub charge: u8,
  /// State of health, %
  pub health: u8,
  pub cycles: u16,
  /// Capacity learning state for layouts with room for it, such as `LRN`; empty when there
  /// is nothing to report
  pub status: &'static str,
  /// Energy left in the pack
  pub remaining_wh: f32,
  /// Minutes until empty while discharging, until full while charging
  pub minutes_left: Option<u16>,
  /// Cell temperature, °C
  pub pack_temp: f32,
  /// Gauge chip temperature, °C
  pub gauge_temp: f32,
  pub uptime_s: u32,
}

impl Snapshot {
  /// Power of the main ports, on the battery side: their own voltage isn't measured
  pub fn main_watts(&self) -> f32 {
    self.pack_volts * self.main_amps
  }

  pub fn pack_watts(&self) -> f32 {
    self.pack_volts * self.pack_amps
  }
}
