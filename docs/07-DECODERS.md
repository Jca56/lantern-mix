# 07 — Decoders and metadata

`lmx_codec`: turn files into `TrackAudio` and `Metadata`. std-only, written by us.
Order of arrival: **WAV → AIFF → FLAC** (Alva's collection is mostly WAV), **MP3 later**.

## Common API

```rust
pub struct TrackAudio { pub sample_rate: u32, pub channels: u16 /* always 2 after decode */,
                        pub frames: Vec<f32> /* interleaved L R L R … */ }
pub struct Metadata { title, artist, album, genre, label, comment: Option<String>,
                      year: Option<u16>, bpm_tag: Option<f32>, key_tag: Option<String>,
                      artwork: Option<Vec<u8>> /* PNG/JPEG bytes */ }

pub fn probe(path) -> Result<Probe>            // format, sample_rate, channels, duration, metadata — header only, fast
pub fn open(path) -> Result<Box<dyn Decoder>>  // streaming decoder
pub trait Decoder {
    fn info(&self) -> &Probe;
    fn seek(&mut self, frame: u64) -> Result<()>;
    fn read(&mut self, out: &mut Vec<f32>, max_frames: usize) -> Result<usize>; // interleaved f32
}
pub fn decode_all(path, progress: impl FnMut(f32)) -> Result<(TrackAudio, Metadata)>
```

- Output is always stereo f32 in −1..1; mono is duplicated, >2 channels are
  downmixed (L/R only) — DJ material is stereo.
- `decode_all` is what a **LoadTrack** job calls; it reports progress so the UI can
  show a bar, and the deck can begin playing once the first N seconds are in (the
  `Vec` is reserved to the full length up front from the header, then filled in place
  — the engine is handed the buffer only when complete; "play while loading" is a
  Phase 6 refinement using a two-stage handoff).
- All decoders are **bounds-checked, never panic on malformed input**, and fuzzed with
  a small in-tree fuzzer (random mutations of fixture files must produce `Err`, never
  a crash).

## Memory

A whole track in RAM as f32 stereo costs ~10.6 MB per minute (44.1 k). Four decks with
six-minute tracks ≈ 250 MB, plus the preview deck. That is fine on any laptop that
runs a DJ set. If it ever isn't, `TrackAudio` gets an `i16` storage variant behind the
same read API; nothing else changes.

## WAV (RIFF / WAVE)

- Chunks: `RIFF` … `WAVE`, `fmt ` (PCM, IEEE float, and `WAVE_FORMAT_EXTENSIBLE` with
  its sub-format GUID), `data`, plus metadata chunks: `LIST/INFO` (INAM/IART/IPRD/
  IGNR/ICMT/ICRD), `id3 ` (an embedded ID3v2 tag — common from Rekordbox/Serato
  exports and Bandcamp), `cue `/`smpl` (ignored for now), `acid` (tempo hint).
- Sample formats: u8, i16, i24 (packed), i32, f32, f64; any channel count; any rate.
- `RF64`/`BW64` (>4 GB) later; flagged clearly if encountered.
- Odd-length chunk padding, `data` chunk size lies (streamed WAVs with 0/0xFFFFFFFF
  size) — handle by clamping to file length.

## AIFF / AIFF-C

- `FORM` … `AIFF`/`AIFC`; `COMM` (channels, frames, bits, 80-bit extended rate — our
  own float80 → f64), `SSND` (offset/blocksize), `ID3 ` chunk, `NAME`/`AUTH`/`ANNO`.
- PCM big-endian 8/16/24/32; AIFC compression `NONE`, `sowt` (little-endian),
  `fl32`/`fl64`. Anything else → unsupported error with the fourcc in the message.

## FLAC

A full native decoder (~1.5k lines), the biggest of the three:

- Container: `fLaC` marker, metadata blocks (`STREAMINFO`, `VORBIS_COMMENT`,
  `PICTURE`, `SEEKTABLE`, `PADDING`, `APPLICATION`), then frames.
- Frame decoding: sync code, header (block size, rate, channel assignment incl. the
  three stereo decorrelation modes, sample size, CRC-8), subframes (`CONSTANT`,
  `VERBATIM`, `FIXED` order 0–4, `LPC` order ≤ 32 with quantized coefficients), Rice
  residual partitions (both parameter widths, escape codes), wasted bits, CRC-16.
- Seeking via `SEEKTABLE` when present, else binary search on frame headers.
- Bit reader is our own, with a 64-bit cache; the whole thing must decode a 6-minute
  track in well under a second in release mode.
- Metadata: Vorbis comments (`TITLE`, `ARTIST`, `ALBUM`, `GENRE`, `LABEL`, `DATE`,
  `COMMENT`, `BPM`, `INITIALKEY`), `PICTURE` for artwork.

## MP3 (later — Phase 6)

Hand-rolled MPEG-1/2/2.5 Layer III: frame sync + header, side info, Huffman
(the standard tables), requantization, stereo (MS/intensity), reordering, alias
reduction, IMDCT (36/12), polyphase synthesis filterbank; Xing/Info/LAME header for
gapless (encoder delay/padding) and duration. ~3.5k lines. Verified against
reference decodes (sample-accurate within rounding). If it turns into a swamp, the
fallback under the pragmatic policy is a decoder crate — but it is well-specified and
worth owning. ID3v2.3/2.4 tags (text frames, `APIC` artwork, unsynchronisation) come
with it and are shared with WAV/AIFF's embedded ID3 chunks.

## Metadata reader (shared)

`lmx_codec::tags`: `id3v2` (2.3/2.4, text + APIC, UTF-16/UTF-8/Latin-1), `vorbis_comment`,
`riff_info`, `aiff_text`. All produce the same `Metadata`. Never writes to files: the
library owns edits ([04-LIBRARY](04-LIBRARY.md)).

## Test fixtures

Small generated files in `lmx_codec/tests/fixtures/`, produced by our own encoder
helpers in the test code (WAV/AIFF writers are trivial; for FLAC we keep a few tiny
real files, ≤ 100 kB, created from synthesized audio so the expected samples are
known). Golden tests assert sample-exact output for PCM and ≤ 1 LSB for float paths.
