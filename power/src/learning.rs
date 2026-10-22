//! Measuring the pack's capacity: one full charge, then one discharge to empty, counted from
//! the gauge's current.
//!
//! The BQ4050 learns its full charge capacity on its own from a cycle like this, but moves it
//! by at most -256 / +512 mAh per cycle. The measured value can be written to the gauge once,
//! so the state of charge is right from the next cycle on.

/// Discharge current that starts the measurement after the full charge
pub const START_A: f32 = 0.05;
/// Charge that ends a discharge, as for the gauge: "a valid charge is 10 mAh into the battery"
/// (BQ4050 TRM 6.3)
pub const MAX_CHARGE_MAH: f32 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Phase {
  /// Not measuring
  Idle,
  /// Waiting for the gauge to report a full charge
  WaitFull,
  /// Full: waiting for the discharge to start
  Full,
  /// Counting the discharge
  Discharging { mah: f32, charged_mah: f32 },
  /// The gauge reported empty: the measured capacity
  Done { mah: u16 },
  /// Charge went into the pack during the discharge
  Interrupted,
}

/// What the gauge reports in one update
#[derive(Debug, Clone, Copy, Default)]
pub struct GaugeState {
  /// GaugingStatus()[FC]: fully charged
  pub full: bool,
  /// GaugingStatus()[EDV0]: empty
  pub empty: bool,
  /// Pack current, A: positive while charging
  pub pack_a: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Measurement {
  phase: Phase,
}

impl Measurement {
  pub const fn new() -> Self {
    Self { phase: Phase::Idle }
  }

  pub fn phase(&self) -> Phase {
    self.phase
  }

  /// Starts over: the next full charge begins a measurement
  pub fn start(&mut self) {
    self.phase = Phase::WaitFull;
  }

  pub fn stop(&mut self) {
    self.phase = Phase::Idle;
  }

  /// Takes one update; `elapsed_s` is the time since the previous one
  pub fn update(&mut self, gauge: &GaugeState, elapsed_s: u32) {
    let mah = gauge.pack_a * elapsed_s as f32 / 3.6;
    self.phase = match self.phase {
      Phase::WaitFull if gauge.full => Phase::Full,
      Phase::Full if gauge.pack_a < -START_A => Phase::Discharging { mah: -mah, charged_mah: 0.0 },
      Phase::Discharging { mah: discharged, charged_mah } => {
        let charged_mah = charged_mah + mah.max(0.0);
        let discharged = discharged + (-mah).max(0.0);
        if charged_mah > MAX_CHARGE_MAH {
          Phase::Interrupted
        } else if gauge.empty {
          Phase::Done { mah: discharged as u16 }
        } else {
          Phase::Discharging { mah: discharged, charged_mah }
        }
      }
      phase => phase,
    };
  }
}

impl Default for Measurement {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn state(pack_a: f32) -> GaugeState {
    GaugeState { full: false, empty: false, pack_a }
  }

  /// Full charge, then a 1 A discharge for an hour, then empty
  fn measured() -> Measurement {
    let mut m = Measurement::new();
    m.start();
    m.update(&state(0.5), 1);
    assert_eq!(m.phase(), Phase::WaitFull);
    m.update(&GaugeState { full: true, ..state(0.1) }, 1);
    assert_eq!(m.phase(), Phase::Full);
    // Charger unplugged: idle, then the load starts
    m.update(&state(-0.01), 60);
    assert_eq!(m.phase(), Phase::Full);
    for _ in 0..3600 {
      m.update(&state(-1.0), 1);
    }
    m
  }

  #[test]
  fn counts_the_discharge_from_full_to_empty() {
    let mut m = measured();
    m.update(&GaugeState { empty: true, ..state(-1.0) }, 1);
    // 3600 s and the step to empty, both at 1 A
    assert_eq!(m.phase(), Phase::Done { mah: 1000 });
  }

  #[test]
  fn charging_during_the_discharge_interrupts() {
    let mut m = measured();
    // 5 mAh in: still fine
    m.update(&state(1.0), 18);
    assert!(matches!(m.phase(), Phase::Discharging { .. }));
    // 10 mAh more
    m.update(&state(1.0), 36);
    assert_eq!(m.phase(), Phase::Interrupted);
  }

  #[test]
  fn nothing_happens_until_started() {
    let mut m = Measurement::new();
    m.update(&GaugeState { full: true, ..state(-1.0) }, 1);
    assert_eq!(m.phase(), Phase::Idle);
  }

  #[test]
  fn empty_before_full_is_ignored() {
    let mut m = Measurement::new();
    m.start();
    m.update(&GaugeState { empty: true, ..state(-1.0) }, 1);
    assert_eq!(m.phase(), Phase::WaitFull);
  }
}
