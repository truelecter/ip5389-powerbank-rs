//! Switching the board off after a stretch with nothing going on.
//!
//! The board can turn its own power off (PB4, see the firmware), and a long button press or
//! plugging in a device turns it back on. Off it draws next to nothing; idle it draws 8 mA.
//!
//! With a charger connected the board stays on until the charger goes away, and plugging in a
//! device doesn't wake it then: never switch off while the battery isn't powering the board.

use crate::{main_port_a, Readings, ACTIVE_A, MAIN_ACTIVE_A};

/// The timeouts to choose from, in seconds; `None` never switches off
pub const CHOICES: [Option<u32>; 4] = [None, Some(10 * 60), Some(30 * 60), Some(2 * 60 * 60)];
/// The battery powers the board when it delivers more than this. Idle the board draws at
/// least 7 mA; with a charger connected the gauge reads 0.
pub const ON_BATTERY_A: f32 = 0.004;

#[derive(Debug, Clone, Copy, Default)]
pub struct AutoOff {
  idle_s: u32,
}

impl AutoOff {
  pub const fn new() -> Self {
    Self { idle_s: 0 }
  }

  /// Takes one update: `event` is a button press, `busy` something that must not be
  /// interrupted, such as a capacity measurement. Returns whether to switch off now.
  pub fn update(&mut self, r: &Readings, event: bool, busy: bool, elapsed_s: u32, timeout_s: Option<u32>) -> bool {
    let ports_active = r.ports_a.iter().any(|&a| a > ACTIVE_A) || main_port_a(r).abs() > MAIN_ACTIVE_A;
    let on_battery = r.pack_a < -ON_BATTERY_A;
    if event || busy || ports_active || !on_battery {
      self.idle_s = 0;
      return false;
    }
    self.idle_s += elapsed_s;
    timeout_s.is_some_and(|timeout| self.idle_s >= timeout)
  }
}

/// The choice after `current` in [`CHOICES`], wrapping around
pub fn next_choice(current: Option<u32>) -> Option<u32> {
  let i = CHOICES.iter().position(|&c| c == current).unwrap_or(0);
  CHOICES[(i + 1) % CHOICES.len()]
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::BOARD_A;

  /// The board idle on battery
  fn idle() -> Readings {
    Readings { ports_a: [0.0; 3], pack_a: -BOARD_A }
  }

  fn run(auto: &mut AutoOff, r: &Readings, seconds: u32, timeout: Option<u32>) -> bool {
    (0..seconds).any(|_| auto.update(r, false, false, 1, timeout))
  }

  #[test]
  fn switches_off_after_the_timeout_on_battery() {
    let mut auto = AutoOff::new();
    assert!(!run(&mut auto, &idle(), 599, Some(600)));
    assert!(auto.update(&idle(), false, false, 1, Some(600)));
  }

  #[test]
  fn never_with_external_power() {
    let mut auto = AutoOff::new();
    let charger = Readings { pack_a: 0.0, ..idle() };
    assert!(!run(&mut auto, &charger, 3600, Some(600)));
  }

  #[test]
  fn activity_restarts_the_wait() {
    let mut auto = AutoOff::new();
    run(&mut auto, &idle(), 500, Some(600));
    let phone = Readings { ports_a: [0.0, 1.0, 0.0], pack_a: -1.0 };
    auto.update(&phone, false, false, 1, Some(600));
    assert!(!run(&mut auto, &idle(), 599, Some(600)));
    auto.update(&idle(), true, false, 1, Some(600));
    assert!(!run(&mut auto, &idle(), 599, Some(600)));
    assert!(!run(&mut auto, &idle(), 600, None));
  }

  #[test]
  fn busy_keeps_it_on() {
    let mut auto = AutoOff::new();
    assert!(!(0..3600).any(|_| auto.update(&idle(), false, true, 1, Some(600))));
  }

  #[test]
  fn choices_cycle() {
    assert_eq!(next_choice(None), Some(600));
    assert_eq!(next_choice(Some(7200)), None);
  }
}
