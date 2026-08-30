//! Engine: owns everything below, process(out, frames), command intake,
//! snapshot publish.

use crate::command::{EngineCommand, Snapshot};
use crate::deck::Deck;
use lmx_core::TrackAudio;
use lmx_rt::{spsc, triple, AtomicF32, Consumer, Producer, TripleReader, TripleWriter};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub const DECKS: usize = 4;
/// Largest block the engine renders in one go; bigger blocks are chunked.
const MAX_FRAMES: usize = 4096;

/// Test-tone controls, shared with the UI.
#[derive(Default)]
pub struct ToneParams {
    pub on: AtomicBool,
    /// 0..1 linear.
    pub gain: AtomicF32,
}

/// UI-side ends of the engine's queues.
pub struct EngineHandles {
    pub cmds: Producer<EngineCommand>,
    pub garbage: Consumer<Box<TrackAudio>>,
    pub snapshot: TripleReader<Snapshot>,
    pub tone: Arc<ToneParams>,
}

pub struct Engine {
    decks: [Deck; DECKS],
    cmds: Consumer<EngineCommand>,
    garbage: Producer<Box<TrackAudio>>,
    /// A retired track the garbage ring couldn't take yet; retried each block.
    pending_garbage: Option<Box<TrackAudio>>,
    snap: TripleWriter<Snapshot>,
    blocks: u64,
    mix: Vec<f32>,
    tone: Arc<ToneParams>,
    tone_phase: f32,
    tone_gain: f32,
}

impl Engine {
    pub fn new() -> (Engine, EngineHandles) {
        let (cmd_tx, cmd_rx) = spsc(256);
        let (gb_tx, gb_rx) = spsc(16);
        let (snap_w, snap_r) = triple(Snapshot::default());
        let tone = Arc::new(ToneParams::default());
        tone.gain.store(0.75);
        let engine = Engine {
            decks: Default::default(),
            cmds: cmd_rx,
            garbage: gb_tx,
            pending_garbage: None,
            snap: snap_w,
            blocks: 0,
            mix: vec![0.0; MAX_FRAMES * 2],
            tone: tone.clone(),
            tone_phase: 0.0,
            tone_gain: 0.0,
        };
        (engine, EngineHandles { cmds: cmd_tx, garbage: gb_rx, snapshot: snap_r, tone })
    }

    fn retire(&mut self, t: Option<Box<TrackAudio>>) {
        if let Some(t) = t {
            match self.garbage.push(t) {
                Ok(()) => {}
                Err(t) => {
                    // Ring full: keep one; a second one in the same block is
                    // vanishingly unlikely (loads come from one UI thread).
                    if self.pending_garbage.is_none() {
                        self.pending_garbage = Some(t);
                    }
                }
            }
        }
    }

    fn apply_commands(&mut self) {
        if let Some(t) = self.pending_garbage.take() {
            self.retire(Some(t));
        }
        while let Some(c) = self.cmds.pop() {
            match c {
                EngineCommand::Load { deck, audio } => {
                    if deck < DECKS {
                        let old = self.decks[deck].load(audio);
                        self.retire(old);
                    } else {
                        self.retire(Some(audio));
                    }
                }
                EngineCommand::Unload { deck } => {
                    if deck < DECKS {
                        let old = self.decks[deck].unload();
                        self.retire(old);
                    }
                }
                EngineCommand::Play { deck, on } => {
                    if deck < DECKS {
                        self.decks[deck].play(on);
                    }
                }
                EngineCommand::Seek { deck, frame } => {
                    if deck < DECKS {
                        self.decks[deck].seek(frame);
                    }
                }
                EngineCommand::SetTempo { deck, ratio } => {
                    if deck < DECKS {
                        self.decks[deck].set_tempo(ratio);
                    }
                }
            }
        }
    }

    /// Fill `out` (interleaved, `channels` wide) with `frames` frames. Channels
    /// 0/1 carry the master; 2/3 (cue) mirror it for now; the rest stay silent.
    pub fn process(&mut self, out: &mut [f32], channels: usize, frames: usize, rate: u32) {
        self.apply_commands();
        let channels = channels.max(1);
        let mut master_peak = [0.0f32; 2];
        let mut done = 0;
        // Move the scratch buffer out for the loop (a pointer move, no alloc).
        let mut mix_buf = std::mem::take(&mut self.mix);
        while done < frames {
            let n = (frames - done).min(MAX_FRAMES);
            let mix = &mut mix_buf[..n * 2];
            mix.fill(0.0);
            for d in &mut self.decks {
                d.render(mix, n, rate);
            }
            self.add_tone(mix, n, rate);
            for f in 0..n {
                let l = mix[f * 2].clamp(-1.0, 1.0);
                let r = mix[f * 2 + 1].clamp(-1.0, 1.0);
                master_peak[0] = master_peak[0].max(l.abs());
                master_peak[1] = master_peak[1].max(r.abs());
                let base = (done + f) * channels;
                if let Some(s) = out.get_mut(base..base + channels) {
                    s[0] = l;
                    if channels > 1 {
                        s[1] = r;
                    }
                    if channels > 3 {
                        s[2] = l;
                        s[3] = r;
                    }
                }
            }
            done += n;
        }
        self.mix = mix_buf;
        self.blocks += 1;
        let mut snap = Snapshot { master_peak, blocks: self.blocks, ..Default::default() };
        for (i, d) in self.decks.iter().enumerate() {
            snap.decks[i] = d.snap();
        }
        self.snap.write(snap);
    }

    fn add_tone(&mut self, mix: &mut [f32], frames: usize, rate: u32) {
        let target = if self.tone.on.load(Ordering::Relaxed) { self.tone.gain.load().clamp(0.0, 1.0) } else { 0.0 };
        if target == 0.0 && self.tone_gain < 1e-5 {
            self.tone_gain = 0.0;
            return;
        }
        let step = std::f32::consts::TAU * 440.0 / rate.max(1) as f32;
        let k = 1.0 - (-1.0 / (0.005 * rate as f32)).exp();
        for f in 0..frames {
            self.tone_gain += (target - self.tone_gain) * k;
            let s = self.tone_phase.sin() * self.tone_gain * 0.5;
            self.tone_phase += step;
            if self.tone_phase >= std::f32::consts::TAU {
                self.tone_phase -= std::f32::consts::TAU;
            }
            mix[f * 2] += s;
            mix[f * 2 + 1] += s;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp_track(n: usize, rate: u32) -> Box<TrackAudio> {
        let mut frames = Vec::with_capacity(n * 2);
        for i in 0..n {
            let v = i as f32 / n as f32;
            frames.push(v);
            frames.push(-v);
        }
        Box::new(TrackAudio { sample_rate: rate, channels: 2, frames })
    }

    #[test]
    fn plays_sample_exact_at_unity_and_returns_old_track() {
        let (mut e, mut h) = Engine::new();
        h.cmds.push(EngineCommand::Load { deck: 0, audio: ramp_track(1000, 48_000) }).ok().unwrap();
        h.cmds.push(EngineCommand::Play { deck: 0, on: true }).ok().unwrap();
        let mut out = vec![0.0f32; 128 * 2];
        e.process(&mut out, 2, 128, 48_000);
        for f in 0..128 {
            assert!((out[f * 2] - f as f32 / 1000.0).abs() < 1e-6, "frame {f}");
            assert!((out[f * 2 + 1] + f as f32 / 1000.0).abs() < 1e-6);
        }
        let s = h.snapshot.read();
        assert!(s.decks[0].playing && s.decks[0].loaded);
        assert_eq!(s.decks[0].pos, 128.0);
        // replacing the track hands the old one back
        h.cmds.push(EngineCommand::Load { deck: 0, audio: ramp_track(10, 48_000) }).ok().unwrap();
        e.process(&mut out, 2, 128, 48_000);
        let old = h.garbage.pop().expect("old track returned");
        assert_eq!(old.frame_count(), 1000);
        assert!(!h.snapshot.read().decks[0].playing, "load stops the deck");
    }

    #[test]
    fn resamples_by_rate_ratio_and_stops_at_end() {
        let (mut e, mut h) = Engine::new();
        h.cmds.push(EngineCommand::Load { deck: 1, audio: ramp_track(441, 44_100) }).ok().unwrap();
        h.cmds.push(EngineCommand::Play { deck: 1, on: true }).ok().unwrap();
        let mut out = vec![0.0f32; 480 * 4];
        e.process(&mut out, 4, 480, 48_000);
        let s = h.snapshot.read().decks[1];
        // 480 device frames at 44.1/48 = 441 source frames → reaches the end
        assert!(!s.playing);
        assert_eq!(s.pos, 440.0);
        // cue channels mirror master, tone off
        assert_eq!(out[0], out[2]);
        assert!(out[100 * 4].abs() > 0.0);
    }
}
