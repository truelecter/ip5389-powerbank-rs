//! Short and long presses of the button, from the times of its edges.

/// Edges closer than this to the press are contact bounce
pub const DEBOUNCE_MS: u32 = 30;
/// A press at least this long is a long press
pub const LONG_MS: u32 = 600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
  Short,
  Long,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Button {
  /// When the button went down
  down_at_ms: Option<u32>,
}

impl Button {
  pub const fn new() -> Self {
    Self { down_at_ms: None }
  }

  /// Takes an edge: `down` is the button state after it, `now_ms` a millisecond clock that
  /// may wrap. Returns the press that a release ends.
  pub fn edge(&mut self, down: bool, now_ms: u32) -> Option<Press> {
    if down {
      self.down_at_ms.get_or_insert(now_ms);
      return None;
    }
    let held = now_ms.wrapping_sub(self.down_at_ms.take()?);
    if held < DEBOUNCE_MS {
      None
    } else if held < LONG_MS {
      Some(Press::Short)
    } else {
      Some(Press::Long)
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn press_length_decides_the_kind() {
    let mut button = Button::new();
    assert_eq!(button.edge(true, 10_000), None);
    assert_eq!(button.edge(false, 10_200), Some(Press::Short));
    button.edge(true, 11_000);
    assert_eq!(button.edge(false, 11_700), Some(Press::Long));
  }

  #[test]
  fn bounces_are_ignored() {
    let mut button = Button::new();
    button.edge(true, 5_000);
    // A bounce while pressed keeps the first down edge
    button.edge(true, 5_010);
    assert_eq!(button.edge(false, 5_010), None);
    // Release without a press
    assert_eq!(button.edge(false, 6_000), None);
  }

  #[test]
  fn time_of_the_first_down_edge_counts() {
    let mut button = Button::new();
    button.edge(true, 1_000);
    button.edge(true, 1_500);
    assert_eq!(button.edge(false, 1_700), Some(Press::Long));
  }

  #[test]
  fn clock_wrapping_during_a_press() {
    let mut button = Button::new();
    button.edge(true, u32::MAX - 100);
    assert_eq!(button.edge(false, 100), Some(Press::Short));
  }
}
