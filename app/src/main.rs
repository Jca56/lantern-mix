//! Lantern Mix — the application: winit loop, repaint-on-demand policy, screens, wiring between audio, MIDI, library, workers and UI.
//!
//! Design: see `docs/` — this crate is a skeleton; responsibilities and module
//! boundaries are decided, logic is not written yet.

pub mod app;
pub mod screens;
pub mod workers;
pub mod wiring;
pub mod settings;

fn main() {
    println!("lantern-mix — design phase. See docs/.");
}
