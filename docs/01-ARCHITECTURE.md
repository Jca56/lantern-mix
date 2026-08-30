# 01 — Architecture

The shape of the whole system: crates, threads, data flow, and the rules that keep the
real-time path safe. Every other doc hangs off this one.

## Crate map

One cargo workspace. Library crates are prefixed `lmx_`; the binary is `lantern-mix`.
Arrows point from dependent to dependency. **Nothing depends on `lmx_app`.**

```
                         ┌──────────────┐
                         │   lmx_app    │  winit loop · screens · wiring
                         └──────┬───────┘
     ┌────────┬───────────┬─────┴────┬───────────┬──────────┐
     ▼        ▼           ▼          ▼           ▼          ▼
 lmx_ui   lmx_audio   lmx_midi  lmx_library  lmx_analysis  lmx_engine
   │          │         │(cmds)     │            │  │          │
   │          │                     │            │  │          │
   ▼          ▼                     │            │  ▼          │
 lmx_gpu   lmx_engine               │            │ lmx_codec   │
   │          │                     │            │              │
   ▼          ▼                     ▼            ▼              ▼
 (wgpu,   lmx_rt ◄─────────────── lmx_core ◄──────────────── lmx_dsp
 lntrn-text)                (ids, types, time, hash, errors)   (std only)
```

| Crate | Responsibility | External deps |
|---|---|---|
| `lmx_core` | Shared vocabulary: `TrackId`, `DeckId`, `Bpm`, `Key`, `Camelot`, sample-time types, our hash, error type, config paths. No logic beyond conversions. | none |
| `lmx_rt` | Real-time-safe primitives: SPSC ring, triple buffer, atomic f32/f64 cells, `RtBox` deferred-drop, fixed-capacity `RtVec`. Unit-tested with loom-style stress tests (our own). | none |
| `lmx_dsp` | FFT, biquads (RBJ), TPT SVF, YIN, one-pole smoothers, resamplers (Hermite now, windowed-sinc later), time-stretch (WSOLA / phase vocoder, later), meters (peak/RMS/LUFS-ish), windows. Pure functions + small state structs. | none |
| `lmx_codec` | Decoders: WAV/RIFF, AIFF/AIFC, FLAC; later MP3. Metadata readers (INFO, ID3v2, Vorbis comments, artwork). Progressive decoding API. | none |
| `lmx_analysis` | Beatgrid/BPM, key, waveform summaries, loudness. Pure CPU, job-shaped: `fn analyze(audio) -> Analysis`. | none |
| `lmx_library` | Data model + storage (snapshot + journal), folder scanning, search/sort index, playlists/tags, history. Runs on the UI thread + workers; never on audio. | none |
| `lmx_engine` | The real-time audio graph: decks, mixer, FX, cue bus, master, sync clock, scratch/jog model, command/event queues. **No I/O, no allocation in `process()`.** | none |
| `lmx_audio` | Audio host: PipeWire stream setup, device/profile discovery, sample-rate negotiation, calls `Engine::process` from the RT callback. | `pipewire` |
| `lmx_midi` | Raw MIDI in/out over `/dev/snd/midi*`, hotplug scan, parser (running status, 14-bit CC), mapping engine, built-in GRV6 profile, LED feedback. Depends on `lmx_engine` only for the `EngineCommand` type. | none |
| `lmx_gpu` | wgpu 28 device/surface, painter (SDF shapes, textures, layers, clip), waveform shader, text via `lntrn-text`. | `wgpu`, `lntrn-text` |
| `lmx_ui` | Immediate-mode widget kit + theme + layout helpers + DJ widgets (waveform, overview, jog ring, pads, faders, meters, table, tree). | none (uses `lmx_gpu`) |
| `lmx_app` | The binary. winit event loop, repaint policy, screen composition (Performance / Library), wiring queues between subsystems, settings. | `winit` |
| `tools/lmx_midisniff` | Dumps raw MIDI from a device with timestamps. The first thing we run when the GRV6 is plugged in. | none |
| `tools/lmx_analyze` | CLI: analyze a file, print BPM/grid/key, dump waveform summary. For tuning the analyzers on real tracks. | none |

Rules for the map:
- `lmx_engine`, `lmx_dsp`, `lmx_codec`, `lmx_analysis`, `lmx_library`, `lmx_midi`, `lmx_rt`,
  `lmx_core` are **std-only** and unit-testable natively with `cargo test`.
- Only three crates touch the outside world: `lmx_audio` (PipeWire), `lmx_gpu` (GPU),
  `lmx_app` (window). Everything else is pure and could run headless.
- Versions are pinned once in `[workspace.dependencies]`; see [DECISIONS](DECISIONS.md).

## Threads

```
 ┌───────────────┐  commands (SPSC)   ┌──────────────────┐
 │  UI / main    │ ─────────────────► │   audio (RT)      │  PipeWire's RT thread
 │  winit loop   │ ◄───────────────── │   Engine::process │  never allocates/locks/blocks
 │  lmx_ui draw  │  events  (SPSC)    └───────┬──────────┘
 │  library      │ ◄──── snapshot ────────────┘  (triple buffer: positions, levels,
 └──┬──────┬─────┘                                phase, loop state — read each frame)
    │      │
    │      └── job queue ──► ┌───────────────────┐
    │                        │ worker pool (N-1)  │  decode · analyze · scan · load
    │      results ◄──────── └───────────────────┘  (std::thread + our own channel)
    │
    └─────────────────────── ┌───────────────────┐
        MIDI events (SPSC)   │ midi reader thread │  one per open device, blocking read
      (to UI *and* engine) ◄─┤ + hotplug scanner  │  timestamps at receipt
                             └───────────────────┘
```

- **UI thread** owns: the window, the GPU, the UI kit, the library, settings, and the
  *authoritative non-RT copy* of engine state (what the user asked for). It never
  blocks on anything but vsync/events.
- **Audio thread** owns: the engine graph and the *authoritative RT state* (where the
  play heads actually are). It receives commands, applies them at buffer boundaries,
  and publishes a snapshot per buffer.
- **MIDI thread(s)**: blocking `read()` on a rawmidi fd, parse, timestamp, fan out.
  Time-critical controls (jog ticks, faders, play/cue) go **straight to the engine
  queue**; the UI receives the same events for display and for non-RT actions (browse,
  load). The mapping layer decides which is which ([06-CONTROLLERS](06-CONTROLLERS.md)).
- **Workers**: `available_parallelism() - 1` threads pulling jobs from a priority
  queue. Loading a track for a deck is highest priority; background analysis lowest.
  Analysis jobs are cancellable (checked between STFT frames).

## Messaging

All cross-thread traffic uses `lmx_rt`:

- **`Spsc<T>`** — bounded single-producer/single-consumer ring, lock-free, `T: Copy`
  or `T` with no `Drop` side effects. Used for commands, events, MIDI.
- **`Triple<T>`** — triple buffer: writer publishes a full `T` snapshot, reader always
  gets the latest complete one without blocking. Used for engine → UI state.
- **`AtomicF32` / `AtomicF64`** — for continuously-controlled parameters that tolerate
  tearing between fields (a fader position, a knob).
- **`RtBox<T>`** — the audio thread must never free memory. Anything heap-owned that
  the engine swaps out (a previous track's audio, an old mapping table) is sent back
  to the UI thread through a "garbage" `Spsc` and dropped there.

Commands are an `enum EngineCommand` (load track into deck, play, cue, set rate, set
loop, set fader, ...). Events are an `enum EngineEvent` (track loaded, reached end,
loop toggled, sync master changed, xrun detected). Both are `Copy`-sized where
possible; a "load track" carries an `RtBox<TrackAudio>` pre-allocated by a worker.

## Data flow: from a file to a speaker

1. User drops a file / presses LOAD → UI enqueues a **LoadTrack** job.
2. Worker: `lmx_codec` decodes (progressively) into `TrackAudio { sample_rate,
   channels: 2, frames: Vec<f32> }` (interleaved, whole track, f32). If no analysis
   exists, an **Analyze** job is chained ([03-ANALYSIS](03-ANALYSIS.md)).
3. Worker hands the `Arc<TrackAudio>` back; UI sends `EngineCommand::Load { deck,
   audio: RtBox }`.
4. Engine swaps it into the deck at the next buffer boundary, returns the old one via
   the garbage queue.
5. Every buffer: each deck produces a stereo block (resample/stretch from its position),
   the mixer applies trim/EQ/color/fader/crossfader, FX units process their sends, the
   cue bus mixes PFL'd decks, the master limiter runs, and 4 channels (master L/R,
   cue L/R) — or 8 in hardware-mix mode — are written to PipeWire's buffer.
6. Engine publishes a `Snapshot` (positions, levels, loop/sync state, jog state).
7. UI reads the snapshot each frame and draws waveforms at the right offset.

## State ownership (one source of truth per fact)

| Fact | Owner | Others see it via |
|---|---|---|
| Track metadata, cues, grids, playlists | `lmx_library` on UI thread | UI reads directly; engine receives copies in commands |
| Play head position, actual rate, loop state | engine | `Snapshot` triple buffer |
| Fader/knob values the user set | UI (from mouse or MIDI) | `AtomicF32` cells the engine reads |
| Controller LED state | `lmx_midi` mapping layer | derived from `Snapshot` + UI state each frame |
| Settings | `lmx_app` | passed down on change |

## File conventions

- Files under 600 lines, flagged at 500; split into folder modules with `mod.rs`
  facades. Generated tables are the only exception and live in clearly named
  `*_tables.rs` files.
- Every crate has a `lib.rs` doc comment stating its responsibility and its RT-safety
  contract.
- `#![forbid(unsafe_code)]` everywhere except `lmx_rt` (atomics/ring internals) and
  the FFI edges in `lmx_audio` / `lmx_gpu`.
- No `unwrap()` on the audio thread; no `panic!` paths reachable from `process()`.

## Cross-machine build

The primary machine is an Arch laptop. The build must not depend on anything
Gentoo-specific. Two things to know when cloning fresh:

- `lntrn-text` is a **path dependency** on `../Lantern-DE/lntrn-text`; Lantern-DE must be
  checked out as a sibling directory. (It is never modified from here.)
- `pipewire-sys` needs `libpipewire-0.3` headers and `pkg-config` + `clang` (bindgen)
  at build time. Both machines have them.
