//! Audio host: PipeWire stream (official pipewire crate), device/profile discovery,
//! rate negotiation; calls the render callback from the RT thread.
//!
//! The engine (`lmx_engine`) will implement `AudioRender`; until then `TestTone`
//! does, which is also what the settings screen will use to test outputs.
//! Design: `docs/02-AUDIO-ENGINE.md` (Latency and buffer policy), `docs/01-ARCHITECTURE.md`.

pub mod devices;
pub mod host;
pub mod pw;
pub mod tone;

pub use host::{AudioConfig, AudioRender, AudioState, AudioStatus};
pub use pw::AudioHost;
pub use tone::{Levels, TestTone, ToneControl};
