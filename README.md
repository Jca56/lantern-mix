# Lantern Mix

DJ software for Linux, in Rust, ours. Built for a DDJ-GRV6, an Arch laptop, and a DJ
who needs big text.

This repository is in its **design phase**: read `docs/00-VISION.md` first, then
`docs/01-ARCHITECTURE.md`. The cargo workspace is a skeleton that mirrors the
architecture; crates carry their responsibilities as doc comments and no logic yet.

```
docs/            design docs — one per subsystem, plus DECISIONS.md
crates/lmx_*     library crates (see docs/01-ARCHITECTURE.md for the map)
app/             the lantern-mix binary
tools/           lmx_midisniff, lmx_analyze
```

Build requirements: Rust stable, `../Lantern-DE` checked out as a sibling (for
`lntrn-text`), PipeWire headers + `pkg-config` + `clang` (for `pipewire-sys`).
