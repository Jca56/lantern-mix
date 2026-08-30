//! The collection on disk: `Store` + `Library`, and the merge logic that turns
//! scan results and deck edits into journaled mutations.

use lmx_library::{Grid, Library, Mutation, Store, Track, TrackId};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// One file the scanner found, probed and identified.
pub struct ScannedFile {
    pub id: TrackId,
    pub path: PathBuf,
    pub file_size: u64,
    pub file_mtime: u64,
    pub probe: lmx_codec::Probe,
}

/// Root-relative snapshot of what the library knows, for the scanner's
/// fast path (unchanged size+mtime → keep the id, skip hashing).
pub type KnownFiles = HashMap<PathBuf, (u64, u64, TrackId)>;

pub struct Db {
    pub lib: Library,
    store: Option<Store>,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn file_stamp(p: &Path) -> Option<(u64, u64)> {
    let m = std::fs::metadata(p).ok()?;
    let mtime = m.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_secs();
    Some((m.len(), mtime))
}

impl Db {
    /// Open the library in the data dir. A store failure degrades to an
    /// in-memory library (with a loud message) rather than killing the app.
    pub fn open() -> Self {
        let dir = lmx_core::paths::data_dir();
        match Store::open(&dir) {
            Ok((lib, store)) => {
                eprintln!("lantern-mix: library {} tracks, {} root(s) from {}", lib.len(), lib.roots.len(), dir.display());
                Self { lib, store: Some(store) }
            }
            Err(e) => {
                eprintln!("lantern-mix: library store unavailable ({e}); changes will not be saved");
                Self { lib: Library::default(), store: None }
            }
        }
    }

    pub fn apply(&mut self, m: Mutation) {
        match &mut self.store {
            Some(st) => {
                if let Err(e) = st.apply(&mut self.lib, m.clone()) {
                    eprintln!("lantern-mix: journal write failed: {e}");
                    m.apply(&mut self.lib);
                }
            }
            None => m.apply(&mut self.lib),
        }
    }

    pub fn snapshot(&mut self) {
        if let Some(st) = &mut self.store {
            if let Err(e) = st.snapshot(&self.lib) {
                eprintln!("lantern-mix: snapshot failed: {e}");
            }
        }
    }

    pub fn known_files(&self) -> KnownFiles {
        self.lib.tracks.iter().map(|t| (t.path.clone(), (t.file_size, t.file_mtime, t.id))).collect()
    }

    fn track_from(f: &ScannedFile) -> Track {
        let m = &f.probe.metadata;
        Track {
            id: f.id,
            path: f.path.clone(),
            file_size: f.file_size,
            file_mtime: f.file_mtime,
            title: m.title.clone().unwrap_or_default(),
            artist: m.artist.clone().unwrap_or_default(),
            album: m.album.clone().unwrap_or_default(),
            genre: m.genre.clone().unwrap_or_default(),
            comment: m.comment.clone().unwrap_or_default(),
            bpm_tag: m.bpm_tag,
            key_tag: m.key_tag.clone(),
            sample_rate: f.probe.sample_rate,
            duration_secs: f.probe.duration_secs(),
            grid: Grid::default(),
            added: now(),
            missing: false,
        }
    }

    /// Fold a scan of `roots` into the library: new ids are added, moved files
    /// re-pathed, files gone from under the roots flagged missing.
    pub fn merge_scan(&mut self, roots: &[PathBuf], files: Vec<ScannedFile>) {
        let mut seen = std::collections::HashSet::new();
        for f in &files {
            seen.insert(f.id);
            match self.lib.get(f.id) {
                Some(t) => {
                    if t.path != f.path || t.file_size != f.file_size || t.file_mtime != f.file_mtime || t.missing {
                        self.apply(Mutation::SetPath { id: f.id, path: f.path.clone(), file_size: f.file_size, file_mtime: f.file_mtime });
                    }
                }
                None => self.apply(Mutation::Upsert(Self::track_from(f))),
            }
        }
        let gone: Vec<TrackId> = self
            .lib
            .tracks
            .iter()
            .filter(|t| !t.missing && !seen.contains(&t.id) && roots.iter().any(|r| t.path.starts_with(r)) && !t.path.exists())
            .map(|t| t.id)
            .collect();
        for id in gone {
            self.apply(Mutation::SetMissing { id, missing: true });
        }
    }

    /// Make sure a file (dropped, or from the command line) is in the
    /// collection; returns its id.
    pub fn ensure_track(&mut self, path: &Path) -> Option<TrackId> {
        if let Some(t) = self.lib.by_path(path) {
            return Some(t.id);
        }
        let probe = lmx_codec::probe(path).ok()?;
        let id = TrackId::from_file(path, probe.audio_offset).ok()?;
        if self.lib.get(id).is_none() {
            let (file_size, file_mtime) = file_stamp(path)?;
            self.apply(Mutation::Upsert(Self::track_from(&ScannedFile { id, path: path.to_path_buf(), file_size, file_mtime, probe })));
        }
        Some(id)
    }

    pub fn set_grid(&mut self, id: TrackId, bpm: f32, anchor_frame: f64) {
        self.apply(Mutation::SetGrid { id, grid: Grid { bpm, anchor_frame, locked: true } });
    }

    pub fn add_root(&mut self, root: PathBuf) {
        self.apply(Mutation::AddRoot(root));
    }
}
