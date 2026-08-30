//! Deck: position/rate, playback via resample or stretch, cues, loops, slip,
//! state machine.

use crate::command::DeckSnap;
use lmx_core::TrackAudio;
use lmx_dsp::hermite;

pub struct Deck {
    audio: Option<Box<TrackAudio>>,
    pos: f64,
    playing: bool,
    tempo: f64,
    peak: f32,
}

impl Default for Deck {
    fn default() -> Self {
        Self { audio: None, pos: 0.0, playing: false, tempo: 1.0, peak: 0.0 }
    }
}

impl Deck {
    /// Install a track; returns the previous one for disposal off-thread.
    pub fn load(&mut self, audio: Box<TrackAudio>) -> Option<Box<TrackAudio>> {
        let old = self.audio.replace(audio);
        self.pos = 0.0;
        self.playing = false;
        self.peak = 0.0;
        old
    }

    pub fn unload(&mut self) -> Option<Box<TrackAudio>> {
        self.playing = false;
        self.pos = 0.0;
        self.audio.take()
    }

    pub fn play(&mut self, on: bool) {
        self.playing = on && self.audio.is_some();
    }

    pub fn seek(&mut self, frame: f64) {
        if let Some(a) = &self.audio {
            self.pos = frame.clamp(0.0, (a.frame_count().saturating_sub(1)) as f64);
        }
    }

    pub fn set_tempo(&mut self, ratio: f64) {
        self.tempo = ratio.clamp(0.0, 4.0);
    }

    /// Add this deck's next `frames` output frames into `out` (stereo
    /// interleaved, `2 * frames` long) at the device rate.
    pub fn render(&mut self, out: &mut [f32], frames: usize, device_rate: u32) {
        self.peak = 0.0;
        let Some(a) = &self.audio else { return };
        if !self.playing {
            return;
        }
        let n = a.frame_count();
        if n < 2 {
            self.playing = false;
            return;
        }
        let ratio = a.sample_rate as f64 / device_rate.max(1) as f64 * self.tempo;
        let src = &a.frames;
        let last = (n - 1) as f64;
        let mut peak = 0.0f32;
        for f in 0..frames {
            if self.pos >= last {
                self.pos = last;
                self.playing = false;
                break;
            }
            let i = self.pos as usize;
            let t = (self.pos - i as f64) as f32;
            let i0 = i.saturating_sub(1);
            let i2 = (i + 1).min(n - 1);
            let i3 = (i + 2).min(n - 1);
            let l = hermite(src[i0 * 2], src[i * 2], src[i2 * 2], src[i3 * 2], t);
            let r = hermite(src[i0 * 2 + 1], src[i * 2 + 1], src[i2 * 2 + 1], src[i3 * 2 + 1], t);
            out[f * 2] += l;
            out[f * 2 + 1] += r;
            peak = peak.max(l.abs()).max(r.abs());
            self.pos += ratio;
        }
        self.peak = peak;
    }

    pub fn snap(&self) -> DeckSnap {
        DeckSnap {
            loaded: self.audio.is_some(),
            playing: self.playing,
            pos: self.pos,
            frames: self.audio.as_ref().map(|a| a.frame_count() as u64).unwrap_or(0),
            sample_rate: self.audio.as_ref().map(|a| a.sample_rate).unwrap_or(0),
            peak: self.peak,
        }
    }
}
