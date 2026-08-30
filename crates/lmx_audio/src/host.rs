//! AudioHost trait: start/stop, device list, rate, block size, xrun count.

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};

/// Something that fills output buffers from the real-time thread.
///
/// `out` is interleaved f32, `frames * channels` long, already zeroed. Must obey
/// the RT contract: no allocation, locks, blocking or I/O.
pub trait AudioRender: Send + 'static {
    fn render(&mut self, out: &mut [f32], channels: usize, frames: usize, rate: u32);
}

#[derive(Clone, Debug)]
pub struct AudioConfig {
    /// Output channels: 2 (master only) or 4 (master + cue), 8 for hardware-mix.
    pub channels: u32,
    /// Sample rate we ask for. PipeWire resamples if its graph runs elsewhere.
    pub rate: u32,
    /// Block size (frames) we ask PipeWire for.
    pub quantum: u32,
    /// PipeWire node name/serial to connect to; `None` = default sink.
    pub target: Option<String>,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self { channels: 2, rate: 48_000, quantum: 128, target: None }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AudioState {
    Unconnected = 0,
    Connecting = 1,
    Paused = 2,
    Streaming = 3,
    Error = 4,
}

/// Live status shared between the RT thread and the UI. All atomics; read any time.
#[derive(Debug, Default)]
pub struct AudioStatus {
    state: AtomicU8,
    /// Negotiated sample rate (0 until the format is known).
    pub rate: AtomicU32,
    pub channels: AtomicU32,
    /// Frames in the most recent block.
    pub block: AtomicU32,
    /// Total process callbacks.
    pub callbacks: AtomicU64,
    /// Callbacks that arrived later than 1.5× the previous block's duration —
    /// our xrun proxy (PipeWire doesn't hand clients an xrun counter).
    pub late: AtomicU64,
    /// Blocks where PipeWire gave us no buffer to fill.
    pub starved: AtomicU64,
}

impl AudioStatus {
    pub fn state(&self) -> AudioState {
        match self.state.load(Ordering::Relaxed) {
            1 => AudioState::Connecting,
            2 => AudioState::Paused,
            3 => AudioState::Streaming,
            4 => AudioState::Error,
            _ => AudioState::Unconnected,
        }
    }
    pub fn set_state(&self, s: AudioState) {
        self.state.store(s as u8, Ordering::Relaxed);
    }
}
