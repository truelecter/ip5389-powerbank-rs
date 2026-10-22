//! The battery page: gauge details and the capacity measurement, as a [`Menu`].
//!
//! Short press: next action. Long press: run the selected action.

use embedded_graphics::{draw_target::DrawTarget, pixelcolor::Rgb565};

use peripherals::bq4050::{gauging, operation};
use power::button::Press;
use power::learning::Phase;

use crate::menu::{Menu, COLUMNS};
use crate::textual::format::{line, Line};
use crate::Snapshot;

type Text = Line<COLUMNS>;

/// Gauge values only the battery page shows
#[derive(Clone, Copy, Debug, Default)]
pub struct Details {
  pub full_mah: u16,
  pub design_mah: u16,
  pub remaining_mah: u16,
  pub max_error: u16,
  /// GaugingStatus()
  pub gauging: u16,
  /// OperationStatus()
  pub operation: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
  StartMeasuring,
  StopMeasuring,
  Save(u16),
  /// The board's power switch: off at once on battery, with a charger once it goes away
  PowerOff,
  /// Gauge shutdown: the gauge turns the battery off until a charger is connected
  GaugeShutdown,
  /// Steps the automatic power-off through its timeouts
  AutoOff,
  Back,
}

/// Lines of the page
const LINES: usize = 14;
/// First line of the actions
const ACTIONS_LINE: usize = 9;

pub struct BatteryPage {
  menu: Menu,
  selected: usize,
  /// The outcome of the last save
  message: &'static str,
}

impl BatteryPage {
  pub fn new() -> Self {
    Self { menu: Menu::new(), selected: 0, message: "" }
  }

  /// The actions in this measurement phase, and how many there are. Powering off isn't
  /// offered while measuring: the measurement lives in RAM.
  pub fn actions(phase: Phase) -> ([Action; 5], usize) {
    use Action::*;
    match phase {
      Phase::Idle | Phase::Interrupted => ([StartMeasuring, AutoOff, PowerOff, GaugeShutdown, Back], 5),
      Phase::Done { mah } => ([Save(mah), StartMeasuring, AutoOff, PowerOff, Back], 5),
      _ => ([StopMeasuring, AutoOff, Back, Back, Back], 3),
    }
  }

  /// A short press selects the next action, a long press returns the selected one
  pub fn press(&mut self, press: Press, phase: Phase) -> Option<Action> {
    let (actions, count) = Self::actions(phase);
    self.selected %= count;
    match press {
      Press::Short => {
        self.selected = (self.selected + 1) % count;
        None
      }
      Press::Long => Some(actions[self.selected]),
    }
  }

  pub fn set_message(&mut self, message: &'static str) {
    self.message = message;
  }

  /// `auto_off_s` is the automatic power-off timeout, `None` when off
  pub fn update<D: DrawTarget<Color = Rgb565>>(
    &mut self,
    s: &Snapshot,
    d: &Details,
    phase: Phase,
    auto_off_s: Option<u32>,
    target: &mut D,
  ) -> Result<(), D::Error> {
    let lines = self.lines(s, d, phase, auto_off_s);
    let rows: [&str; LINES] = core::array::from_fn(|i| lines[i].as_str());
    let selected = ACTIONS_LINE + self.selected % Self::actions(phase).1;
    self.menu.update("Battery", &rows, Some(selected), target)
  }

  fn lines(&self, s: &Snapshot, d: &Details, phase: Phase, auto_off_s: Option<u32>) -> [Text; LINES] {
    let mut lines = [Text::new(); LINES];
    lines[0] = line(format_args!("Full     {} mAh", d.full_mah));
    lines[1] = line(format_args!("Design   {} mAh", d.design_mah));
    lines[2] = line(format_args!("Left     {} mAh  {} %", d.remaining_mah, s.charge));
    lines[3] = line(format_args!("Error    {} %   Cycles {}", d.max_error, s.cycles));
    lines[4] = flags(d.gauging);
    lines[5] = line(format_args!("Access   {}  {:#010x}", access(d.operation), d.operation));
    let c = s.cell_volts;
    lines[6] = line(format_args!("Cells    {:.2} {:.2} {:.2} {:.2}", c[0], c[1], c[2], c[3]));
    let (state, hint) = describe(phase);
    lines[7] = line(format_args!("Measure  {}", state.as_str()));
    // The outcome of the last action takes the place of the hint
    let note = if self.message.is_empty() { hint } else { self.message };
    lines[8] = line(format_args!("         {}", note));
    let (actions, count) = Self::actions(phase);
    for (row, action) in lines[ACTIONS_LINE..].iter_mut().zip(&actions[..count]) {
      *row = match action {
        Action::StartMeasuring => line(format_args!("> Start measuring")),
        Action::StopMeasuring => line(format_args!("> Stop measuring")),
        Action::Save(mah) => line(format_args!("> Save {} mAh to gauge", mah)),
        Action::PowerOff => line(format_args!("> Power off")),
        Action::GaugeShutdown => line(format_args!("> Gauge shutdown")),
        Action::AutoOff => match auto_off_s {
          None => line(format_args!("> Auto off: never")),
          Some(s) if s >= 3600 => line(format_args!("> Auto off: {} h", s / 3600)),
          Some(s) => line(format_args!("> Auto off: {} min", s / 60)),
        },
        Action::Back => line(format_args!("> Back")),
      };
    }
    lines
  }
}

impl Default for BatteryPage {
  fn default() -> Self {
    Self::new()
  }
}

/// Status word for the dashboard: the measurement while there is one, else the gauge's own
/// learning
pub fn status(phase: Phase, gauging_flags: u16) -> &'static str {
  match phase {
    Phase::WaitFull => "WAIT",
    Phase::Full => "FULL",
    Phase::Discharging { .. } => "MEAS",
    Phase::Done { .. } => "DONE",
    Phase::Interrupted => "STOP",
    Phase::Idle if gauging_flags & gauging::FCCX != 0 => "UPD",
    Phase::Idle if gauging_flags & gauging::VDQ != 0 => "LRN",
    Phase::Idle => "",
  }
}

fn describe(phase: Phase) -> (Text, &'static str) {
  match phase {
    Phase::Idle => (line(format_args!("off")), "gauge learns itself"),
    Phase::WaitFull => (line(format_args!("waiting")), "charge to full"),
    Phase::Full => (line(format_args!("full")), "unplug, start load"),
    Phase::Discharging { mah, .. } => (line(format_args!("{} mAh", mah as u32)), "discharge to empty"),
    Phase::Done { mah } => (line(format_args!("{} mAh", mah)), "long press Save"),
    Phase::Interrupted => (line(format_args!("interrupted")), "charged mid-way"),
  }
}

fn flags(gauging_flags: u16) -> Text {
  let mut text: Text = line(format_args!("Flags   "));
  for (bit, name) in gauging::NAMES {
    if gauging_flags & bit != 0 {
      text = line(format_args!("{} {}", text.as_str(), name));
    }
  }
  text
}

fn access(operation_status: u32) -> &'static str {
  match (operation_status >> operation::SEC_SHIFT) & operation::SEC_MASK {
    0b01 => "full",
    0b10 => "unsealed",
    operation::SEALED => "sealed",
    _ => "?",
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn save_is_offered_only_after_a_measurement() {
    let mut page = BatteryPage::new();
    assert_eq!(page.press(Press::Long, Phase::Idle), Some(Action::StartMeasuring));
    let done = Phase::Done { mah: 6870 };
    assert_eq!(page.press(Press::Long, done), Some(Action::Save(6870)));
    let measuring = Phase::Discharging { mah: 10.0, charged_mah: 0.0 };
    assert_eq!(page.press(Press::Long, measuring), Some(Action::StopMeasuring));
  }

  #[test]
  fn short_presses_cycle_through_the_actions() {
    let mut page = BatteryPage::new();
    let done = Phase::Done { mah: 6870 };
    page.press(Press::Short, done);
    page.press(Press::Short, done);
    assert_eq!(page.press(Press::Long, done), Some(Action::AutoOff));
    page.press(Press::Short, done);
    assert_eq!(page.press(Press::Long, done), Some(Action::PowerOff));
    page.press(Press::Short, done);
    assert_eq!(page.press(Press::Long, done), Some(Action::Back));
    page.press(Press::Short, done);
    assert_eq!(page.press(Press::Long, done), Some(Action::Save(6870)));
  }

  #[test]
  fn power_off_is_not_offered_while_measuring() {
    let mut page = BatteryPage::new();
    let measuring = Phase::Discharging { mah: 10.0, charged_mah: 0.0 };
    for _ in 0..4 {
      page.press(Press::Short, measuring);
      let action = page.press(Press::Long, measuring);
      assert!(!matches!(action, Some(Action::PowerOff | Action::GaugeShutdown)), "{action:?}");
    }
  }

  #[test]
  fn selection_stays_valid_when_the_actions_shrink() {
    let mut page = BatteryPage::new();
    let done = Phase::Done { mah: 6870 };
    for _ in 0..3 {
      page.press(Press::Short, done);
    }
    // Fourth action selected; measuring has three
    let measuring = Phase::Discharging { mah: 10.0, charged_mah: 0.0 };
    assert_eq!(page.press(Press::Long, measuring), Some(Action::StopMeasuring));
  }
}
