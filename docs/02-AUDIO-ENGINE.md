# 02 — Audio Engine

`lmx_engine` is the real-time heart. It is a pure, std-only crate: a struct with a
`process(&mut self, out: &mut [f32], frames: usize)` method that `lmx_audio` calls from
PipeWire's real-time thread. This doc is its contract.

## The RT contract

Inside `process()` — and everything it calls — the engine **never**:
allocates or frees, locks a mutex, blocks, does I/O, logs, formats strings, panics,
calls `Vec::push` on a `Vec` that might grow, or touches anything behind an `Arc`
that could be the last reference.

Everything it needs is pre-allocated at construction (`Engine::new(max_frames,
sample_rate)`) or delivered pre-allocated through commands (`RtBox`). Deallocation
goes back out through the garbage queue ([01-ARCHITECTURE](01-ARCHITECTURE.md)).

Denormals are flushed (FTZ/DAZ set on the RT thread by `lmx_audio`). All per-sample
math is f32; positions and time are f64.

## Graph

```
 Deck 1 ─┐                                 ┌─► Cue bus (PFL sum · cue/master blend) ─► out ch 3/4
 Deck 2 ─┼─► Channel strip ×4 ─► Crossfader ┼─► FX return ─► Master (gain · limiter) ─► out ch 1/2
 Deck 3 ─┤   trim · 3-band EQ ·             │
 Deck 4 ─┘   color FX · fader · PFL tap     └─► (hardware-mix mode: per-deck stems ─► out ch 1..8)
```

### Deck

```rust
struct Deck {
    audio: Option<RtBox<TrackAudio>>, // interleaved stereo f32, whole track
    grid:  Beatgrid,                  // copy of the library's grid (anchor + bpm, or beat list)
    pos:   f64,                       // play head in *source* frames (fractional)
    rate:  f64,                       // playback speed multiplier the deck is *at*
    target_rate: f64,                 // where the tempo fader + sync + nudge want it
    state: Playing | Paused | Cueing | Scratching,
    keylock: bool, stretch: TimeStretch,
    loop_: Option<Loop { in_, out, active }>, slip: Option<SlipState>,
    cues: [Option<f64>; 8], memory_cue: Option<f64>,
    jog: JogModel,
    smooth_rate: OnePole,             // rate changes are smoothed to avoid zipper/clicks
}
```

Per block a deck renders `frames` output samples by walking `pos` at `rate` through the
source and interpolating:

- **Keylock off**: 4-point Hermite interpolation (vinyl-style; pitch follows tempo).
  Windowed-sinc upgrade lives in `lmx_dsp::resample` if Alva hears aliasing at ±16 %.
- **Keylock on**: time-stretch. Two candidates in `lmx_dsp::stretch`, chosen by ear:
  1. **WSOLA** (waveform-similarity overlap-add) — cheap, transparent at ±8 %, the
     classic DJ choice.
  2. **Phase vocoder** with phase locking — better on wide ranges, more CPU, smearier
     on transients unless transient-preserving.
  Keylight's period-locked granular shifter is *not* the answer here (period locking
  only makes sense on monophonic input); it is mentioned so nobody tries it twice.
- Rate targets are smoothed by a one-pole (~20 ms) so nudges and sync corrections are
  inaudible.

Position semantics: `pos` is in source frames at the *track's* sample rate. Everything
user-facing (cues, loops, grid) is stored in source frames (f64) so it is independent
of the output sample rate. Conversion to seconds happens in the UI.

### Jog / scratch model (`JogModel`)

The controller sends relative ticks; the engine turns them into motion:

- **Vinyl mode, platter touched**: the deck is *slaved to the platter*. The engine
  integrates ticks into a platter angle; `rate = platter_velocity / nominal_velocity`,
  where `nominal` = ticks-per-revolution × (33⅓ RPM). Velocity is estimated from tick
  timestamps with a short window so a stopped platter reads 0 within ~10 ms. Release →
  rate ramps back to `target_rate` over a configurable "brake/release" time.
- **Pitch bend (platter side / vinyl off)**: `target_rate += k · velocity` while ticks
  arrive, decaying to 0 — a nudge.
- **Search mode** (shift + jog): jumps `pos` by beats.
- Ticks-per-revolution for the GRV6 is **measured** in Phase 0 with the sniffer
  ([06-CONTROLLERS](06-CONTROLLERS.md)); it is a controller-profile constant.

### Channel strip

`trim → 3-band EQ → color FX → fader → PFL tap`. 

- **EQ**: Pioneer-style isolator behaviour is the target: low/mid/high with full kill
  at minimum. Implementation: a Linkwitz-Riley crossover into three bands (two
  4th-order LR splits built from `lmx_dsp::biquad` pairs) with per-band gain, which
  gives true kill and phase-coherent recombination. Corner frequencies ~ 200 Hz and
  ~2.5 kHz (tunable). Gains −∞ .. +6 dB with the knob center = 0 dB.
- **Color FX** (one knob per channel, selectable): Filter (LP left / HP right, TPT SVF,
  resonance rises toward the ends), Dub Echo, Noise, Pitch — Filter first, others later.
- **Fader**: curve selectable (linear / smooth / sharp) applied to the MIDI value.
- **Crossfader**: assignable A / thru / B per channel; curve selectable; reverse.

### FX

Beat FX unit (Pioneer-style: one effect at a time, tempo-synced, assigned to a channel
or master): Echo, Delay, Reverb, Filter, Flanger, Phaser, Roll, Slip Roll, Trans,
Pitch. Beat length 1/16 … 8 beats from the sync master's BPM. Level/depth knob. FX
run at the master's tempo; Echo/Delay buffers are pre-allocated for 8 beats at 60 BPM.
The first three shipped: Echo, Filter, Roll.

### Cue bus and outputs

- Each channel has a PFL switch. Cue bus = Σ PFL'd post-EQ pre-fader signals, blended
  with master via the headphones MIX knob, headphones LEVEL applied. Written to output
  channels 3/4.
- Master = crossfader output → FX return → master gain → brick-wall limiter (look-ahead
  ~1 ms, pre-allocated delay) → output channels 1/2.
- **Hardware-mix mode**: if the GRV6 turns out to expose 8 outputs (mixing done in the
  controller, like a DDJ-1000), the engine bypasses its channel strips and writes each
  deck post-trim to its own stereo pair. This is decided by the audio host at device
  open time and passed to `Engine::set_output_mode`. Confirmed in Phase 0.
- **Booth** and **record** taps: master pre-limiter feeds the recorder (Phase 7).

## Time, tempo, sync

- **Beatgrid** per loaded track: either `Constant { anchor_frame, bpm }` or a `Vec` of
  beat frames for variable tempo (the engine holds a fixed-capacity copy, up to 64k
  beats). `beat_at(pos) -> (beat_index: i64, phase: f64)`.
- **Sync master**: the first deck that is playing and has sync engaged, or the one the
  user pins. Each synced deck sets `target_rate = master_bpm / own_bpm` and phase-
  corrects: if `phase_error > threshold` it nudges rate briefly (or jumps if the deck
  is not audible — i.e. fader down). Snap to beat, not to bar, unless "bar sync" is on.
- **Quantize**: cue / hot-cue / loop-in / loop-out / beat-jump actions snap to the
  nearest beat when engaged. Applies at the command level inside the engine (it knows
  the true position).
- **Nudge** and **tempo fader** modify `target_rate` for non-synced decks; the tempo
  fader range is selectable (±6 / ±10 / ±16 / wide).
- **Slip mode**: while looping/scratching/hot-cueing, a shadow position keeps advancing
  at `rate`; on release the deck jumps to it.

## Commands and snapshot

`EngineCommand` (UI/MIDI → engine, SPSC, applied at buffer boundaries — except jog
ticks, which carry timestamps and are integrated as they arrive):

`Load{deck, audio, grid}`, `Play`, `Pause`, `Cue{mode}`, `HotCue{n, action}`, `Seek{pos}`,
`SetTargetRate`, `SetKeylock`, `Sync{on/off/master}`, `LoopIn/Out/Toggle/Set{beats}`,
`BeatJump{beats}`, `Jog{ticks, touch, mode, t}`, `SetFader`, `SetXfader`, `SetEq`,
`SetColor`, `SetPfl`, `Fx{...}`, `SetOutputMode`, `Panic`.

`Snapshot` (engine → UI, triple buffer, per block): per deck `pos`, `rate`, `state`,
`beat/phase`, `loop`, `peak/rms L/R`; master/cue levels; sync master; xrun counter;
block timestamp (for UI interpolation between blocks).

`EngineEvent` (engine → UI, SPSC): `Loaded`, `EndOfTrack`, `LoopToggled`, `SyncMaster`,
`Xrun`, `HotCueHit` — things the UI must *react* to rather than merely display.

## Latency and buffer policy

- Sample rate follows the device: the GRV6 is 44.1 kHz; the engine runs at the device
  rate and tracks are resampled on the fly only if their rate differs (a 48 k WAV on a
  44.1 k device plays through the same interpolator with `rate *= 48000/44100`).
- Block size requested from PipeWire: 128 frames (~2.9 ms @ 44.1 k) by default, user
  selectable 64–1024. The engine is written to handle *any* block size ≤ `max_frames`
  (PipeWire may deliver variable quanta).
- Total round trip target on the GRV6: under 10 ms. Measured in Phase 2 with a
  loopback.

## Testing

`lmx_engine` is tested natively without any audio device: construct an `Engine`, feed
commands, call `process` on a scratch buffer, assert on positions, levels and
snapshot contents. Golden tests: a click track through a deck at rate 1.0 must be
sample-exact; at rate 1.05 with keylock the click spacing must change while the
click's spectrum does not.
