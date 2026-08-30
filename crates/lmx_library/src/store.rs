//! Snapshot + journal on disk; atomic rewrite; replay on open.
//!
//! `library.lmx` holds the whole collection; `library.lmx.log` holds mutations
//! since that snapshot. Every mutation is journaled (and fsync'd) before it is
//! applied in memory; the snapshot is rewritten on request (clean exit) and
//! automatically every `SNAPSHOT_EVERY` journal entries.

use crate::format::{self, Entries, Writer};
use crate::model::Library;
use crate::mutation::{decode_track, encode_track, Mutation};
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

pub const SNAPSHOT_FILE: &str = "library.lmx";
pub const JOURNAL_FILE: &str = "library.lmx.log";
pub const FORMAT_VERSION: u32 = 1;
const SNAPSHOT_EVERY: usize = 200;

mod lt {
    pub const ROOT: u16 = 1;
    pub const TRACK: u16 = 2;
    pub const ORDER: u16 = 3;
}

pub struct Store {
    dir: PathBuf,
    journal: BufWriter<File>,
    entries_since_snapshot: usize,
}

fn encode_library(lib: &Library) -> Vec<u8> {
    let mut w = Writer::new();
    for r in &lib.roots {
        w.str(lt::ROOT, &r.to_string_lossy());
    }
    for t in &lib.tracks {
        w.blob(lt::TRACK, encode_track(t));
    }
    for id in &lib.order {
        w.u64(lt::ORDER, id.0);
    }
    w.finish()
}

fn decode_library(body: &[u8]) -> Library {
    let mut lib = Library::default();
    for e in Entries::new(body) {
        match e.tag {
            lt::ROOT => {
                if let Some(s) = e.str() {
                    lib.roots.push(PathBuf::from(s));
                }
            }
            lt::TRACK => {
                if let Some(t) = decode_track(e) {
                    lib.tracks.push(t);
                }
            }
            lt::ORDER => {
                if let Some(v) = e.u64() {
                    lib.order.push(crate::model::TrackId(v));
                }
            }
            _ => {}
        }
    }
    lib.rebuild_index();
    lib
}

impl Store {
    /// Load (or create) the library in `dir`: snapshot, then journal replay.
    pub fn open(dir: &Path) -> io::Result<(Library, Store)> {
        std::fs::create_dir_all(dir)?;
        let snap = dir.join(SNAPSHOT_FILE);
        let mut lib = match File::open(&snap) {
            Ok(mut f) => match format::read_container(&mut f) {
                Ok((_version, body)) => decode_library(&body),
                Err(e) => {
                    eprintln!("lmx_library: snapshot unreadable ({e}); starting empty");
                    Library::default()
                }
            },
            Err(_) => Library::default(),
        };
        let jpath = dir.join(JOURNAL_FILE);
        let mut replayed = 0;
        if let Ok(data) = std::fs::read(&jpath) {
            for rec in format::read_records(&data) {
                if let Some(m) = Mutation::decode(rec) {
                    m.apply(&mut lib);
                    replayed += 1;
                }
            }
        }
        lib.rebuild_index();
        let journal = BufWriter::new(OpenOptions::new().create(true).append(true).open(&jpath)?);
        Ok((lib, Store { dir: dir.to_path_buf(), journal, entries_since_snapshot: replayed }))
    }

    /// Journal `m` (durably), then apply it to `lib`.
    pub fn apply(&mut self, lib: &mut Library, m: Mutation) -> io::Result<()> {
        format::write_record(&mut self.journal, &m.encode())?;
        self.journal.flush()?;
        self.journal.get_ref().sync_data()?;
        m.apply(lib);
        self.entries_since_snapshot += 1;
        if self.entries_since_snapshot >= SNAPSHOT_EVERY {
            self.snapshot(lib)?;
        }
        Ok(())
    }

    /// Rewrite the snapshot atomically and truncate the journal.
    pub fn snapshot(&mut self, lib: &Library) -> io::Result<()> {
        let snap = self.dir.join(SNAPSHOT_FILE);
        let tmp = self.dir.join(format!("{SNAPSHOT_FILE}.tmp"));
        {
            let mut f = BufWriter::new(File::create(&tmp)?);
            format::write_container(&mut f, FORMAT_VERSION, &encode_library(lib))?;
            f.flush()?;
            f.get_ref().sync_all()?;
        }
        std::fs::rename(&tmp, &snap)?;
        let jpath = self.dir.join(JOURNAL_FILE);
        self.journal = BufWriter::new(OpenOptions::new().create(true).write(true).truncate(true).open(&jpath)?);
        self.entries_since_snapshot = 0;
        Ok(())
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Grid, Track, TrackId};

    fn track(n: u64) -> Track {
        Track { id: TrackId(n), path: format!("/m/{n}.wav").into(), title: format!("T{n}"), ..Default::default() }
    }

    fn tmpdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("lmx_store_{}_{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn journal_replays_and_snapshot_compacts() {
        let dir = tmpdir("replay");
        {
            let (mut lib, mut st) = Store::open(&dir).unwrap();
            st.apply(&mut lib, Mutation::AddRoot("/m".into())).unwrap();
            st.apply(&mut lib, Mutation::Upsert(track(1))).unwrap();
            st.apply(&mut lib, Mutation::Upsert(track(2))).unwrap();
            st.apply(&mut lib, Mutation::SetGrid { id: TrackId(1), grid: Grid { bpm: 140.0, anchor_frame: 3.0, locked: true } }).unwrap();
            st.apply(&mut lib, Mutation::Move { id: TrackId(2), before: Some(TrackId(1)) }).unwrap();
            // no snapshot: everything lives in the journal
        }
        {
            let (mut lib, mut st) = Store::open(&dir).unwrap();
            assert_eq!(lib.len(), 2);
            assert_eq!(lib.order, vec![TrackId(2), TrackId(1)]);
            assert_eq!(lib.get(TrackId(1)).unwrap().grid.bpm, 140.0);
            assert_eq!(lib.roots, vec![PathBuf::from("/m")]);
            st.snapshot(&lib).unwrap();
            assert_eq!(std::fs::metadata(dir.join(JOURNAL_FILE)).unwrap().len(), 0);
            st.apply(&mut lib, Mutation::Remove(TrackId(2))).unwrap();
        }
        {
            let (lib, _st) = Store::open(&dir).unwrap();
            assert_eq!(lib.len(), 1);
            assert_eq!(lib.get(TrackId(1)).unwrap().title, "T1");
            assert_eq!(lib.order, vec![TrackId(1)]);
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn torn_journal_tail_is_ignored() {
        let dir = tmpdir("torn");
        {
            let (mut lib, mut st) = Store::open(&dir).unwrap();
            st.apply(&mut lib, Mutation::Upsert(track(1))).unwrap();
            st.apply(&mut lib, Mutation::Upsert(track(2))).unwrap();
        }
        let j = dir.join(JOURNAL_FILE);
        let mut data = std::fs::read(&j).unwrap();
        data.truncate(data.len() - 5);
        std::fs::write(&j, &data).unwrap();
        let (lib, _st) = Store::open(&dir).unwrap();
        assert_eq!(lib.len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn corrupt_snapshot_starts_empty_not_dead() {
        let dir = tmpdir("corrupt");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(SNAPSHOT_FILE), b"LMX1\0\0\0\0trash").unwrap();
        let (lib, _st) = Store::open(&dir).unwrap();
        assert!(lib.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }
}
