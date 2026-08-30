//! Queues between MIDI, engine, UI; garbage return; snapshot reads.
//!
//! Phase 0: the audio host + test tone, and the level snapshot the UI reads.

use lmx_audio::{AudioConfig, AudioHost, AudioState, Levels, TestTone, ToneControl};
use lmx_rt::TripleReader;
use std::sync::atomic::Ordering;

pub struct Audio {
    host: Option<AudioHost>,
    pub error: Option<String>,
    pub tone: ToneControl,
    levels: TripleReader<Levels>,
    last: Levels,
}

impl Audio {
    /// Start PipeWire with the test tone as the render source. Never fails the
    /// app: an error is kept for the UI to show.
    pub fn start() -> Self {
        let (tone, ctl, levels) = TestTone::new(440.0, 0.75);
        let mut config = AudioConfig::default();
        if let Ok(t) = std::env::var("LMX_AUDIO_TARGET") {
            config.target = Some(t);
        }
        if let Some(ch) = std::env::var("LMX_AUDIO_CHANNELS").ok().and_then(|v| v.parse().ok()) {
            config.channels = ch;
        }
        let (host, error) = match AudioHost::start(config, Box::new(tone)) {
            Ok(h) => (Some(h), None),
            Err(e) => {
                eprintln!("lantern-mix: audio: {e}");
                (None, Some(e))
            }
        };
        Self { host, error, tone: ctl, levels, last: Levels::default() }
    }

    /// Latest per-channel peak levels published by the RT thread.
    pub fn levels(&mut self) -> Levels {
        self.last = self.levels.read();
        self.last
    }

    pub fn state(&self) -> AudioState {
        self.host.as_ref().map(|h| h.status().state()).unwrap_or(AudioState::Error)
    }

    /// Negotiated sample rate (0 until known).
    pub fn rate(&self) -> u32 {
        self.host.as_ref().map(|h| h.status().rate.load(Ordering::Relaxed)).unwrap_or(0)
    }

    /// Frames per block as delivered.
    pub fn block(&self) -> u32 {
        self.host.as_ref().map(|h| h.status().block.load(Ordering::Relaxed)).unwrap_or(0)
    }

    pub fn xruns(&self) -> u64 {
        self.host.as_ref().map(|h| h.status().late.load(Ordering::Relaxed) + h.status().starved.load(Ordering::Relaxed)).unwrap_or(0)
    }
}
