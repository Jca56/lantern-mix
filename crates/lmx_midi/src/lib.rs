//! Raw MIDI over /dev/snd (std I/O only), parser with 14-bit CC pairing, mapping engine (.lmxmap), built-in DDJ-GRV6 profile, LED feedback.
//!
//! Design: see `docs/` — this crate is a skeleton; responsibilities and module
//! boundaries are decided, logic is not written yet.
#![forbid(unsafe_code)]

pub mod transport;
pub mod parse;
pub mod mapping;
pub mod action;
pub mod feedback;
pub mod profiles;
