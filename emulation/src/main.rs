//! Runs the display layouts on the desktop with made-up readings.
//!
//! `emulation [text|classic|dashboard|battery]` opens a window with readings that change
//! every second (dashboard by default).
//! `emulation --png DIR` writes one preview strip per layout to `DIR`. The layouts strips show
//! the first draw, an update with changed values (highlighted) and the same values again; the
//! battery page strip shows its measurement phases.

use std::{env, path::Path, thread, time::Duration};

use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
use embedded_graphics_simulator::{OutputSettings, OutputSettingsBuilder, SimulatorDisplay, SimulatorEvent, Window};

use graphics::battery::{BatteryPage, Details};
use graphics::layouts::{Classic, Dashboard, Layout, Text};
use graphics::{Port, Snapshot};
use peripherals::bq4050::gauging;
use power::learning::Phase;

const SIZE: u32 = 240;
/// Space between the frames of a strip
const GAP: u32 = 8;

fn main() {
  let args: Vec<String> = env::args().skip(1).collect();
  let settings = OutputSettingsBuilder::new().scale(2).build();
  match args.as_slice() {
    [flag, dir] if flag == "--png" => {
      let dir = Path::new(dir);
      layout_strip::<Text>(&dir.join("text.png"), &settings);
      layout_strip::<Classic>(&dir.join("classic.png"), &settings);
      layout_strip::<Dashboard>(&dir.join("dashboard.png"), &settings);
      battery_strip(&dir.join("battery.png"), &settings);
    }
    [] => run(&settings, layout_frame::<Dashboard>),
    [name] if name == "dashboard" => run(&settings, layout_frame::<Dashboard>),
    [name] if name == "classic" => run(&settings, layout_frame::<Classic>),
    [name] if name == "text" => run(&settings, layout_frame::<Text>),
    [name] if name == "battery" => run(&settings, battery_frame),
    _ => eprintln!("usage: emulation [text|classic|dashboard|battery] | emulation --png DIR"),
  }
}

/// Readings `t` seconds in: a phone charging on USB2, the pack slowly discharging
fn snapshot(t: u32) -> Snapshot {
  let wobble = (t % 4) as f32 * 0.013;
  let usb2 = Port { volts: 9.02, amps: 1.65 + wobble };
  let main_amps = if t % 10 < 5 { 0.0 } else { 0.48 };
  let pack_amps = -(usb2.watts() / 14.9) - main_amps - 0.012;
  Snapshot {
    ports: [Port { volts: 14.93, amps: 0.0 }, Port { volts: 14.93, ..usb2 }, Port { volts: 14.93, amps: 0.004 }],
    main_amps,
    pack_volts: 14.93 - t as f32 * 0.001,
    pack_amps,
    cell_volts: [3.731, 3.742 + wobble / 10.0, 3.735, 3.729],
    charge: 62 - (t / 30) as u8,
    health: 100,
    cycles: 3,
    status: if t % 20 < 10 { "" } else { "MEAS" },
    remaining_wh: 30.512 - t as f32 * 0.004,
    minutes_left: Some(263 - (t / 60) as u16),
    pack_temp: 27.15 + (t % 3) as f32 * 0.1,
    gauge_temp: 29.4,
    uptime_s: 3_600 + t,
  }
}

/// The battery page `t` seconds into a measurement that ends at 30 s
fn battery_state(t: u32) -> (Details, Phase) {
  let phase = match t {
    0..=4 => Phase::Idle,
    5..=9 => Phase::Full,
    10..=29 => Phase::Discharging { mah: 2310.0 + t as f32 * 0.4, charged_mah: 0.0 },
    _ => Phase::Done { mah: 6870 },
  };
  let gauging = match phase {
    Phase::Discharging { .. } => gauging::DSG | gauging::VDQ,
    Phase::Done { .. } => gauging::DSG | gauging::EDV2 | gauging::FCCX | gauging::EDV0,
    _ => gauging::DSG,
  };
  let details = Details { full_mah: 9800, design_mah: 9800, remaining_mah: 5168, max_error: 100, gauging, operation: 0x0187 };
  (details, phase)
}

/// Draws the screen at `t` seconds; `state` keeps the screen between calls
type Frame<S> = fn(&mut Option<S>, u32, &mut SimulatorDisplay<Rgb565>);

fn layout_frame<L: Layout>(state: &mut Option<L>, t: u32, display: &mut SimulatorDisplay<Rgb565>) {
  state.get_or_insert_with(L::new).update(&snapshot(t), display).unwrap();
}

fn battery_frame(state: &mut Option<BatteryPage>, t: u32, display: &mut SimulatorDisplay<Rgb565>) {
  let (details, phase) = battery_state(t % 40);
  let auto_off_s = Some(30 * 60);
  state.get_or_insert_with(BatteryPage::new).update(&snapshot(t), &details, phase, auto_off_s, display).unwrap();
}

/// A strip of frames side by side, each one the screen after the updates at `times` up to it
fn strip(frames: usize, mut draw: impl FnMut(usize, &mut SimulatorDisplay<Rgb565>), path: &Path, settings: &OutputSettings) {
  let width = frames as u32 * (SIZE + GAP) - GAP;
  let mut out = SimulatorDisplay::<Rgb565>::new(Size::new(width, SIZE));
  out.clear(Rgb565::CSS_DIM_GRAY).unwrap();
  for frame in 0..frames {
    let mut screen = SimulatorDisplay::<Rgb565>::new(Size::new_equal(SIZE));
    draw(frame, &mut screen);
    let offset = Point::new((frame as u32 * (SIZE + GAP)) as i32, 0);
    let pixels = screen.bounding_box().points().map(|p| Pixel(p + offset, screen.get_pixel(p)));
    out.draw_iter(pixels).unwrap();
  }
  out.to_rgb_output_image(settings).save_png(path).unwrap();
  println!("{}", path.display());
}

fn layout_strip<L: Layout>(path: &Path, settings: &OutputSettings) {
  // The first draw, values changed after 13 s, and the same values again
  let times = [0, 13, 13];
  let draw = |frame: usize, screen: &mut SimulatorDisplay<Rgb565>| {
    let mut layout = None;
    for &t in &times[..=frame] {
      layout_frame::<L>(&mut layout, t, screen);
    }
  };
  strip(times.len(), draw, path, settings);
}

fn battery_strip(path: &Path, settings: &OutputSettings) {
  // Idle, full, measuring, done
  let times = [0, 5, 20, 30];
  let draw = |frame: usize, screen: &mut SimulatorDisplay<Rgb565>| {
    let mut page = None;
    battery_frame(&mut page, times[frame], screen);
  };
  strip(times.len(), draw, path, settings);
}

fn run<S>(settings: &OutputSettings, frame: Frame<S>) {
  let mut display = SimulatorDisplay::<Rgb565>::new(Size::new_equal(SIZE));
  let mut window = Window::new("Powerbank display", settings);
  let mut state = None;
  for t in 0.. {
    frame(&mut state, t, &mut display);
    window.update(&display);
    if window.events().any(|event| event == SimulatorEvent::Quit) {
      return;
    }
    thread::sleep(Duration::from_secs(1));
  }
}
