//! Audio host: PipeWire stream (official pipewire crate), device/profile discovery, rate negotiation; calls Engine::process from the RT thread.
//!
//! Design: see `docs/` — this crate is a skeleton; responsibilities and module
//! boundaries are decided, logic is not written yet.

pub mod host;
pub mod pw;
pub mod devices;
