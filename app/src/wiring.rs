//! Queues between MIDI, engine, UI; garbage return; snapshot reads.
//!
//! `Audio` owns the PipeWire host with the engine as its render source, plus
//! the UI-side handles: commands in, snapshot out, garbage back, tone controls.

use lmx_audio::{AudioConfig, AudioHost, AudioRender, AudioState};
use lmx_core::TrackAudio;
use lmx_engine::{Engine, EngineCommand, EngineHandles, Snapshot};
use std::sync::atomic::Ordering;

struct EngineRender(Engine);

impl AudioRender for EngineRender {
    fn render(&mut self, out: &mut [f32], channels: usize, frames: usize, rate: u32) {
        self.0.process(out, channels, frames, rate);
    }
}

pub struct Audio {
    host: Option<AudioHost>,
    pub error: Option<String>,
    handles: EngineHandles,
    snapshot: Snapshot,
    /// Load commands that didn't fit the ring yet.
    pending: Vec<EngineCommand>,
}

impl Audio {
    /// Start PipeWire with the engine as the render source. Never fails the
    /// app: an error is kept for the UI to show.
    pub fn start() -> Self {
        let (engine, handles) = Engine::new();
        let mut config = AudioConfig::default();
        if let Ok(t) = std::env::var("LMX_AUDIO_TARGET") {
            config.target = Some(t);
        }
        if let Some(ch) = std::env::var("LMX_AUDIO_CHANNELS").ok().and_then(|v| v.parse().ok()) {
            config.channels = ch;
        }
        let (host, error) = match AudioHost::start(config, Box::new(EngineRender(engine))) {
            Ok(h) => (Some(h), None),
            Err(e) => {
                eprintln!("lantern-mix: audio: {e}");
                (None, Some(e))
            }
        };
        Self { host, error, handles, snapshot: Snapshot::default(), pending: Vec::new() }
    }

    /// Call once per frame: drains retired tracks (dropping them here, off the
    /// RT thread), retries queued commands, refreshes the snapshot.
    pub fn poll(&mut self) -> Snapshot {
        while let Some(t) = self.handles.garbage.pop() {
            drop::<Box<TrackAudio>>(t);
        }
        let pending = std::mem::take(&mut self.pending);
        for c in pending {
            self.send(c);
        }
        self.snapshot = self.handles.snapshot.read();
        self.snapshot
    }

    pub fn snapshot(&self) -> Snapshot {
        self.snapshot
    }

    pub fn send(&mut self, cmd: EngineCommand) {
        if let Err(c) = self.handles.cmds.push(cmd) {
            self.pending.push(c);
        }
    }

    pub fn load(&mut self, deck: usize, audio: TrackAudio) {
        self.send(EngineCommand::Load { deck, audio: Box::new(audio) });
    }

    pub fn play(&mut self, deck: usize, on: bool) {
        self.send(EngineCommand::Play { deck, on });
    }

    pub fn set_tone(&self, on: bool, gain: f32) {
        self.handles.tone.on.store(on, Ordering::Relaxed);
        self.handles.tone.gain.store(gain);
    }

    pub fn state(&self) -> AudioState {
        self.host.as_ref().map(|h| h.status().state()).unwrap_or(AudioState::Error)
    }

    pub fn rate(&self) -> u32 {
        self.host.as_ref().map(|h| h.status().rate.load(Ordering::Relaxed)).unwrap_or(0)
    }

    pub fn block(&self) -> u32 {
        self.host.as_ref().map(|h| h.status().block.load(Ordering::Relaxed)).unwrap_or(0)
    }

    pub fn xruns(&self) -> u64 {
        self.host.as_ref().map(|h| h.status().late.load(Ordering::Relaxed) + h.status().starved.load(Ordering::Relaxed)).unwrap_or(0)
    }
}
