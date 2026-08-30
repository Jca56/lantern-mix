# 06 — Controllers

`lmx_midi`: talk to the DDJ-GRV6 (and any class-compliant MIDI controller) with as
little between us and the wire as possible. std-only.

## Transport: raw MIDI over `/dev/snd`

Class-compliant USB MIDI devices appear as ALSA rawmidi nodes `/dev/snd/midiC<card>D<dev>`.
Reading and writing them is plain `std::fs::File` I/O — no libasound, no sequencer,
no PipeWire involvement, works identically on Arch and Gentoo.

- **Discovery**: list `/dev/snd/midi*`; read the card name from
  `/proc/asound/card<N>/id` and the device name from `/proc/asound/card<N>/midi<D>`.
  Match profiles by name ("DDJ-GRV6").
- **Hotplug**: a scanner thread re-lists every 2 s (no inotify; polling a directory of
  a dozen entries is free). Plug the GRV6 in after launch and it just appears;
  unplug and its reader thread ends cleanly.
- **Reading**: one thread per device, blocking `read()` into a 256-byte buffer,
  timestamped on receipt (`Instant`), parsed, then fanned out:
  - to the **engine** SPSC for time-critical controls (jog, faders, EQ, play/cue/hot
    cues, loops, sync) — the mapping resolves them straight to `EngineCommand`s;
  - to the **UI** SPSC for everything else (browse, load, screen toggles) and for
    display (the UI mirrors controller gestures).
- **Writing**: LED/output messages are written to the same fd from the UI thread
  (rate-limited, deduplicated per (status, data1) so we never spam identical states).
- **Permissions**: `/dev/snd/*` is group `audio` on both distros; Alva is in it. If a
  device isn't readable, the UI says so instead of failing silently.

If a device ever needs the ALSA sequencer (virtual ports, routing to other apps) that
is a future `SeqTransport` behind the same `Transport` trait; not planned.

## Parser

`lmx_midi::parse`: byte stream → `MidiMsg`, handling running status, real-time bytes
interleaved anywhere, SysEx accumulation (bounded), and **14-bit CC pairing**:
Pioneer sends MSB on CC `n` followed by LSB on CC `n+32`; the parser emits a
`Cc14 { cc: n, value: u14 }` when the LSB arrives (and a plain `Cc` if the pair never
completes within the same read).

Messages carry `(device_id, timestamp, channel, kind)`.

## Mapping engine

A mapping is data: a list of rules from `(channel, kind, number)` to an **action**,
with options (14-bit, toggle, shift layer, deck target, value curve, center value for
relative encoders). Format is our own plain-text `.lmxmap`, easy to write by hand:

```
controller "DDJ-GRV6"
deck 1 ch 1      # note/cc channel per deck
deck 2 ch 2
deck 3 ch 3
deck 4 ch 4
mixer  ch 7
fx     ch 5

[deck]                        # rules under [deck] apply to every deck's channel
note 0x0B  -> play_toggle
note 0x0C  -> cue              { on press, on release }
note 0x3F  -> shift            { layer }
note 0x36  -> jog_touch
cc   0x22  -> jog_vinyl        { relative center 0x40 }
cc   0x21  -> jog_bend         { relative center 0x40 }
cc14 0x00  -> tempo            { range -1..1, takeover }
cc14 0x13  -> fader            { takeover }
cc14 0x04  -> trim
cc14 0x07  -> eq_high
cc14 0x0B  -> eq_mid
cc14 0x0F  -> eq_low
note 0x54  -> pfl              { toggle, led }
note 0x58  -> sync             { led }
note 0x10  -> loop_in
note 0x11  -> loop_out
note 0x4D  -> reloop_toggle

[mixer]
cc   0x40  -> browse           { relative center 0x40 }
note 0x41  -> browse_press
note 0x46  -> load deck 1
note 0x47  -> load deck 2
note 0x48  -> load deck 3
note 0x49  -> load deck 4
cc14 0x1F  -> crossfader       { takeover }
cc14 0x0C  -> headphone_mix
cc14 0x0D  -> headphone_level

[fx]
note 0x47  -> fx_on            { toggle, led }
cc14 0x02  -> fx_level
```

- **Actions** are an enum in `lmx_midi::action`, split into `EngineAction` (becomes an
  `EngineCommand` immediately, with the deck resolved) and `UiAction`.
- **Shift layer**: rules may be `shift+note 0x0B -> reverse_roll`; the layer state is
  per device.
- **Soft takeover** (`takeover`): after (re)connect, a physical fader whose position
  differs from the app's value does nothing until it crosses the app's value — no
  jumps mid-set. Applies to faders/knobs that aren't motorized (all of them).
- **Deck toggle**: GRV6 has physical deck 1/3 and 2/4 buttons; these arrive as MIDI on
  their own channel numbers already (the hardware switches the *channel* the left side
  sends on) — to be **confirmed with the sniffer**. If instead the hardware sends a
  toggle message and keeps the channel, the mapping engine's `deck_select` action
  re-targets subsequent rules.
- **LED feedback**: rules with `{ led }` register an output: the mapping layer watches
  the `Snapshot`/UI state each frame and writes `note on/off` (or a value) to the
  device when it changes. Hot cue pad colors are values on the pad's note (Pioneer
  convention) — exact palette to sniff.
- **MIDI learn**: the settings screen has a "learn" mode: touch a control, pick an
  action, done; it writes the rule into the user's mapping file.

Built-in profiles ship as `.lmxmap` files embedded in the binary; user copies in
`~/.config/lantern-mix/mappings/` override by controller name.

## DDJ-GRV6 protocol (official)

Source: AlphaTheta's *DDJ-GRV6 MIDI Message List E1* (`docs/reference/`), cross-checked
against the community Mixxx mapping. Plain MIDI, no HID, no SysEx. `n` = deck 0–3,
`p` = pad channel, `hh` = data 2. Every button sends note-on `0x7F` on press and
`0x00` on release; shifted variants are separate note numbers.

### Channel map

| Channel | Status | What |
|---|---|---|
| 1–4 | `0x90+n` / `0xB0+n` | Deck n: transport, jog, loop, tempo, mixer strip (trim/EQ/fader/cue), Groove Circuit, pad-mode buttons |
| 5 | `0x94` / `0xB4` | Beat FX section |
| 6 | — | unused |
| 7 | `0x96` / `0xB6` | Browse + global: encoder, LOAD, crossfader, master/booth/headphones/mic, Sound Color FX |
| 8, 10, 12, 14 | `0x97`,`0x99`,`0x9B`,`0x9D` | Performance pads, deck 1–4, **without** shift |
| 9, 11, 13, 15 | `0x98`,`0x9A`,`0x9C`,`0x9E` | Performance pads, deck 1–4, **with** shift |
| 16 | `0x9F` / `0xBF` | MIDI-OUT only: LOAD-button illumination (notes `0x00–0x03`), jog ring illumination (CC `0x00–0x03`, value `0x00–0x48`) |

**Deck toggles are a channel switch.** Pressing DECK 3 sends note `0x72` on channel 3
and the left side's controls then transmit on channel 3 (same for DECK 4 → channel 4).
The active-deck LEDs are driven by `9n 3C hh` (`7F` lit / `00` off).

### Relative encoders (all "difference since last message")

| Control | Message | Encoding |
|---|---|---|
| Browse encoder | `B6 40` (shift `B6 64`) | CW `0x01…0x1E`, CCW `0x7F…0x62` → `delta = v < 64 ? v : v − 128` |
| Jog platter, vinyl ON | `Bn 22` | CW increases from `0x41`, CCW decreases from `0x3F` → `delta = v − 64` |
| Jog platter, vinyl OFF | `Bn 23` | same |
| Jog platter + shift | `Bn 29` | same (search) |
| Jog wheel side | `Bn 21` (shift `Bn 26`) | same (pitch bend) |
| Jog touch | `9n 36` (shift `9n 67`) | note on/off |

Ticks per revolution are **not** in the document — measured with the sniffer.

### 14-bit continuous controls (MSB on `cc`, LSB on `cc + 0x20`)

| Control | Status | MSB / LSB |
|---|---|---|
| Tempo slider | `Bn` | `00 / 20` |
| Trim | `Bn` | `04 / 24` |
| EQ Hi / Mid / Low | `Bn` | `07/27`, `0B/2B`, `0F/2F` |
| Channel fader | `Bn` | `13 / 33` |
| Groove Circuit GAIN | `Bn` | `12 / 32` |
| Crossfader | `B6` | `1F / 3F` |
| Master level / Booth level | `B6` | `08/28`, `09/29` |
| Headphones level / mix | `B6` | `0D/2D`, `0C/2C` |
| Mic level | `B6` | `05 / 25` |
| Sound Color FX knob CH 1 | `B6` | `17 / 37` (CH 2–4 follow as `18/38`, `19/39`, `1A/3A` — verify) |
| Beat FX level/depth | `B4` | `02 / 22` |

Fader-start edges are also sent as notes: crossfader `96 50–55`, channel fader
`9n 5D` (bottom→up), `9n 52` (→bottom), `9n 66` (shift). We ignore them unless
fader-start is implemented.

### Deck buttons (`9n`, LED feedback on the same note where marked ●)

| Button | Note | + shift |
|---|---|---|
| PLAY/PAUSE ● | `0B` | `47` |
| CUE ● | `0C` | `48` |
| SHIFT | `3F` | — |
| SLIP ● | `40` | `17` |
| QUANTIZE ● | `35` | `68` |
| LOOP IN / 4 BEAT ● | `10` (long press `14`) | `4C` |
| LOOP OUT ● | `11` | `77` |
| RELOOP / EXIT | `4D` | `50` |
| CUE/LOOP CALL ◀ / ▶ | `51` / `53` | `61` / `62` |
| MEMORY | `3D` | `3E` |
| DECK select (this deck) ● | `72` | `73` |
| MASTER TEMPO (keylock) ● | `1A` | `60` |
| BEAT SYNC ● | `58` | `5C` |
| KEY SYNC ● | `65` | `64` |
| CH CUE (PFL) ● | `54` | `39` |
| Pad mode: HOT CUE ● | `1B` | `69` |
| Pad mode: STEMS ● | `1E` | `6B` |
| Pad mode: BEAT JUMP ● | `20` | `6D` |
| Pad mode: SAMPLER ● | `22` (long press `79`) | `6F` |
| Groove Circuit DRUM SWAP 1–4 ● | `00–03` | `2C–2F` |
| Groove Circuit DRUM ROLL 1–4 ● | `04–07` | `30–33` |
| Groove Circuit CAPTURE ● | `08` | `2B` |
| Groove Circuit DRUM RELEASE (tilt fwd / back) | `24` / `25` (verify) | `26` / `27` |

Channel level meters are driven by the software: `Bn 02 hh` — `0x77–0x7F` red + two
orange + two green, `0x65–0x76` two orange + two green, `0x57–0x64` one orange + two
green, `0x41–0x56` two green, lower = one / none.

### Performance pads (`9p`, 8 modes × 8 pads, LED feedback on the same note)

`note = mode_base + pad (0–7)`: Hot Cue `0x00`, Stems `0x10`, Beat Jump `0x20`,
Sampler `0x30`, Keyboard `0x40`, Pad FX `0x50`, Beat Loop `0x60`, Key Shift `0x70`.
The shifted pad channel carries the same numbers. Pad LED **color/brightness values
for `hh` are not documented** — probed with `lmx_midisniff --send`.

### Browse / global (`96` / `B6`)

Encoder press `41` (shift `42`); tilt fwd `38`/`39`, back `3A`/`3B`, left `3C`/`3D`,
right `2E`/`3F`; DISCOVER `35`/`68`; BACK `65`/`66`; VIEW `7A`/`67`; PREVIEW `36`/`37`;
LOAD 1–4 `46–49` (shift `58, 59, 60, 61`); MASTER CUE `63`/`62`; Sound Color FX ON/OFF ●
`00`/`08`; mono/stereo switch `6D`.

### Beat FX (`94` / `B4`)

BEAT ◀ `4A`/`66`, ▶ `4B`/`6B`; FX ON/OFF ● `47`/`43`; FX CH SELECT: CH1 `10`, CH2 `11`,
CH3 `12`, CH4 `13`, MST `14`, SP `16` (shift `18, 19, 1A, 1B, 1C, 1E`). FX SELECT rotary
sends a note per effect: `20` DELAY, `21` ECHO, `22` LOW CUT ECHO, `23` SPIRAL, `24`
HELIX, `25` REVERB, `26` FLANGER, `27` PHASER, `28` FILTER, `29` TRANS, `2A` PITCH, `2B`
ROLL, `2C` MOBIUS SAW, `2D` MOBIUS TRI. Beat-length indicator OUT `B4 64 hh`: `03`=1/4,
`04`=1/2, `21`=3/4, `05`=1, `06`=2, `07`=4, `08`=8, `09`=16, `0A`=32.

### Still to measure with `lmx_midisniff` (Phase 0)

- Jog **ticks per revolution** and the max tick rate.
- Pad LED `hh` color palette; whether LED writes need a startup "wake" message.
- Whether the controller dumps fader/knob positions on connect.
- Sound Color FX CH 2–4 CC numbers and DRUM RELEASE tilt-back note (pattern-inferred).
- The **USB audio topology**: output channel count (4 → software mix, 8 → hardware
  mix), inputs, PipeWire profile names ([02-AUDIO-ENGINE](02-AUDIO-ENGINE.md)).

## `tools/lmx_midisniff`

`lmx_midisniff [device]` opens a rawmidi node (default: first non-"Midi Through"
device), prints every message with a millisecond timestamp, decoded channel/kind/
number/value, and pairs 14-bit CCs on the fly. `--raw` prints bytes. `--send 90 0B 7F`
writes a message (LED probing). It is the Phase 0 deliverable for this crate and the
tool we use to fill the table above.

## Testing

Parser: byte-exact fixtures for running status, interleaved real-time bytes, split
14-bit pairs across reads, SysEx. Mapping: parse the GRV6 file, feed synthetic
messages, assert the resulting `EngineCommand`s/`UiAction`s including shift layers,
takeover crossing, and relative-encoder sign. No hardware required.
