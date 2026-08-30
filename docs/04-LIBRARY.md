# 04 — Library

`lmx_library` is the collection: what tracks exist, what we know about them, how they
are organized, and how that survives restarts. It lives on the UI thread with worker
help for scanning and analysis. It is std-only and owns its own on-disk format.

## Data model

```rust
struct TrackId(u64);        // content hash — see "Identity"
struct Track {
    id: TrackId,
    path: PathBuf,           // current location; may change (moved files are re-found by id)
    file_size: u64, file_mtime: u64,
    format: Wav | Aiff | Flac | Mp3, sample_rate: u32, channels: u8, duration_frames: u64,
    // metadata (from tags, editable — edits never write back to the file)
    title, artist, album, genre, label, comment: String, year: Option<u16>,
    color: Option<u8>, rating: u8 /* 0..5 */,
    // DJ data
    grid: Beatgrid, grid_locked: bool, bpm_override: Option<f32>,
    key: Key, key_override: Option<Key>,
    cues: [Option<Cue>; 8],            // hot cues A..H
    memory_cues: Vec<Cue>,             // Rekordbox-style memory points, sorted
    loops: Vec<SavedLoop>,             // named loops (in/out frames)
    autogain_db: f32,
    tags: Vec<TagId>,                  // "My Tag"-style
    added: Timestamp, last_played: Option<Timestamp>, play_count: u32,
    analysis_state: None | Queued | Running(f32) | Done | Failed(String),
}
struct Cue { frame: f64, name: String, color: u8, kind: Hot | Memory | LoopIn/LoopOut }
struct Playlist { id, name, tracks: Vec<TrackId>, folder: Option<PlaylistFolderId> }
struct SmartPlaylist { id, name, rules: Vec<Rule> }     // bpm range, key, tag, rating, ...
struct Tag { id, name, group: String, color: u8 }
struct HistorySet { started: Timestamp, entries: Vec<(Timestamp, TrackId, DeckId)> }
struct Library { tracks, playlists, playlist_folders, smart_playlists, tags, history, roots: Vec<PathBuf> }
```

Everything a deck needs (audio, grid, cues, loops) is **copied** into the engine
command at load time; the library never shares references with the audio thread.

## Identity

`TrackId` = our 64-bit hash of (`file_size`, first 1 MiB of the *audio data* — after the
headers — and last 64 KiB). Not the whole file (WAV files are hundreds of MB) and not
the path (files move). Two byte-identical files are one track — that is a feature.
Retagging a FLAC changes the header but not the audio data, so the id survives it.
Re-encoding does change it; that's a new track (correct).

## Storage

Our own format; no SQLite, no serde. Location: `~/.local/share/lantern-mix/`.

```
 library.lmx        snapshot — the whole Library, written atomically
 library.lmx.log    journal — mutations since the snapshot, appended and fsync'd
 analysis/<id>.lmxa one Analysis blob per track (waveforms, grid, key, loudness)
 art/<id>.png       extracted cover art, if any (later)
```

- **Snapshot + journal**: every mutation (`AddTrack`, `SetCue`, `MoveGrid`,
  `AddToPlaylist`, …) is an enum variant serialized to the journal *first*, then
  applied in memory. On a clean exit — or every 5 minutes / 500 entries — the full
  snapshot is rewritten (write temp, fsync, rename) and the journal truncated. On
  startup: load snapshot, replay journal. Crash at any point loses at most the last
  un-fsync'd entry.
- **Encoding**: a tiny self-describing binary format in `lmx_library::format`:
  little-endian, length-prefixed fields with numeric tags (`tag: u16, len: u32,
  bytes`), unknown tags skipped. That gives forward/backward compatibility without a
  schema language: old app reads new file (ignores new fields), new app reads old file
  (missing fields default). Every file starts with magic `LMX1` + format version.
- **Analysis blobs** use the same encoding; they are immutable per
  `(track_id, ANALYZER_VERSION)` and regenerated when either changes. User edits
  (grid moves, cues) live in the *library*, not in the blob, so re-analysis never
  destroys them.
- **Sizes**: 10k tracks ≈ a 5 MB snapshot; analysis ≈ 700 kB/track. Whole library
  stays in RAM; analysis blobs load on demand (deck load, waveform preview in the
  table) with an LRU of ~200.

## Scanning and watching

- Roots: folders the user adds. Scanning walks them (workers), identifies audio files
  by extension + magic, computes ids, reads metadata ([07-DECODERS](07-DECODERS.md))
  and enqueues analysis. Moved/renamed files are matched by id and their `path`
  updated; missing files are flagged, not deleted, until the user says so.
- Rescan on demand and at startup (cheap: stat-compare `size`/`mtime` before hashing).
  Filesystem watching (inotify) is a later nicety; explicit rescans are enough.
- Files dropped onto a deck or the library from outside the roots are added as
  individual tracks.

## Search, sort, filter

All in memory, rebuilt on load and updated incrementally:

- **Search**: case-folded token index over title/artist/album/genre/label/comment/tag
  names; prefix matching per token ("skr bang" matches "Skrillex – Bangarang"). Also
  numeric filters: BPM range (with ±% around a deck's tempo — "compatible tempo"),
  key compatibility (same, ±1 Camelot, relative major/minor — "compatible key"),
  rating, color, date added.
- **Sort**: any column; stable; remembers per view.
- **Views**: Collection, a Playlist, a Smart playlist, a Tag, a Folder (roots browse),
  History. Each view is a `Vec<TrackId>` + sort + filter → the table's row source
  ([05-UI](05-UI.md) virtualizes the rows).

## Playlists, folders, tags, history

- Playlists are ordered; drag to reorder; a track may appear in many.
- Playlist folders nest.
- Smart playlists evaluate rules live over the collection.
- Tags have groups (Genre / Components / Situation / Mood by default, editable) and
  colors, and are filterable from the browser.
- History logs every deck load with time and deck; a "set" starts on app launch or
  when the user says "new set"; sets are exportable as text.

## API shape

```rust
impl Library {
    fn open(dir) -> Result<Library>;
    fn apply(&mut self, m: Mutation) -> Result<()>;   // journals then mutates
    fn track(&self, id) -> Option<&Track>;
    fn view(&self, v: &View) -> &[TrackId];           // cached
    fn search(&self, q: &Query) -> Vec<TrackId>;
    fn analysis(&mut self, id) -> Option<Arc<Analysis>>; // LRU-cached blob load
    fn snapshot(&mut self) -> Result<()>;
}
```

Mutations are the only way to change state, which also makes undo (a stack of inverse
mutations) and the journal the same mechanism.

## Testing

Pure crate: round-trip every struct through the format; journal replay equals
snapshot; corrupted trailing journal entry is ignored, not fatal; unknown-tag files
load; search index matches a brute-force filter on 10k synthetic tracks; id stability
across a re-tag (synthesized FLAC with different Vorbis comments, same audio).
