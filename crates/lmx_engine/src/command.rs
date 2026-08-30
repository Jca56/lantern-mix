//! EngineCommand / EngineEvent / Snapshot definitions.

use lmx_core::TrackAudio;

/// UI/MIDI → engine. Applied at the start of the next block.
pub enum EngineCommand {
    /// Swap a track into a deck (stops it, position 0). The old one comes back
    /// through the garbage ring.
    Load { deck: usize, audio: Box<TrackAudio> },
    Unload { deck: usize },
    Play { deck: usize, on: bool },
    /// Seek to a source-frame position.
    Seek { deck: usize, frame: f64 },
    /// Playback speed multiplier (1.0 = original tempo).
    SetTempo { deck: usize, ratio: f64 },
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DeckSnap {
    pub loaded: bool,
    pub playing: bool,
    /// Play head, source frames.
    pub pos: f64,
    pub frames: u64,
    pub sample_rate: u32,
    /// Block peak of this deck's output, linear.
    pub peak: f32,
}

/// Engine → UI, published once per block.
#[derive(Clone, Copy, Debug, Default)]
pub struct Snapshot {
    pub decks: [DeckSnap; 4],
    /// Master block peak, linear, L/R.
    pub master_peak: [f32; 2],
    pub blocks: u64,
}
