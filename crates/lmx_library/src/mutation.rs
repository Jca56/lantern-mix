//! Mutation enum — the only way state changes; journaled then applied; inverse
//! for undo. Also the TLV encoding of tracks and mutations.

use crate::format::{Entries, Entry, Writer};
use crate::model::{Grid, Library, Playlist, PlaylistId, Track, TrackId};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub enum Mutation {
    /// Insert or replace a whole track record.
    Upsert(Track),
    Remove(TrackId),
    SetGrid { id: TrackId, grid: Grid },
    SetPath { id: TrackId, path: PathBuf, file_size: u64, file_mtime: u64 },
    SetMissing { id: TrackId, missing: bool },
    AddRoot(PathBuf),
    RemoveRoot(PathBuf),
    /// Put `id` just before `before` in the manual order (`None` = last).
    Move { id: TrackId, before: Option<TrackId> },
    CreatePlaylist { id: PlaylistId, name: String },
    RenamePlaylist { id: PlaylistId, name: String },
    DeletePlaylist(PlaylistId),
    /// Add (or move, if present) `track` before `before` in playlist `id`.
    PlaylistPlace { id: PlaylistId, track: TrackId, before: Option<TrackId> },
    PlaylistRemove { id: PlaylistId, track: TrackId },
}

impl Mutation {
    pub fn apply(&self, lib: &mut Library) {
        match self {
            Mutation::Move { id, before } => lib.move_before(*id, *before),
            Mutation::CreatePlaylist { id, name } => {
                if lib.playlist(*id).is_none() {
                    lib.playlists.push(Playlist { id: *id, name: name.clone(), tracks: Vec::new() });
                    lib.generation += 1;
                }
            }
            Mutation::RenamePlaylist { id, name } => {
                if let Some(p) = lib.playlist_mut(*id) {
                    p.name = name.clone();
                }
            }
            Mutation::DeletePlaylist(id) => {
                lib.playlists.retain(|p| p.id != *id);
                lib.generation += 1;
            }
            Mutation::PlaylistPlace { id, track, before } => {
                let known = lib.get(*track).is_some();
                if let (Some(p), true) = (lib.playlist_mut(*id), known) {
                    p.place(*track, *before);
                }
            }
            Mutation::PlaylistRemove { id, track } => {
                if let Some(p) = lib.playlist_mut(*id) {
                    p.tracks.retain(|x| x != track);
                }
            }
            Mutation::Upsert(t) => lib.upsert(t.clone()),
            Mutation::Remove(id) => {
                lib.remove(*id);
            }
            Mutation::SetGrid { id, grid } => {
                if let Some(t) = lib.get_mut(*id) {
                    t.grid = *grid;
                }
            }
            Mutation::SetPath { id, path, file_size, file_mtime } => {
                if let Some(t) = lib.get_mut(*id) {
                    t.path = path.clone();
                    t.file_size = *file_size;
                    t.file_mtime = *file_mtime;
                    t.missing = false;
                }
            }
            Mutation::SetMissing { id, missing } => {
                if let Some(t) = lib.get_mut(*id) {
                    t.missing = *missing;
                }
            }
            Mutation::AddRoot(p) => {
                if !lib.roots.contains(p) {
                    lib.roots.push(p.clone());
                    lib.generation += 1;
                }
            }
            Mutation::RemoveRoot(p) => {
                lib.roots.retain(|r| r != p);
                lib.generation += 1;
            }
        }
    }
}

// ── track encoding ───────────────────────────────────────────────────────

mod tt {
    pub const ID: u16 = 1;
    pub const PATH: u16 = 2;
    pub const TITLE: u16 = 3;
    pub const ARTIST: u16 = 4;
    pub const ALBUM: u16 = 5;
    pub const BPM_TAG: u16 = 6;
    pub const KEY_TAG: u16 = 7;
    pub const RATE: u16 = 8;
    pub const DURATION: u16 = 9;
    pub const SIZE: u16 = 10;
    pub const MTIME: u16 = 11;
    pub const GRID_BPM: u16 = 12;
    pub const GRID_ANCHOR: u16 = 13;
    pub const GRID_LOCKED: u16 = 14;
    pub const ADDED: u16 = 15;
    pub const MISSING: u16 = 16;
    pub const GENRE: u16 = 17;
    pub const COMMENT: u16 = 18;
}

pub fn encode_track(t: &Track) -> Writer {
    let mut w = Writer::new();
    w.u64(tt::ID, t.id.0);
    w.str(tt::PATH, &t.path.to_string_lossy());
    w.str(tt::TITLE, &t.title);
    w.str(tt::ARTIST, &t.artist);
    w.str(tt::ALBUM, &t.album);
    w.str(tt::GENRE, &t.genre);
    w.str(tt::COMMENT, &t.comment);
    if let Some(b) = t.bpm_tag {
        w.f32(tt::BPM_TAG, b);
    }
    if let Some(k) = &t.key_tag {
        w.str(tt::KEY_TAG, k);
    }
    w.u32(tt::RATE, t.sample_rate);
    w.f64(tt::DURATION, t.duration_secs);
    w.u64(tt::SIZE, t.file_size);
    w.u64(tt::MTIME, t.file_mtime);
    w.f32(tt::GRID_BPM, t.grid.bpm);
    w.f64(tt::GRID_ANCHOR, t.grid.anchor_frame);
    w.bool(tt::GRID_LOCKED, t.grid.locked);
    w.u64(tt::ADDED, t.added);
    w.bool(tt::MISSING, t.missing);
    w
}

pub fn decode_track(e: Entry<'_>) -> Option<Track> {
    let mut t = Track::default();
    let mut has_id = false;
    for f in e.entries() {
        match f.tag {
            tt::ID => {
                t.id = TrackId(f.u64()?);
                has_id = true;
            }
            tt::PATH => t.path = PathBuf::from(f.str()?),
            tt::TITLE => t.title = f.str()?.to_string(),
            tt::ARTIST => t.artist = f.str()?.to_string(),
            tt::ALBUM => t.album = f.str()?.to_string(),
            tt::GENRE => t.genre = f.str()?.to_string(),
            tt::COMMENT => t.comment = f.str()?.to_string(),
            tt::BPM_TAG => t.bpm_tag = f.f32(),
            tt::KEY_TAG => t.key_tag = f.str().map(str::to_string),
            tt::RATE => t.sample_rate = f.u32()?,
            tt::DURATION => t.duration_secs = f.f64()?,
            tt::SIZE => t.file_size = f.u64()?,
            tt::MTIME => t.file_mtime = f.u64()?,
            tt::GRID_BPM => t.grid.bpm = f.f32()?,
            tt::GRID_ANCHOR => t.grid.anchor_frame = f.f64()?,
            tt::GRID_LOCKED => t.grid.locked = f.bool()?,
            tt::ADDED => t.added = f.u64()?,
            tt::MISSING => t.missing = f.bool()?,
            _ => {}
        }
    }
    if has_id { Some(t) } else { None }
}

// ── mutation encoding ────────────────────────────────────────────────────

mod mt {
    pub const KIND: u16 = 1;
    pub const TRACK: u16 = 2;
    pub const ID: u16 = 3;
    pub const BPM: u16 = 4;
    pub const ANCHOR: u16 = 5;
    pub const PATH: u16 = 6;
    pub const FLAG: u16 = 7;
    pub const SIZE: u16 = 8;
    pub const MTIME: u16 = 9;
    pub const BEFORE: u16 = 10;
    pub const PLAYLIST: u16 = 11;
    pub const NAME: u16 = 12;
}

impl Mutation {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::new();
        match self {
            Mutation::Upsert(t) => {
                w.u8(mt::KIND, 1);
                w.blob(mt::TRACK, encode_track(t));
            }
            Mutation::Remove(id) => {
                w.u8(mt::KIND, 2);
                w.u64(mt::ID, id.0);
            }
            Mutation::SetGrid { id, grid } => {
                w.u8(mt::KIND, 3);
                w.u64(mt::ID, id.0);
                w.f32(mt::BPM, grid.bpm);
                w.f64(mt::ANCHOR, grid.anchor_frame);
                w.bool(mt::FLAG, grid.locked);
            }
            Mutation::SetPath { id, path, file_size, file_mtime } => {
                w.u8(mt::KIND, 4);
                w.u64(mt::ID, id.0);
                w.str(mt::PATH, &path.to_string_lossy());
                w.u64(mt::SIZE, *file_size);
                w.u64(mt::MTIME, *file_mtime);
            }
            Mutation::SetMissing { id, missing } => {
                w.u8(mt::KIND, 5);
                w.u64(mt::ID, id.0);
                w.bool(mt::FLAG, *missing);
            }
            Mutation::AddRoot(p) => {
                w.u8(mt::KIND, 6);
                w.str(mt::PATH, &p.to_string_lossy());
            }
            Mutation::RemoveRoot(p) => {
                w.u8(mt::KIND, 7);
                w.str(mt::PATH, &p.to_string_lossy());
            }
            Mutation::Move { id, before } => {
                w.u8(mt::KIND, 8);
                w.u64(mt::ID, id.0);
                if let Some(b) = before {
                    w.u64(mt::BEFORE, b.0);
                }
            }
            Mutation::CreatePlaylist { id, name } => {
                w.u8(mt::KIND, 9);
                w.u64(mt::PLAYLIST, id.0);
                w.str(mt::NAME, name);
            }
            Mutation::RenamePlaylist { id, name } => {
                w.u8(mt::KIND, 10);
                w.u64(mt::PLAYLIST, id.0);
                w.str(mt::NAME, name);
            }
            Mutation::DeletePlaylist(id) => {
                w.u8(mt::KIND, 11);
                w.u64(mt::PLAYLIST, id.0);
            }
            Mutation::PlaylistPlace { id, track, before } => {
                w.u8(mt::KIND, 12);
                w.u64(mt::PLAYLIST, id.0);
                w.u64(mt::ID, track.0);
                if let Some(b) = before {
                    w.u64(mt::BEFORE, b.0);
                }
            }
            Mutation::PlaylistRemove { id, track } => {
                w.u8(mt::KIND, 13);
                w.u64(mt::PLAYLIST, id.0);
                w.u64(mt::ID, track.0);
            }
        }
        w.finish()
    }

    pub fn decode(bytes: &[u8]) -> Option<Mutation> {
        let (mut kind, mut track, mut id, mut bpm, mut anchor, mut path, mut flag, mut size, mut mtime) =
            (0u8, None, TrackId(0), 0.0f32, 0.0f64, PathBuf::new(), false, 0u64, 0u64);
        let mut before = None;
        let mut playlist = PlaylistId(0);
        let mut name = String::new();
        for e in Entries::new(bytes) {
            match e.tag {
                mt::BEFORE => before = Some(TrackId(e.u64()?)),
                mt::PLAYLIST => playlist = PlaylistId(e.u64()?),
                mt::NAME => name = e.str()?.to_string(),
                mt::KIND => kind = e.u8()?,
                mt::TRACK => track = decode_track(e),
                mt::ID => id = TrackId(e.u64()?),
                mt::BPM => bpm = e.f32()?,
                mt::ANCHOR => anchor = e.f64()?,
                mt::PATH => path = PathBuf::from(e.str()?),
                mt::FLAG => flag = e.bool()?,
                mt::SIZE => size = e.u64()?,
                mt::MTIME => mtime = e.u64()?,
                _ => {}
            }
        }
        Some(match kind {
            1 => Mutation::Upsert(track?),
            2 => Mutation::Remove(id),
            3 => Mutation::SetGrid { id, grid: Grid { bpm, anchor_frame: anchor, locked: flag } },
            4 => Mutation::SetPath { id, path, file_size: size, file_mtime: mtime },
            5 => Mutation::SetMissing { id, missing: flag },
            6 => Mutation::AddRoot(path),
            7 => Mutation::RemoveRoot(path),
            8 => Mutation::Move { id, before },
            9 => Mutation::CreatePlaylist { id: playlist, name },
            10 => Mutation::RenamePlaylist { id: playlist, name },
            11 => Mutation::DeletePlaylist(playlist),
            12 => Mutation::PlaylistPlace { id: playlist, track: id, before },
            13 => Mutation::PlaylistRemove { id: playlist, track: id },
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn sample_track(n: u64) -> Track {
        Track {
            id: TrackId(n),
            path: PathBuf::from(format!("/music/{n}.wav")),
            file_size: 1000 + n,
            file_mtime: 5,
            title: format!("Track {n}"),
            artist: "Alva".into(),
            bpm_tag: Some(140.0),
            key_tag: Some("8A".into()),
            sample_rate: 44_100,
            duration_secs: 123.4,
            grid: Grid { bpm: 140.02, anchor_frame: 12345.5, locked: true },
            added: 99,
            ..Default::default()
        }
    }

    #[test]
    fn every_mutation_roundtrips() {
        let ms = vec![
            Mutation::Upsert(sample_track(1)),
            Mutation::Remove(TrackId(2)),
            Mutation::SetGrid { id: TrackId(3), grid: Grid { bpm: 150.0, anchor_frame: 1.0, locked: false } },
            Mutation::SetPath { id: TrackId(4), path: "/x/y.wav".into(), file_size: 7, file_mtime: 8 },
            Mutation::SetMissing { id: TrackId(5), missing: true },
            Mutation::AddRoot("/music".into()),
            Mutation::RemoveRoot("/music".into()),
            Mutation::Move { id: TrackId(6), before: Some(TrackId(1)) },
            Mutation::Move { id: TrackId(6), before: None },
            Mutation::CreatePlaylist { id: PlaylistId(9), name: "Set".into() },
            Mutation::RenamePlaylist { id: PlaylistId(9), name: "Set 2".into() },
            Mutation::DeletePlaylist(PlaylistId(9)),
            Mutation::PlaylistPlace { id: PlaylistId(9), track: TrackId(1), before: Some(TrackId(2)) },
            Mutation::PlaylistPlace { id: PlaylistId(9), track: TrackId(1), before: None },
            Mutation::PlaylistRemove { id: PlaylistId(9), track: TrackId(1) },
        ];
        for m in ms {
            assert_eq!(Mutation::decode(&m.encode()), Some(m));
        }
        assert_eq!(Mutation::decode(b"garbage"), None);
    }

    #[test]
    fn apply_sequence() {
        let mut lib = Library::default();
        Mutation::Upsert(sample_track(1)).apply(&mut lib);
        Mutation::Upsert(sample_track(2)).apply(&mut lib);
        Mutation::SetGrid { id: TrackId(1), grid: Grid { bpm: 100.0, anchor_frame: 0.0, locked: true } }.apply(&mut lib);
        Mutation::Remove(TrackId(2)).apply(&mut lib);
        Mutation::AddRoot("/m".into()).apply(&mut lib);
        assert_eq!(lib.len(), 1);
        assert_eq!(lib.get(TrackId(1)).unwrap().grid.bpm, 100.0);
        assert_eq!(lib.roots, vec![PathBuf::from("/m")]);
    }

    #[test]
    fn playlists_lifecycle() {
        let mut lib = Library::default();
        for n in 1..=3 {
            Mutation::Upsert(sample_track(n)).apply(&mut lib);
        }
        let pl = PlaylistId(77);
        Mutation::CreatePlaylist { id: pl, name: "Opening".into() }.apply(&mut lib);
        Mutation::PlaylistPlace { id: pl, track: TrackId(1), before: None }.apply(&mut lib);
        Mutation::PlaylistPlace { id: pl, track: TrackId(2), before: None }.apply(&mut lib);
        Mutation::PlaylistPlace { id: pl, track: TrackId(3), before: Some(TrackId(1)) }.apply(&mut lib);
        Mutation::PlaylistPlace { id: pl, track: TrackId(99), before: None }.apply(&mut lib); // unknown track ignored
        assert_eq!(lib.playlist(pl).unwrap().tracks, vec![TrackId(3), TrackId(1), TrackId(2)]);
        Mutation::PlaylistPlace { id: pl, track: TrackId(2), before: Some(TrackId(3)) }.apply(&mut lib);
        assert_eq!(lib.playlist(pl).unwrap().tracks, vec![TrackId(2), TrackId(3), TrackId(1)]);
        Mutation::PlaylistRemove { id: pl, track: TrackId(3) }.apply(&mut lib);
        Mutation::Remove(TrackId(1)).apply(&mut lib); // leaving the collection leaves playlists
        assert_eq!(lib.playlist(pl).unwrap().tracks, vec![TrackId(2)]);
        Mutation::RenamePlaylist { id: pl, name: "Closing".into() }.apply(&mut lib);
        assert_eq!(lib.playlist(pl).unwrap().name, "Closing");
        Mutation::DeletePlaylist(pl).apply(&mut lib);
        assert!(lib.playlist(pl).is_none());
    }

    #[test]
    fn manual_order_moves() {
        let mut lib = Library::default();
        for n in 1..=4 {
            Mutation::Upsert(sample_track(n)).apply(&mut lib);
        }
        assert_eq!(lib.order, vec![TrackId(1), TrackId(2), TrackId(3), TrackId(4)]);
        Mutation::Move { id: TrackId(4), before: Some(TrackId(2)) }.apply(&mut lib);
        assert_eq!(lib.order, vec![TrackId(1), TrackId(4), TrackId(2), TrackId(3)]);
        Mutation::Move { id: TrackId(1), before: None }.apply(&mut lib);
        assert_eq!(lib.order, vec![TrackId(4), TrackId(2), TrackId(3), TrackId(1)]);
        Mutation::Remove(TrackId(2)).apply(&mut lib);
        assert_eq!(lib.order, vec![TrackId(4), TrackId(3), TrackId(1)]);
        assert_eq!(lib.positions()[&TrackId(1)], 2);
    }
}
