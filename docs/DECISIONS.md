# Decisions log

Short records of the choices that shape the project, with the reasoning, so future
us doesn't relitigate them by accident. Newest at the bottom. Date format: YYYY-MM-DD.

| # | Date | Decision | Why |
|---|---|---|---|
| D1 | 2026-08-29 | **Rust only.** | House rule for everything Lantern. |
| D2 | 2026-08-29 | **Dependency policy: pragmatic for the hard stuff.** DJ-specific code (engine, analysis, library, UI kit, MIDI, codecs) is ours; commodity plumbing may be a crate. | Ownership is the point, but weeks spent on Wayland protocol or PipeWire pod builders teach nothing about DJ software. |
| D3 | 2026-08-29 | **Sanctioned external crates (exhaustive):** `wgpu` 28, `winit` 0.30 (`rwh_06`), `pipewire` 0.10 (+ its `libspa`/`-sys` tree), and `lntrn-text` as a path dependency (which brings `lntrn-draw`, `lntrn-gfx`, `lntrn-theme`, `bytemuck`, `x11`, `pollster`, `raw-window-handle` transitively). Anything else is discussed first. | See D4–D7. |
| D4 | 2026-08-29 | **Own immediate-mode UI on wgpu**, no GUI framework. Heavy things (waveforms, glyphs, table rows) are cached/virtualized; render only on demand when idle. | A DJ app's widgets are all custom anyway; one source of truth for UI state; the screen animates while playing regardless. Idle rendering is solved by repaint-on-demand (laptop battery). |
| D5 | 2026-08-29 | **winit for windowing.** | Native Wayland + fractional scaling, keyboard maps, file drag-and-drop events. The Xlib-via-`x11` path used by some Lantern-DE apps works but runs through XWayland and lacks native drag-drop. |
| D6 | 2026-08-29 | **PipeWire via the official `pipewire` crate** rather than hand-rolled FFI or ALSA-direct. Audio host is behind our own trait so ALSA-direct can be added later. | Alva: audio quality, minimal latency, performance and stability matter enough that a proven binding is worth it. Both machines run PipeWire. |
| D7 | 2026-08-29 | **Text via `../Lantern-DE/lntrn-text`** (path dep, never modified from here). Pins `wgpu` at **28**. | It's ours already; from-scratch shaping/rasterizing/atlas. Requires Lantern-DE checked out as a sibling on every machine. |
| D8 | 2026-08-29 | **DSP keepers imported** from the plugin work: radix-2 FFT (`lantern_eq/fft.rs`), RBJ biquads (`lantern_eq/biquad.rs`), TPT SVF (`lantern_synth/synth.rs`), YIN (`lantern_keylight/tuner.rs`). Copied into `lmx_dsp` with tests; originals untouched. Nothing else from those repos is reused. | Alva's pick. The rest of that codebase is not a reference for this project. |
| D9 | 2026-08-29 | **Scope: full Rekordbox replacement** — library/prep and performance are both first-class. No MVP rush; foundation first. | Alva's framing of the project. |
| D10 | 2026-08-29 | **Controller: DDJ-GRV6**, plain MIDI, raw MIDI over `/dev/snd` with std I/O only; generic mapping engine with an own `.lmxmap` format. | Class-compliant, no HID needed; zero deps for the whole MIDI layer; works on any Linux. |
| D11 | 2026-08-29 | **Formats: WAV, AIFF, FLAC first; MP3 later.** All decoders hand-rolled. No AAC/OGG planned. | Collection is mostly WAV. MP3 is well-specified and worth owning; AAC is not on the menu. |
| D12 | 2026-08-29 | **Fresh library, own storage format** (snapshot + journal, tag-length-value encoding). No SQLite, no serde. No Rekordbox/Mixxx import, no USB export for now. | Starting fresh, controller only. Our format gives crash safety and forward/backward compatibility in a few hundred lines. |
| D13 | 2026-08-29 | **Performance layout: Rekordbox-style horizontal stacked waveforms**, 2 decks default, 4-deck view. | Alva's preference. |
| D14 | 2026-08-29 | **Primary target: Arch laptop** (PipeWire, Wayland). Gentoo desktop secondary. Nothing distro-specific in the build; idle power matters. | Where the app will actually be used. |
| D15 | 2026-08-29 | **Big text is a hard requirement**: body 25 px, readouts 45 px, buttons 55 px, nothing under 20 px — and **every size is a multiple of 5**. Palette direction: warm, not blue, high contrast (first on-screen review). No glows, no help text, no live debug counters. | Vision accessibility + Alva's first review of the Phase 0 window. |
| D16 | 2026-08-29 | **Verification is Alva at the machine** + native unit tests. No screenshots, no UI automation, no preview-image harnesses. | House rule. |
| D17 | 2026-08-29 | **Crate prefix `lmx_`**, binary `lantern-mix`. Trivially renamable if Alva prefers otherwise. | Short, unambiguous in a workspace with other Lantern projects. |
| D18 | 2026-08-29 | **Whole-track f32 audio in RAM** per loaded deck; `i16` storage as a fallback variant if ever needed. | ~10.6 MB/min; simplest possible RT read path; what the incumbents do. |
| D19 | 2026-08-29 | **Engine supports both software-mix (4-out) and hardware-mix (8-out) output modes**; which one the GRV6 needs is measured in Phase 0. | Unknown until the device is sniffed; designing for both costs one enum. |

## Open questions (answered by Phase 0 measurements, not by guessing)

- GRV6 (official MIDI list is in `docs/reference/`, transcribed in 06-CONTROLLERS): still
  unknown — jog ticks per revolution, pad LED color values, connect-time state dump,
  USB audio channel count and PipeWire profile names.
- Xrun behaviour at 128 frames on the laptop's PipeWire config (may need
  `PIPEWIRE_LATENCY`/quantum settings documented for users).
- Whether `lntrn-text`'s font discovery finds a suitable big UI font on Arch out of the
  box, or whether we embed one.
