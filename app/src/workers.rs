//! Job queue + worker pool: load, analyze, scan; results back to the UI thread.
//!
//! Phase 1 slice: one thread per load (decode + waveform summary), results over
//! a channel, and a winit proxy poke so the event loop wakes up to collect them.

use lmx_analysis::WaveformSummary;
use lmx_codec::{Metadata, Probe};
use lmx_core::TrackAudio;
use lmx_library::{Track, TrackId};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use winit::event_loop::EventLoopProxy;

/// Sent through the winit event loop purely to wake it up.
#[derive(Debug, Clone, Copy)]
pub enum UserEvent {
    Wake,
}

pub struct Loaded {
    pub deck: usize,
    pub path: PathBuf,
    pub result: Result<(TrackAudio, Metadata, Probe, WaveformSummary), String>,
}

pub enum WorkerMsg {
    Loaded(Loaded),
    /// A library scan finished: every readable audio file under the roots.
    Scanned(Vec<Track>),
}

/// Progress 0..1000 per deck, written by workers, read by the UI.
pub type Progress = Arc<[AtomicU32; 4]>;

pub struct Loader {
    tx: Sender<WorkerMsg>,
    rx: Receiver<WorkerMsg>,
    proxy: Option<EventLoopProxy<UserEvent>>,
    pub progress: Progress,
}

impl Loader {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        Self { tx, rx, proxy: None, progress: Arc::new([AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0)]) }
    }

    pub fn set_proxy(&mut self, proxy: EventLoopProxy<UserEvent>) {
        self.proxy = Some(proxy);
    }

    /// Decode + summarize `path` for `deck` on a new thread.
    pub fn load(&self, deck: usize, path: PathBuf) {
        let tx = self.tx.clone();
        let proxy = self.proxy.clone();
        let progress = self.progress.clone();
        let deck = deck.min(3);
        progress[deck].store(1, Ordering::Relaxed);
        std::thread::Builder::new()
            .name(format!("lmx-load-{deck}"))
            .spawn(move || {
                let wake = |p: u32| {
                    progress[deck].store(p, Ordering::Relaxed);
                    if let Some(px) = &proxy {
                        let _ = px.send_event(UserEvent::Wake);
                    }
                };
                let result = (|| {
                    let probe = lmx_codec::probe(&path).map_err(|e| e.to_string())?;
                    let mut last = 0;
                    let (audio, meta) = lmx_codec::decode_all(&path, |f| {
                        let p = (f * 600.0) as u32;
                        if p >= last + 50 {
                            last = p;
                            wake(p);
                        }
                    })
                    .map_err(|e| e.to_string())?;
                    let mut last = 600;
                    let summary = lmx_analysis::waveform::compute(&audio, |f| {
                        let p = 600 + (f * 400.0) as u32;
                        if p >= last + 50 {
                            last = p;
                            wake(p);
                        }
                    });
                    Ok((audio, meta, probe, summary))
                })();
                progress[deck].store(0, Ordering::Relaxed);
                let _ = tx.send(WorkerMsg::Loaded(Loaded { deck, path, result }));
                if let Some(px) = &proxy {
                    let _ = px.send_event(UserEvent::Wake);
                }
            })
            .expect("spawn loader");
    }

    /// Progress of a deck's load in 0..1, or None if idle.
    pub fn progress(&self, deck: usize) -> Option<f32> {
        let p = self.progress[deck.min(3)].load(Ordering::Relaxed);
        if p == 0 { None } else { Some(p as f32 / 1000.0) }
    }

    pub fn try_recv(&self) -> Option<WorkerMsg> {
        self.rx.try_recv().ok()
    }

    /// Walk `roots` and probe every audio file (headers only) on a thread.
    pub fn scan(&self, roots: Vec<PathBuf>) {
        let tx = self.tx.clone();
        let proxy = self.proxy.clone();
        std::thread::Builder::new()
            .name("lmx-scan".into())
            .spawn(move || {
                let mut tracks = Vec::new();
                for root in &roots {
                    for path in lmx_library::walk_audio_files(root) {
                        let Ok(p) = lmx_codec::probe(&path) else { continue };
                        let m = &p.metadata;
                        tracks.push(Track {
                            id: TrackId::from_path(&path),
                            title: m.title.clone().unwrap_or_default(),
                            artist: m.artist.clone().unwrap_or_default(),
                            album: m.album.clone().unwrap_or_default(),
                            bpm: m.bpm_tag,
                            key: m.key_tag.clone(),
                            sample_rate: p.sample_rate,
                            duration_secs: p.duration_secs(),
                            path,
                        });
                    }
                }
                eprintln!("lantern-mix: scanned {} tracks from {} root(s)", tracks.len(), roots.len());
                let _ = tx.send(WorkerMsg::Scanned(tracks));
                if let Some(px) = &proxy {
                    let _ = px.send_event(UserEvent::Wake);
                }
            })
            .expect("spawn scan");
    }
}
