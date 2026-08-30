//! Immediate-mode widget kit: ids, widget memory, input, layout, theme (big text), DJ widgets. Headless-testable; draws through lmx_gpu.
//!
//! Design: see `docs/` — this crate is a skeleton; responsibilities and module
//! boundaries are decided, logic is not written yet.
#![forbid(unsafe_code)]

pub mod ui;
pub mod id;
pub mod input;
pub mod layout;
pub mod theme;
pub mod widgets;
pub mod table;
pub mod waveform;
pub mod deck;
