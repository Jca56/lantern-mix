//! The collection: tracks, cues, grids, playlists, tags, history; own snapshot+journal storage; scanning; search.
//!
//! Design: see `docs/` — this crate is a skeleton; responsibilities and module
//! boundaries are decided, logic is not written yet.
#![forbid(unsafe_code)]

pub mod model;
pub mod mutation;
pub mod format;
pub mod store;
pub mod scan;
pub mod search;
pub mod analysis_cache;
