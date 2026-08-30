//! Queues between MIDI, engine, UI; garbage return; snapshot reads.
//!
//! `Audio` owns the PipeWire host with the engine as its render source, plus
//! the UI-side handles: commands in, snapshot out, garbage back, tone controls.

use crate::workers::UserEvent;
use lmx_audio::{AudioConfig, AudioHost, AudioRender, AudioState};
use lmx_core::TrackAudio;
use lmx_engine::{Engine, EngineCommand, EngineHandles, Snapshot};
use std::sync::atomic::Ordering;
use winit::event_loop::EventLoopProxy;

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
    seen_xruns: u64,
    started: std::time::Instant,
}

impl Audio {
    /// Start PipeWire with the engine as the render source. Never fails the
    /// app: an error is kept for the UI to show.
    pub fn start(proxy: EventLoopProxy<UserEvent>) -> Self {
        // Engine state changes → eventfd → watcher thread → event loop wake.
        let notify: Option<Box<dyn Fn() + Send>> = match lmx_rt::wake_pair() {
            Some((n, w)) => {
                std::thread::Builder::new()
                    .name("lmx-wake".into())
                    .spawn(move || loop {
                        w.wait();
                        if proxy.send_event(UserEvent::Wake).is_err() {
                            break;
                        }
                    })
                    .expect("spawn wake thread");
                Some(Box::new(move || n.notify()))
            }
            None => None,
        };
        let (engine, handles) = Engine::new(notify);
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
        Self { host, error, handles, snapshot: Snapshot::default(), pending: Vec::new(), seen_xruns: 0, started: std::time::Instant::now() }
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
        let x = self.xruns();
        if x != self.seen_xruns {
            eprintln!("lantern-mix: late audio callback #{x} at t+{:.1}s", self.started.elapsed().as_secs_f32());
            self.seen_xruns = x;
        }
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
