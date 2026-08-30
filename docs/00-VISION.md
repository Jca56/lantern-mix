# 00 — Vision

**Lantern Mix** is a DJ application for Linux, written in Rust, built to be *ours*.

It exists because there is no Rekordbox on Linux and Mixxx is not the tool Alva wants
to perform on. It is not a clone of either. It is the DJ software we would build if
we were only building it for one DJ, one controller, and one laptop — and then making
sure the foundation could grow past that.

## Who it is for

- **The DJ:** Alva. Poor eyesight → every readout, waveform, button and hit target is
  *big*. This is a hard requirement, not a style (see [05-UI](05-UI.md)).
- **The controller:** an AlphaTheta / Pioneer **DDJ-GRV6** — 4-channel, USB-C,
  class-compliant audio, plain MIDI (see [06-CONTROLLERS](06-CONTROLLERS.md)).
- **The machine:** primarily an **Arch Linux laptop** (PipeWire, Wayland), secondarily
  the Gentoo desktop this was designed on. Battery life and hot laptops are real
  concerns; nothing may spin the CPU or GPU while idle.
- **The music:** mostly **WAV**, some FLAC/AIFF; MP3 comes later. Dubstep and bass
  music — 140 BPM, half-time feel — which is exactly the material where naive tempo
  detectors guess 70 (see [03-ANALYSIS](03-ANALYSIS.md)).

## What "done" looks like (the north star)

> Prepare a crate of tracks in Lantern Mix — analyzed, gridded, keyed, cued — and play
> a full set on the GRV6 with beat sync and key lock, headphone cueing, four decks,
> loops, hot cues and beat FX, without the app ever glitching, crashing, or making the
> DJ squint.

Everything in the roadmap ([08-ROADMAP](08-ROADMAP.md)) is a step toward that
sentence.

## Two pillars, both first-class

1. **Library / prep** — the place you live before a set. Collection, folders, playlists,
   tags, search, analysis (beatgrid, BPM, key, waveform, loudness), cue and loop
   editing, grid correction. ([04-LIBRARY](04-LIBRARY.md), [03-ANALYSIS](03-ANALYSIS.md))
2. **Performance** — decks, mixer, sync, key lock, FX, cue bus, controller. Rock solid
   real-time audio. ([02-AUDIO-ENGINE](02-AUDIO-ENGINE.md), [05-UI](05-UI.md))

Neither is "the MVP"; the foundation is designed so both can be built out fully.

## Principles

- **Ours.** DJ-specific code — the engine, analysis, library, UI kit, MIDI layer,
  codecs — is written by us, in Rust. We borrow only *commodity plumbing* where
  hand-rolling would cost weeks and teach us nothing about DJ software
  (see [DECISIONS](DECISIONS.md) for the exact, short list).
- **Stability over features.** A DJ app that glitches once at a gig is worthless. The
  real-time thread never allocates, locks, or does I/O. Every feature that touches the
  audio path is designed with that in mind before it is written.
- **Big.** Text, controls, waveforms, hit targets. When in doubt, larger.
- **Everything you need and nothing you don't.** Fewer, well-spaced controls beat
  feature completeness. Features earn their place on the screen.
- **Foundation first.** This project is deliberately *not* rushing to a minimal
  viable product. Threading model, messaging, data formats and crate boundaries are
  decided up front ([01-ARCHITECTURE](01-ARCHITECTURE.md)) so later features slot in
  instead of forcing rewrites.
- **Alva tests at the machine.** The app is never screenshotted or driven by
  automation; verification is by Alva's hands and ears, plus native unit tests for
  everything that can be tested headlessly (DSP, codecs, analysis, library, mapping).

## Explicit non-goals (for now)

These are not "never"; they are "not in the foundation, and the foundation must not be
distorted to accommodate them early":

- Streaming services (Beatport/Beatsource/SoundCloud/Tidal integration).
- Rekordbox USB export for CDJs/XDJs (the Pioneer PDB database format).
- Importing existing Rekordbox / Serato / Mixxx libraries (library starts fresh).
- DVS / timecode vinyl.
- Lighting (DMX/Pro DJ Link).
- Stems separation.
- Video.
- Windows / macOS builds. Linux only; Wayland first, X11 via winit's fallback.

## Naming

Crates are prefixed `lmx_` (Lantern MiX). The application binary is `lantern-mix`.
User data lives under `~/.local/share/lantern-mix/`, config under
`~/.config/lantern-mix/`.
