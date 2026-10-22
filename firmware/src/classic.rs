//! Firmware with the classic layout: charge line, the cells drawn as batteries, port table
#![no_main]
#![no_std]

mod app;
mod clock;
mod capacity;

/// What the display shows
type Screen = graphics::layouts::Classic;
