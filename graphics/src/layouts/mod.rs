//! Complete screens. Each firmware variant shows one of them.

mod classic;
mod dashboard;
mod text;

pub use self::classic::Classic;
pub use self::dashboard::Dashboard;
pub use self::text::Text;

use embedded_graphics::{draw_target::DrawTarget, pixelcolor::Rgb565};

use crate::Snapshot;

/// A screen that shows a [`Snapshot`] and redraws only what changed since the last update
pub trait Layout {
  fn new() -> Self;

  /// Draws the readings. The first update draws the static parts too, on a cleared screen.
  fn update<D: DrawTarget<Color = Rgb565>>(&mut self, snapshot: &Snapshot, target: &mut D) -> Result<(), D::Error>;
}
