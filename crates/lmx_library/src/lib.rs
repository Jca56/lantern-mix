//! The collection: tracks, cues, grids, playlists, tags, history; own
//! snapshot+journal storage; scanning; search.
//!
//! MVP slice: an in-memory collection rebuilt from folder roots at launch, with
//! search and sort. Storage, cues and playlists follow (`docs/04-LIBRARY.md`).
#![forbid(unsafe_code)]

pub mod analysis_cache;
pub mod format;
pub mod model;
pub mod mutation;
pub mod scan;
pub mod search;
pub mod store;

pub use model::{Grid, Library, SortBy, Track, TrackId};
pub use mutation::Mutation;
pub use scan::walk_audio_files;
pub use store::Store;
