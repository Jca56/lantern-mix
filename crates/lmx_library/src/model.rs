//! Track, Cue, SavedLoop, Playlist, SmartPlaylist, Tag, HistorySet, Library.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Identity of a track: a content hash of (file size, first 1 MiB of audio
/// data, last 64 KiB). Survives moves and retags; changes on re-encode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TrackId(pub u64);

impl TrackId {
    /// Hash the file's content. `audio_offset` is where the sample data starts
    /// (after headers/tags) so retagging leaves the id alone.
    pub fn from_file(path: &Path, audio_offset: u64) -> std::io::Result<Self> {
        let mut f = std::fs::File::open(path)?;
        let size = f.metadata()?.len();
        let mut h = lmx_core::hash::Hasher::new();
        h.write_u64(size);
        let mut buf = vec![0u8; 1 << 20];
        f.seek(SeekFrom::Start(audio_offset.min(size)))?;
        let n = read_up_to(&mut f, &mut buf)?;
        h.write(&buf[..n]);
        let tail = 64 * 1024;
        if size > audio_offset + n as u64 + tail {
            f.seek(SeekFrom::Start(size - tail))?;
            let n = read_up_to(&mut f, &mut buf[..tail as usize])?;
            h.write(&buf[..n]);
        }
        Ok(TrackId(h.finish()))
    }
}

fn read_up_to(f: &mut std::fs::File, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut got = 0;
    while got < buf.len() {
        let n = f.read(&mut buf[got..])?;
        if n == 0 {
            break;
        }
        got += n;
    }
    Ok(got)
}

/// Beat grid. `bpm == 0` means unknown (nothing analyzed or typed yet).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Grid {
    pub bpm: f32,
    /// Source frame of bar 1, beat 1.
    pub anchor_frame: f64,
    /// User-edited: re-analysis must not overwrite.
    pub locked: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Track {
    pub id: TrackId,
    pub path: PathBuf,
    pub file_size: u64,
    pub file_mtime: u64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub comment: String,
    pub bpm_tag: Option<f32>,
    pub key_tag: Option<String>,
    pub sample_rate: u32,
    pub duration_secs: f64,
    pub grid: Grid,
    /// Unix seconds when the track entered the collection.
    pub added: u64,
    /// File not found at its path during the last scan.
    pub missing: bool,
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

    /// Effective tempo: the grid if known, else the tag.
    pub fn bpm(&self) -> Option<f32> {
        if self.grid.bpm > 0.0 { Some(self.grid.bpm) } else { self.bpm_tag }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortBy {
    /// The user's own order (drag to arrange).
    Manual,
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
    /// Manual order of the collection; every track appears exactly once.
    pub order: Vec<TrackId>,
    index: HashMap<TrackId, usize>,
    /// Bumps on every change; views cache against it.
    pub generation: u64,
}

impl Library {
    /// Rebuild the id index and repair `order` (drop unknown ids, append
    /// tracks that have none — e.g. from journals written before ordering).
    pub fn rebuild_index(&mut self) {
        self.index = self.tracks.iter().enumerate().map(|(i, t)| (t.id, i)).collect();
        let mut seen = std::collections::HashSet::new();
        self.order.retain(|id| self.index.contains_key(id) && seen.insert(*id));
        for t in &self.tracks {
            if !seen.contains(&t.id) {
                self.order.push(t.id);
            }
        }
        self.generation += 1;
    }

    /// Position of each track in the manual order.
    pub fn positions(&self) -> HashMap<TrackId, usize> {
        self.order.iter().enumerate().map(|(i, id)| (*id, i)).collect()
    }

    /// Move `id` so it sits just before `before` (or last when `None`).
    pub fn move_before(&mut self, id: TrackId, before: Option<TrackId>) {
        if Some(id) == before || !self.index.contains_key(&id) {
            return;
        }
        self.order.retain(|x| *x != id);
        let at = before.and_then(|b| self.order.iter().position(|x| *x == b)).unwrap_or(self.order.len());
        self.order.insert(at, id);
        self.generation += 1;
    }

    pub fn get(&self, id: TrackId) -> Option<&Track> {
        self.index.get(&id).map(|i| &self.tracks[*i])
    }

    pub fn get_mut(&mut self, id: TrackId) -> Option<&mut Track> {
        self.generation += 1;
        self.index.get(&id).map(|i| &mut self.tracks[*i])
    }

    pub fn by_path(&self, p: &Path) -> Option<&Track> {
        self.tracks.iter().find(|t| t.path == p)
    }

    /// Insert or replace by id. New tracks go to the end of the manual order.
    pub fn upsert(&mut self, t: Track) {
        match self.index.get(&t.id) {
            Some(i) => self.tracks[*i] = t,
            None => {
                self.index.insert(t.id, self.tracks.len());
                self.order.push(t.id);
                self.tracks.push(t);
            }
        }
        self.generation += 1;
    }

    pub fn remove(&mut self, id: TrackId) -> Option<Track> {
        let i = self.index.remove(&id)?;
        let t = self.tracks.remove(i);
        self.order.retain(|x| *x != id);
        self.rebuild_index();
        Some(t)
    }

    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }
}
