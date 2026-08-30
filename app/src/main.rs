//! Lantern Mix — the application: winit loop, repaint-on-demand policy, screens,
//! wiring between audio, MIDI, library, workers and UI.
//!
//! Design: `docs/01-ARCHITECTURE.md`, `docs/05-UI.md`.

pub mod app;
pub mod browser;
pub mod screens;
pub mod settings;
pub mod titlebar;
pub mod wiring;
pub mod workers;

fn main() {
    let paths: Vec<std::path::PathBuf> = std::env::args().skip(1).map(Into::into).collect();
    if let Err(e) = app::App::run(paths) {
        eprintln!("lantern-mix: {e}");
        std::process::exit(1);
    }
}
