# 08 — Roadmap

Phases, each with a **"done when"** that Alva can verify at the machine. Nothing in a
later phase is allowed to require rewriting an earlier one — that is what the design
docs are for. Phases are ordered by *foundation risk*: the things that could invalidate
the architecture (audio host, controller protocol, GPU/text stack) come first.

Crate names refer to [01-ARCHITECTURE](01-ARCHITECTURE.md).

## Phase 0 — Foundation proof (no DJ features yet)

Build the three risky edges and the primitives everything else stands on.

- `lmx_rt`: SPSC ring, triple buffer, atomic cells, `RtBox` + garbage queue. Stress
  tests with producer/consumer threads.
- `lmx_dsp`: import the keepers (FFT, RBJ biquads, TPT SVF, YIN) with tests; add
  one-pole smoother, Hermite interpolator, windows.
- `lmx_audio`: open a PipeWire stream at the device rate with 4 output channels,
  RT callback calls a stub engine. **Measure**: xruns over 10 minutes at 128 frames
  on both the laptop's internal card and the GRV6.
- `lmx_midi` + `tools/lmx_midisniff`: raw MIDI in/out, parser, hotplug. **Measure** the
  GRV6: fill the unknowns table in [06-CONTROLLERS](06-CONTROLLERS.md) (pads, LEDs,
  ticks/rev, deck toggle, connect dump, audio topology).
- `lmx_gpu` + `lmx_app`: winit window (Wayland), wgpu 28 surface, painter with SDF
  shapes, `lntrn-text` rendering big text, repaint-on-demand loop that draws nothing at
  idle.

**Done when:** a window shows a sine-wave test tone's level meter driven from the RT
thread, a big "LANTERN MIX" in `lntrn-text`, and moving the GRV6's crossfader moves an
on-screen fader — with 0 xruns and ~0 % CPU when idle. And the sniffer log has answered
every "measure" item.

## Phase 1 — One deck plays

- `lmx_codec`: WAV + AIFF decoders + metadata; `decode_all` with progress.
- `lmx_engine`: `Deck` with Hermite playback, play/pause/cue/seek, rate from tempo
  fader, jog model (vinyl scratch + bend), commands + snapshot.
- `lmx_analysis`: waveform summary only (fine + overview).
- `lmx_ui`: deck header, scrolling waveform (GPU pipeline), overview, transport
  buttons, readouts. Drop a file onto the deck to load.

**Done when:** Alva drops a WAV on deck 1, sees the colored waveform, presses PLAY on
the GRV6, scratches, nudges, uses the tempo fader, and it sounds like a CDJ with
keylock off. Position readout matches the audio to the sample.

## Phase 2 — Two decks and a mixer

- Channel strips (trim, 3-band kill EQ, fader, PFL), crossfader with curves, cue bus
  to outputs 3/4, master limiter; hardware-mix mode if the GRV6 turned out to be 8-out.
- Full GRV6 mapping for decks 1–2 + mixer + browse/load; LED feedback for play/cue/
  sync/PFL; soft takeover.
- Two-deck performance layout.

**Done when:** a two-deck blend mixed entirely on the controller, cueing in
headphones, with EQ kills clean, no clicks on fader moves, and a measured round-trip
latency under 10 ms.

## Phase 3 — Know the music (analysis + library)

- `lmx_analysis`: tempo/beatgrid with octave prior, key, loudness; `tools/lmx_analyze`.
- `lmx_library`: data model, format, snapshot + journal, scanning roots, search/sort/
  filter, playlists/folders/tags, history.
- Browser UI: tree + virtualized table (52 px rows), search field, BPM/key filters,
  analysis progress; load from the browser via the GRV6 encoder.

**Done when:** Alva's real crate is scanned, analyzed in the background, searchable,
and BPM/key are right on the tracks Alva knows (tuned with `lmx_analyze`); a grid that
is wrong can be fixed in the editor and stays fixed.

## Phase 4 — Sync, keylock, loops, cues, quantize

- Beat sync + phase alignment + sync master; quantize; beat jump; nudge.
- Keylock: WSOLA first, phase vocoder if it earns its place — chosen by ear.
- Hot cues (8, with pad LEDs), memory cues, auto/manual loops, loop roll, slip mode.
- Waveform overlays: beats, bars, cues, loop region, other-deck beat markers; beat-
  phase dots.

**Done when:** a synced blend with keylock on both decks sounds clean at ±8 %, hot cues
fire quantized from the pads with correct colors, and loops sit dead on the grid.

## Phase 5 — Four decks and FX

- Decks 3/4, 4-deck layout, deck toggles on the controller.
- Beat FX unit (Echo, Filter, Roll first; then Delay, Reverb, Flanger, Phaser, Trans,
  Pitch) with the FX section mapped; Color FX filter per channel.
- FX strip UI.

**Done when:** a four-deck routine with tempo-synced echo tails and filter sweeps from
the controller, no CPU spikes (profile the FX at 128 frames).

## Phase 6 — FLAC and MP3, load-while-decoding

- FLAC decoder (native), MP3 decoder (native), ID3v2, artwork.
- Progressive load: deck playable before decode completes.
- Rescan smarts (moved files, missing files).

**Done when:** the full collection loads regardless of format, with sample-accurate
decodes verified against fixtures, and a 10-minute FLAC is playable within a second of
pressing LOAD.

## Phase 7 — Prep tools and polish

- Library/prep screen: grid editor (tap, drag, ×2/÷2, lock), cue/loop editor, tag
  editor, smart playlists, set history export.
- Recording the master to WAV; autogain; settings screens; MIDI learn; mapping editor.
- Groove Circuit section (if Alva wants it) — needs its own design note once the
  sniffer shows what it sends.

**Done when:** Alva can prep an entire set without touching another program, record
it, and export what was played.

## Later / maybe

Downbeat & phrase detection, dynamic (variable tempo) grids in the editor, sampler
deck, key-shift pads, Rekordbox XML import, USB export (PDB), stems, streaming.
Each gets its own doc before any code.

## Working agreements

- Docs first for anything that touches the engine graph, the library format, or the
  mapping format; then code.
- Every crate keeps `cargo test` green natively; the app is tested by Alva at the
  machine with the GRV6. No screenshots, no automation of the UI.
- Commit at every milestone with a short message (feature name / fix).
- Files under 600 lines, flagged at 500.
