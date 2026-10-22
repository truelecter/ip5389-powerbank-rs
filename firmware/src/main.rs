//! Firmware with the text layout: one reading per line
#![no_main]
#![no_std]

mod app;
mod clock;
mod capacity;
mod store;

/// What the display shows
type Screen = graphics::layouts::Text;
