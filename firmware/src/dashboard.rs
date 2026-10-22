//! Firmware with the dashboard layout: dense colored readout of every value
#![no_main]
#![no_std]

mod app;
mod clock;
mod capacity;
mod store;

/// What the display shows
type Screen = graphics::layouts::Dashboard;
