# 03 — Analysis

`lmx_analysis` turns decoded audio into everything the DJ needs to know about a track
before playing it: tempo and beatgrid, musical key, waveform summaries, loudness. It is
a pure, std-only, job-shaped crate — `analyze(&TrackAudio, &Options, cancel) ->
Analysis` — run on worker threads, never on the audio thread.

Results are cached per track ([04-LIBRARY](04-LIBRARY.md)) and versioned by
`ANALYZER_VERSION`; bumping it invalidates and re-queues.

## Pipeline

```
 TrackAudio (stereo f32)
   │
   ├─► mono downmix, 44.1 k (resample if needed) ──► STFT (win 2048, hop 512, Hann; lmx_dsp::Fft)
   │                                                   │
   │                                                   ├─► Onset detection function ─► Tempo ─► Beatgrid
   │                                                   │                                        └─► Downbeats (later)
   │                                                   └─► Chroma (12 bins/octave) ─► Key
   │
   ├─► Waveform summary (fine + overview), 3-band RGB
   └─► Loudness (integrated RMS, peak, suggested autogain)
```

STFT frames are computed once and shared by the tempo and key stages.

## Tempo and beatgrid

The material is bass music: 140 BPM with half-time drums, where the obvious
periodicity is 70. Getting the octave right matters more than 0.01 BPM precision.

1. **Onset detection function (ODF)**: spectral flux — per frame, sum of positive
   differences of log-magnitude between consecutive frames, weighted toward
   20–200 Hz and 2–8 kHz (kick + snare/hat), high-pass filtered and normalized.
   Frame rate = 44100 / 512 ≈ 86 Hz.
2. **Period estimate**: autocorrelation of the ODF over 8-second windows (stepping
   4 s), combined with a comb-filter bank scoring candidate periods for 60–200 BPM.
   Scores across windows are summed into a global tempo histogram.
3. **Octave resolution**: candidates at T, T/2, 2T are compared with a prior favoring
   the configured range (default **100–180 BPM**, per-genre presets). When the
   half-time and double-time interpretations score within 10 %, prefer the one inside
   the range; expose the alternative in the grid editor as a one-click "×2 / ÷2".
4. **Phase / anchor**: with the period fixed, cross-correlate the ODF against a beat
   comb to find the beat phase; pick the strongest onset near the first beat as the
   `anchor_frame`.
5. **Grid type**: if the per-window period estimates agree within ±0.5 % the grid is
   `Constant { anchor_frame, bpm }` (nearly all electronic music). Otherwise a
   `Dynamic(Vec<beat_frame>)` grid is produced by tracking beats window to window
   (dynamic programming over onset strength + tempo continuity). The engine and UI
   handle both ([02-AUDIO-ENGINE](02-AUDIO-ENGINE.md)).
6. **Downbeats** (later phase): bar detection from low-frequency energy patterns and
   chord-change positions; until then bar = every 4th beat from the anchor, adjustable
   by the user.

Precision goal: ±0.02 BPM on constant-tempo material (a 6-minute track drifts < 1 beat
otherwise). The ODF resolution (11.6 ms) is refined by parabolic interpolation of the
autocorrelation peak and by least-squares fitting of the anchor over all detected beats.

**User correction** always wins: the grid editor lets the DJ set the anchor by tapping,
nudge it, set BPM by typing or by "halve/double", and lock the grid so re-analysis
never overwrites it ([04-LIBRARY](04-LIBRARY.md) stores `grid_locked`).

## Key

1. **Chroma**: from the shared STFT, map bins 55 Hz–4 kHz to the 12 pitch classes
   (log-frequency weighting, harmonic-summation with 4 harmonics to sharpen roots).
   Average over the whole track, and also per 30-second segment (for key changes /
   confidence).
2. **Matching**: correlate the mean chroma against 24 key profiles (Temperley /
   Krumhansl-Kessler major + minor, rotated). Best correlation → key; the margin over
   the runner-up → confidence.
3. **Output**: `Key { root: 0..12, minor: bool }` with conversions to Camelot (8A…)
   and Open Key (1m…) in `lmx_core`. Display defaults to Camelot with color coding.

Verification: a set of known-key tracks + synthesized chord progressions in tests;
target > 85 % agreement on real material (the industry ceiling is around 90 %).

## Waveform summary

What the deck and overview widgets draw. Computed at two resolutions:

- **Fine**: one column per **256 frames** (5.8 ms @ 44.1 k) → ~10k columns/minute.
- **Overview**: one column per **4096 frames** (~93 ms) → ~650 columns/minute.

Per column, per stereo side: `peak: u8`, `rms: u8`, and three band energies
`low/mid/high: u8` from a 3-band split (LR crossovers at 200 Hz / 2.5 kHz, same
`lmx_dsp::biquad` as the mixer EQ) — 5 bytes/side, 10 bytes/column. The GPU waveform
shader maps bands to RGB (Rekordbox-style: low = red-ish, mid = green/amber, high =
blue) and height to peak with an RMS core ([05-UI](05-UI.md)).

A 6-minute track: fine ≈ 620 kB, overview ≈ 39 kB. Uploaded once per load as textures.

## Loudness / autogain

Integrated RMS (K-weighted, gated — a LUFS approximation using `lmx_dsp::biquad` for
the K-filter), true peak, and a suggested `autogain_db` to bring tracks to a common
target (−14 LUFS-ish, user configurable). The mixer applies it at the trim stage when
"autogain" is on; the DJ's trim knob is relative to it.

## The `Analysis` record

```rust
struct Analysis {
    version: u32,
    grid: Beatgrid,            // Constant or Dynamic, plus confidence
    tempo_alternatives: [f32; 2], // halve/double candidates
    key: Key, key_confidence: f32, key_segments: Vec<(frame, Key)>,
    waveform_fine: Vec<[u8; 10]>, waveform_overview: Vec<[u8; 10]>,
    loudness_lufs: f32, true_peak_db: f32, autogain_db: f32,
    duration_frames: u64, sample_rate: u32,
}
```

Stored as one binary blob per track in the library's analysis cache; read fully into
memory on load (they are small). Format in [04-LIBRARY](04-LIBRARY.md).

## Jobs, priority, cancellation

- Loading a track for a deck with no analysis runs analysis at **high** priority so
  the grid exists before the DJ presses play; the waveform summary is computed first
  and streamed to the UI in chunks so the deck draws while tempo/key are still
  running.
- Background analysis of a freshly scanned folder runs at **low** priority, one job
  per worker, cancellable between STFT frames, resumable (the STFT is cheap; nothing
  is checkpointed).
- The Library shows analysis state per track (none / queued / running % / done /
  failed) and a global progress bar.

## Tooling

`tools/lmx_analyze <file>` prints tempo candidates with scores, the chosen grid, key
with confidence and the runner-up, loudness, and timing. It can dump the ODF and
chroma as CSV for eyeballing. This is how the detectors get tuned on Alva's actual
crate before any of it is trusted in the app.
