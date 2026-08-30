//! Track, Cue, SavedLoop, Playlist, SmartPlaylist, Tag, HistorySet, Library.

use std::path::{Path, PathBuf};

/// Identity of a track. MVP: a hash of the path; the content hash from
/// `docs/04-LIBRARY.md` replaces this when the store lands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TrackId(pub u64);

impl TrackId {
    pub fn from_path(p: &Path) -> Self {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in p.to_string_lossy().as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        TrackId(h)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Track {
    pub id: TrackId,
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub bpm: Option<f32>,
    pub key: Option<String>,
    pub sample_rate: u32,
    pub duration_secs: f64,
}

impl Default for TrackId {
    fn default() -> Self {
        TrackId(0)
    }
}

impl Track {
    /// Title falls back to the file stem so nothing shows up blank.
    pub fn display_title(&self) -> &str {
        if self.title.is_empty() {
            self.path.file_stem().and_then(|s| s.to_str()).unwrap_or("?")
        } else {
            &self.title
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortBy {
    Title,
    Artist,
    Bpm,
    Key,
    Time,
}

#[derive(Clone, Debug, Default)]
pub struct Library {
    pub roots: Vec<PathBuf>,
    pub tracks: Vec<Track>,
}

impl Library {
    /// Replace the collection with a fresh scan result (dedup by id).
    pub fn set_tracks(&mut self, mut tracks: Vec<Track>) {
        tracks.sort_by_key(|t| t.id);
        tracks.dedup_by_key(|t| t.id);
        self.tracks = tracks;
    }

    pub fn get(&self, id: TrackId) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == id)
    }

    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }
}
