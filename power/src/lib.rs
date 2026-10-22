//! When the screen is bright, dimmed or off.
//!
//! The screen wakes up on the button and on port activity: a port starting or stopping to
//! draw current, a port's current changing a lot, or the pack starting or stopping to charge.
//! Without a wake event it dims after [`DIM_AFTER_S`] and turns off after [`OFF_AFTER_S`],
//! except while the pack charges: then it stays dimmed.
//!
//! The main USB-C and USB-A ports belong to the charger chip, which the firmware can't read
//! and which drives no MCU pin. Their current is what the battery delivers or takes beyond
//! the measured ports and the board itself, see [`main_port_a`].
#![no_std]

pub mod button;
pub mod learning;

/// Seconds without a wake event before the backlight dims
pub const DIM_AFTER_S: u32 = 15;
/// Seconds without a wake event before the screen turns off, unless the pack charges
pub const OFF_AFTER_S: u32 = 30;
/// A port, or the pack while charging, counts as active above this current...
pub const ACTIVE_A: f32 = 0.10;
/// ...and as idle again below this one, so a current near the threshold doesn't flap
pub const IDLE_A: f32 = 0.05;
/// The main ports' current is a difference of two meters, so it gets wider thresholds
pub const MAIN_ACTIVE_A: f32 = 0.20;
pub const MAIN_IDLE_A: f32 = 0.10;
/// A port current change this large since the last wake event wakes the screen
pub const JUMP_A: f32 = 0.20;
/// Battery current of the board itself: 8 mA with the screen off, 22 mA bright
pub const BOARD_A: f32 = 0.012;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
  Bright,
  Dim,
  Off,
}

/// Currents measured in one update
#[derive(Debug, Clone, Copy, Default)]
pub struct Readings {
  /// Current of each output port, in A
  pub ports_a: [f32; 3],
  /// Battery pack current, in A: positive while charging
  pub pack_a: f32,
}

/// Current through the main ports, in A: positive while they power a device, negative while
/// a charger on them charges the pack
pub fn main_port_a(readings: &Readings) -> f32 {
  -readings.pack_a - readings.ports_a.iter().sum::<f32>() - BOARD_A
}

pub struct ScreenPower {
  idle_s: u32,
  /// The measured ports, then the main ports
  ports_active: [bool; 4],
  charging: bool,
  ports_at_wake: [f32; 4],
}

impl Default for ScreenPower {
  fn default() -> Self {
    Self::new()
  }
}

impl ScreenPower {
  /// Starts bright, as if just woken
  pub const fn new() -> Self {
    Self { idle_s: 0, ports_active: [false; 4], charging: false, ports_at_wake: [0.0; 4] }
  }

  /// Takes new readings and returns the screen level. Call it once a second with
  /// `elapsed_s = 1`, and right after a button press with `button = true` and `elapsed_s = 0`.
  pub fn update(&mut self, readings: &Readings, button: bool, elapsed_s: u32) -> Level {
    let mut wake = button;

    let ports = [readings.ports_a[0], readings.ports_a[1], readings.ports_a[2], main_port_a(readings)];
    for (i, &amps) in ports.iter().enumerate() {
      let (on, off) = if i == 3 { (MAIN_ACTIVE_A, MAIN_IDLE_A) } else { (ACTIVE_A, IDLE_A) };
      if switch(&mut self.ports_active[i], amps.abs(), on, off) {
        wake = true;
      }
      if (amps - self.ports_at_wake[i]).abs() > JUMP_A {
        wake = true;
      }
    }
    if switch(&mut self.charging, readings.pack_a, ACTIVE_A, IDLE_A) {
      wake = true;
    }

    if wake {
      self.idle_s = 0;
      self.ports_at_wake = ports;
    } else {
      self.idle_s = self.idle_s.saturating_add(elapsed_s);
    }

    if self.idle_s < DIM_AFTER_S {
      Level::Bright
    } else if self.charging || self.idle_s < OFF_AFTER_S {
      Level::Dim
    } else {
      Level::Off
    }
  }
}

/// Updates an active flag from a current with hysteresis; returns whether it changed
fn switch(active: &mut bool, amps: f32, on: f32, off: f32) -> bool {
  let now = if *active { amps > off } else { amps > on };
  let changed = now != *active;
  *active = now;
  changed
}

#[cfg(test)]
mod tests {
  use super::*;

  /// A device on output port 1; the battery delivers it plus the board's own current
  fn load(port_a: f32) -> Readings {
    Readings { ports_a: [port_a, 0.0, 0.0], pack_a: -port_a - BOARD_A }
  }

  /// A device on the main port: the battery current with nothing on the measured ports
  fn main_load(amps: f32) -> Readings {
    Readings { ports_a: [0.0; 3], pack_a: -amps - BOARD_A }
  }

  fn charging(pack_a: f32) -> Readings {
    Readings { ports_a: [0.0; 3], pack_a }
  }

  /// Runs `seconds` one-second updates with the same readings and returns the last level
  fn run(power: &mut ScreenPower, readings: Readings, seconds: u32) -> Level {
    let mut level = Level::Bright;
    for _ in 0..seconds {
      level = power.update(&readings, false, 1);
    }
    level
  }

  #[test]
  fn discharging_dims_then_turns_off() {
    let mut power = ScreenPower::new();
    assert_eq!(run(&mut power, load(1.0), DIM_AFTER_S), Level::Bright);
    assert_eq!(run(&mut power, load(1.0), 1), Level::Dim);
    assert_eq!(run(&mut power, load(1.0), OFF_AFTER_S - DIM_AFTER_S - 1), Level::Dim);
    assert_eq!(run(&mut power, load(1.0), 1), Level::Off);
  }

  #[test]
  fn charging_dims_but_never_turns_off() {
    let mut power = ScreenPower::new();
    assert_eq!(run(&mut power, charging(1.5), DIM_AFTER_S), Level::Bright);
    assert_eq!(run(&mut power, charging(1.5), 1000), Level::Dim);
  }

  #[test]
  fn button_wakes_a_dark_screen() {
    let mut power = ScreenPower::new();
    assert_eq!(run(&mut power, Readings::default(), 60), Level::Off);
    assert_eq!(power.update(&Readings::default(), true, 0), Level::Bright);
    assert_eq!(run(&mut power, Readings::default(), DIM_AFTER_S - 1), Level::Bright);
  }

  #[test]
  fn plugging_and_unplugging_a_device_wakes() {
    let mut power = ScreenPower::new();
    run(&mut power, Readings::default(), 60);
    assert_eq!(power.update(&load(0.15), false, 1), Level::Bright);
    run(&mut power, load(0.15), 60);
    assert_eq!(power.update(&Readings::default(), false, 1), Level::Bright);
  }

  #[test]
  fn charger_connect_and_disconnect_wake() {
    let mut power = ScreenPower::new();
    run(&mut power, Readings::default(), 60);
    assert_eq!(power.update(&charging(1.0), false, 1), Level::Bright);
    run(&mut power, charging(1.0), 60);
    assert_eq!(power.update(&charging(-0.01), false, 1), Level::Bright);
  }

  #[test]
  fn current_jump_wakes_but_slow_drift_does_not() {
    let mut power = ScreenPower::new();
    run(&mut power, load(1.0), 60);
    // 0.15 A in small steps stays within the jump threshold of the value at the last wake
    for step in 1..=3 {
      assert_eq!(power.update(&load(1.0 + step as f32 * 0.05), false, 1), Level::Off);
    }
    assert_eq!(power.update(&load(1.25), false, 1), Level::Bright);
  }

  #[test]
  fn current_near_the_threshold_does_not_flap() {
    let mut power = ScreenPower::new();
    run(&mut power, load(0.15), 60);
    for &amps in &[0.07, 0.12, 0.06, 0.11] {
      assert_eq!(power.update(&load(amps), false, 1), Level::Off);
    }
  }

  #[test]
  fn main_port_current_is_what_the_measured_ports_dont_explain() {
    assert!(main_port_a(&load(2.0)).abs() < 1e-6);
    assert!((main_port_a(&main_load(1.5)) - 1.5).abs() < 1e-6);
    assert!(main_port_a(&charging(2.0)) < -2.0);
  }

  #[test]
  fn device_on_the_main_port_wakes() {
    let mut power = ScreenPower::new();
    run(&mut power, Readings::default(), 60);
    assert_eq!(power.update(&main_load(0.5), false, 1), Level::Bright);
    assert_eq!(run(&mut power, main_load(0.5), 60), Level::Off);
    assert_eq!(power.update(&Readings::default(), false, 1), Level::Bright);
  }

  #[test]
  fn main_port_ignores_meter_mismatch_below_its_threshold() {
    let mut power = ScreenPower::new();
    run(&mut power, Readings::default(), 60);
    for &amps in &[0.15, 0.05, 0.18, 0.12] {
      assert_eq!(power.update(&main_load(amps), false, 1), Level::Off);
    }
  }
}
