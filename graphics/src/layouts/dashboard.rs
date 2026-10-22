use embedded_graphics::{
  draw_target::DrawTarget,
  geometry::{Point, Size},
  mono_font::{ascii::FONT_10X20, MonoFont},
  pixelcolor::{Rgb565, RgbColor, WebColors},
  primitives::{Circle, Line, Primitive, PrimitiveStyle, Rectangle, StyledDrawable},
  Drawable,
};

use super::Layout;
use crate::textual::format::{fixed, line};
use crate::textual::{Align, Cell};
use crate::{Snapshot, PORT_NAMES};

const FONT: &MonoFont<'static> = &FONT_10X20;
const BACKGROUND: Rgb565 = Rgb565::BLACK;
/// Background of a value that changed since the last update
const HIGHLIGHT: Rgb565 = Rgb565::new(8, 16, 8);

const VOLTS: Rgb565 = Rgb565::YELLOW;
const AMPS: Rgb565 = Rgb565::CYAN;
const WATTS: Rgb565 = Rgb565::CSS_MEDIUM_SLATE_BLUE;
const ENERGY: Rgb565 = Rgb565::CSS_ORANGE;
const TEMPERATURE: Rgb565 = Rgb565::CSS_YELLOW_GREEN;
const CYCLES: Rgb565 = Rgb565::CSS_DODGER_BLUE;
const HEALTH: Rgb565 = Rgb565::RED;
const FRAME: Rgb565 = Rgb565::CSS_GRAY;
const HEADER: Rgb565 = Rgb565::CSS_LIGHT_GRAY;
const HEADER_TEXT: Rgb565 = Rgb565::CSS_DIM_GRAY;

/// Pack rows: one line of `FONT` every 24 px
const ROW: i32 = 24;
/// Port boxes: top, width, the second box's left edge, row height
const BOX_TOP: i32 = 100;
const BOX_WIDTH: u32 = 94;
const BOX_LEFT: [i32; 2] = [2, 100];
const BOX_ROW: u32 = 22;
const BOX_BORDER: u32 = 2;
/// The DC and main port lines
const LINES_TOP: i32 = 194;

/// Dense colored readout after the display of a commercial powerbank: pack readings on top,
/// the two USB-C modules in boxes with the cell voltages beside them, and the DC and main
/// ports at the bottom. A value that changed since the last update is drawn highlighted.
pub struct Dashboard {
  pack_volts: Cell<7>,
  bar: ChargeBar,
  charge: Cell<4>,
  pack_amps: Cell<7>,
  energy: Cell<8>,
  pack_watts: Cell<7>,
  time_left: Cell<5>,
  pack_temp: Cell<5>,
  cycles: Cell<7>,
  health: Cell<4>,
  gauge_temp: Cell<5>,
  /// Volts, amps and watts of USB1 and USB2
  boxes: [[Cell<6>; 3]; 2],
  cells: [Cell<4>; 4],
  /// DC and main ports: amps and watts
  lines: [[Cell<7>; 2]; 2],
  drawn_static: bool,
}

impl Layout for Dashboard {
  fn new() -> Self {
    let row = |i: i32| 2 + i * ROW;
    let box_rows = [0, 1, 2].map(|i| BOX_TOP + (BOX_BORDER + 20 + i * BOX_ROW) as i32);
    let box_cell = |left: i32, top: i32, color| {
      let area = Rectangle::new(Point::new(left + BOX_BORDER as i32, top), Size::new(BOX_WIDTH - 2 * BOX_BORDER, BOX_ROW));
      Cell::new(area, FONT, Align::Center, color, BACKGROUND).with_highlight(HIGHLIGHT)
    };
    let header_top = BOX_TOP + BOX_BORDER as i32;
    // Beside the box header and the three box rows
    let cell_rows = [(header_top, 20), (box_rows[0], BOX_ROW), (box_rows[1], BOX_ROW), (box_rows[2], BOX_ROW)];
    let line_row = |i: i32| LINES_TOP + i * 22;

    Self {
      pack_volts: cell(2, row(0), Align::Left, VOLTS),
      bar: ChargeBar::new(Point::new(78, row(0) + 1)),
      charge: cell(198, row(0), Align::Right, Rgb565::WHITE),
      pack_amps: cell(2, row(1), Align::Left, AMPS),
      energy: cell(158, row(1), Align::Right, ENERGY),
      pack_watts: cell(2, row(2), Align::Left, WATTS),
      time_left: cell(80, row(2), Align::Right, Rgb565::WHITE),
      pack_temp: cell(188, row(2), Align::Right, TEMPERATURE),
      cycles: cell(2, row(3), Align::Left, CYCLES),
      health: cell(80, row(3), Align::Right, HEALTH),
      gauge_temp: cell(188, row(3), Align::Right, TEMPERATURE),
      boxes: BOX_LEFT.map(|left| {
        [box_cell(left, box_rows[0], VOLTS), box_cell(left, box_rows[1], AMPS), box_cell(left, box_rows[2], WATTS)]
      }),
      cells: cell_rows.map(|(top, height)| {
        let area = Rectangle::new(Point::new(198, top), Size::new(40, height));
        Cell::new(area, FONT, Align::Right, Rgb565::WHITE, BACKGROUND).with_highlight(HIGHLIGHT)
      }),
      lines: [0, 1].map(|i| [cell(48, line_row(i), Align::Left, AMPS), cell(168, line_row(i), Align::Right, WATTS)]),
      drawn_static: false,
    }
  }

  fn update<D: DrawTarget<Color = Rgb565>>(&mut self, s: &Snapshot, target: &mut D) -> Result<(), D::Error> {
    if !self.drawn_static {
      target.clear(BACKGROUND)?;
      self.draw_static(target)?;
      self.drawn_static = true;
    }

    self.pack_volts.update(fixed::<7>(s.pack_volts, 3, "V").as_str(), target)?;
    self.bar.update(s.charge, target)?;
    self.charge.update(line::<4>(format_args!("{}%", s.charge)).as_str(), target)?;
    self.pack_amps.update(fixed::<7>(s.pack_amps, 3, "A").as_str(), target)?;
    self.energy.update(fixed::<8>(s.remaining_wh, 3, "Wh").as_str(), target)?;
    self.pack_watts.update(fixed::<7>(s.pack_watts(), 3, "W").as_str(), target)?;
    self.time_left.update(time_left(s.minutes_left).as_str(), target)?;
    self.pack_temp.update(fixed::<5>(s.pack_temp, 2, "").as_str(), target)?;
    self.cycles.update(line::<7>(format_args!("CYC:{:03}", s.cycles)).as_str(), target)?;
    // The learning state takes the place of the health while there is one
    let health = line::<4>(format_args!("{}%", s.health));
    let status = if s.status.is_empty() { health.as_str() } else { s.status };
    self.health.update(status, target)?;
    self.gauge_temp.update(fixed::<5>(s.gauge_temp, 2, "").as_str(), target)?;

    for (cells, port) in self.boxes.iter_mut().zip(&s.ports) {
      let [volts, amps, watts] = cells;
      volts.update(fixed::<6>(port.volts, 3, "V").as_str(), target)?;
      amps.update(fixed::<6>(port.amps, 3, "A").as_str(), target)?;
      watts.update(fixed::<6>(port.watts(), 3, "W").as_str(), target)?;
    }
    for (cell, &volts) in self.cells.iter_mut().zip(&s.cell_volts) {
      cell.update(fixed::<4>(volts, 2, "").as_str(), target)?;
    }
    let dc = &s.ports[2];
    for (cells, (amps, watts)) in self.lines.iter_mut().zip([(dc.amps, dc.watts()), (s.main_amps, s.main_watts())]) {
      let [amps_cell, watts_cell] = cells;
      amps_cell.update(fixed::<7>(amps, 4, "A").as_str(), target)?;
      watts_cell.update(fixed::<7>(watts, 4, "W").as_str(), target)?;
    }
    Ok(())
  }
}

impl Dashboard {
  /// Box frames and headers, port labels, the bar outline and the temperature icons
  fn draw_static<D: DrawTarget<Color = Rgb565>>(&self, target: &mut D) -> Result<(), D::Error> {
    let box_height = BOX_BORDER * 2 + 20 + 3 * BOX_ROW;
    for (&left, name) in BOX_LEFT.iter().zip(PORT_NAMES) {
      let frame = Rectangle::new(Point::new(left, BOX_TOP), Size::new(BOX_WIDTH, box_height));
      frame.draw_styled(&PrimitiveStyle::with_stroke(FRAME, BOX_BORDER), target)?;
      let header = Rectangle::new(
        frame.top_left + Point::new_equal(BOX_BORDER as i32),
        Size::new(BOX_WIDTH - 2 * BOX_BORDER, 20),
      );
      let mut title = Cell::<8>::new(header, FONT, Align::Center, HEADER_TEXT, HEADER);
      title.update(name, target)?;
    }

    for (i, name) in [PORT_NAMES[2], "MAIN"].into_iter().enumerate() {
      let mut label = Cell::<4>::fitted(Point::new(2, LINES_TOP + i as i32 * 22), FONT, Align::Left, WATTS, BACKGROUND);
      label.update(name, target)?;
    }

    self.bar.draw_static(target)?;
    thermometer(Point::new(172, 2 + 2 * ROW), target)?;
    chip(Point::new(171, 2 + 3 * ROW + 4), target)
  }
}

/// A cell of `N` characters of `FONT` at `(x, y)`, highlighted on change
fn cell<const N: usize>(x: i32, y: i32, align: Align, color: Rgb565) -> Cell<N> {
  Cell::fitted(Point::new(x, y), FONT, align, color, BACKGROUND).with_highlight(HIGHLIGHT)
}

/// `h:mm`, or dashes when the gauge has no estimate
fn time_left(minutes: Option<u16>) -> crate::textual::format::Line<5> {
  match minutes {
    Some(m) if m / 60 < 100 => line(format_args!("{}:{:02}", m / 60, m % 60)),
    Some(_) => line(format_args!(">99h")),
    None => line(format_args!("--:--")),
  }
}

fn thermometer<D: DrawTarget<Color = Rgb565>>(top_left: Point, target: &mut D) -> Result<(), D::Error> {
  let style = PrimitiveStyle::with_fill(TEMPERATURE);
  Rectangle::new(top_left + Point::new(3, 2), Size::new(4, 11)).draw_styled(&style, target)?;
  Circle::new(top_left + Point::new(0, 11), 10).draw_styled(&style, target)
}

fn chip<D: DrawTarget<Color = Rgb565>>(top_left: Point, target: &mut D) -> Result<(), D::Error> {
  let style = PrimitiveStyle::with_stroke(TEMPERATURE, 2);
  Rectangle::new(top_left + Point::new(3, 3), Size::new(10, 10)).draw_styled(&style, target)?;
  let pins = PrimitiveStyle::with_stroke(TEMPERATURE, 1);
  for offset in [5, 8, 11] {
    let (a, b) = (top_left + Point::new(offset - 1, 0), top_left + Point::new(offset - 1, 15));
    Line::new(a, a + Point::new(0, 2)).into_styled(pins).draw(target)?;
    Line::new(b - Point::new(0, 2), b).into_styled(pins).draw(target)?;
    let (c, d) = (top_left + Point::new(0, offset - 1), top_left + Point::new(15, offset - 1));
    Line::new(c, c + Point::new(2, 0)).into_styled(pins).draw(target)?;
    Line::new(d - Point::new(2, 0), d).into_styled(pins).draw(target)?;
  }
  Ok(())
}

/// Horizontal battery gauge: red below 20 %, orange below 50 %, green above
struct ChargeBar {
  frame: Rectangle,
  shown: Option<u8>,
}

impl ChargeBar {
  const SIZE: Size = Size::new(106, 18);

  fn new(top_left: Point) -> Self {
    Self { frame: Rectangle::new(top_left, Self::SIZE), shown: None }
  }

  fn draw_static<D: DrawTarget<Color = Rgb565>>(&self, target: &mut D) -> Result<(), D::Error> {
    self.frame.draw_styled(&PrimitiveStyle::with_stroke(FRAME, 1), target)?;
    let tip = Rectangle::new(self.frame.top_left + Point::new(Self::SIZE.width as i32, 5), Size::new(3, 8));
    target.fill_solid(&tip, FRAME)
  }

  fn update<D: DrawTarget<Color = Rgb565>>(&mut self, percents: u8, target: &mut D) -> Result<(), D::Error> {
    let percents = percents.min(100);
    if self.shown == Some(percents) {
      return Ok(());
    }
    self.shown = Some(percents);

    let inside = Rectangle::new(self.frame.top_left + Point::new(2, 2), Self::SIZE - Size::new(4, 4));
    let filled = inside.size.width * u32::from(percents) / 100;
    let color = match percents {
      0..20 => Rgb565::RED,
      20..50 => Rgb565::CSS_ORANGE,
      _ => Rgb565::GREEN,
    };
    target.fill_solid(&Rectangle::new(inside.top_left, Size::new(filled, inside.size.height)), color)?;
    let rest = Rectangle::new(
      inside.top_left + Point::new(filled as i32, 0),
      Size::new(inside.size.width - filled, inside.size.height),
    );
    target.fill_solid(&rest, BACKGROUND)
  }
}
