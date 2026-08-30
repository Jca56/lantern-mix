//! PipeWire implementation: thread loop, N-channel f32 stream, RT callback,
//! FTZ/DAZ setup, late-callback detection.
//!
//! Objects are created on the calling (UI) thread under the thread-loop lock and
//! stay there; PipeWire runs `process` on its real-time data thread.

use crate::host::{AudioConfig, AudioRender, AudioState, AudioStatus};
use pipewire as pw;
use pw::properties::properties;
use pw::spa;
use spa::param::audio::{AudioFormat, AudioInfoRaw, MAX_CHANNELS};
use spa::pod::Pod;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

struct RtData {
    render: Box<dyn AudioRender>,
    status: Arc<AudioStatus>,
    channels: usize,
    rate: u32,
    last: Option<Instant>,
    /// Frames in the previous block: the gap to this callback is *its* duration.
    last_frames: usize,
    /// Scratch f32 block; PipeWire's buffer is bytes, we render here then copy.
    scratch: Vec<f32>,
    ftz_done: bool,
}

pub struct AudioHost {
    thread_loop: pw::thread_loop::ThreadLoopRc,
    _context: pw::context::ContextRc,
    _core: pw::core::CoreRc,
    stream: pw::stream::StreamRc,
    _listener: pw::stream::StreamListener<RtData>,
    status: Arc<AudioStatus>,
    config: AudioConfig,
}

impl AudioHost {
    /// Connect to PipeWire and start streaming. Returns immediately; watch
    /// `status()` for the negotiated rate and state.
    pub fn start(config: AudioConfig, render: Box<dyn AudioRender>) -> Result<Self, String> {
        pw::init();
        let status = Arc::new(AudioStatus::default());
        status.set_state(AudioState::Connecting);

        // SAFETY: the loop is used only through its lock from this thread and
        // through PipeWire's own callbacks; it is stopped before being dropped.
        let thread_loop = unsafe { pw::thread_loop::ThreadLoopRc::new(Some("lmx-audio"), None) }.map_err(|e| e.to_string())?;
        let lock = thread_loop.lock();
        let context = pw::context::ContextRc::new(&thread_loop, None).map_err(|e| e.to_string())?;
        let core = context.connect_rc(None).map_err(|e| e.to_string())?;

        let latency = format!("{}/{}", config.quantum, config.rate);
        let mut props = properties! {
            *pw::keys::MEDIA_TYPE => "Audio",
            *pw::keys::MEDIA_CATEGORY => "Playback",
            *pw::keys::MEDIA_ROLE => "Music",
            *pw::keys::NODE_NAME => "lantern-mix",
            *pw::keys::NODE_LATENCY => latency.as_str(),
            *pw::keys::APP_NAME => "Lantern Mix",
        };
        if let Some(t) = &config.target {
            props.insert("target.object", t.as_str());
        }
        let stream = pw::stream::StreamRc::new(core.clone(), "Lantern Mix", props).map_err(|e| e.to_string())?;

        let data = RtData {
            render,
            status: status.clone(),
            channels: config.channels as usize,
            rate: config.rate,
            last: None,
            last_frames: 0,
            scratch: vec![0.0; 8192 * config.channels as usize],
            ftz_done: false,
        };
        let listener = stream
            .add_local_listener_with_user_data(data)
            .state_changed(|_, d, _old, new| {
                use pw::stream::StreamState::*;
                d.status.set_state(match new {
                    Unconnected => AudioState::Unconnected,
                    Connecting => AudioState::Connecting,
                    Paused => AudioState::Paused,
                    Streaming => AudioState::Streaming,
                    Error(_) => AudioState::Error,
                });
                if let Error(e) = new {
                    eprintln!("lmx_audio: stream error: {e}");
                }
            })
            .param_changed(|_, d, id, param| {
                let Some(param) = param else { return };
                if id != spa::param::ParamType::Format.as_raw() {
                    return;
                }
                let mut info = AudioInfoRaw::new();
                if info.parse(param).is_ok() {
                    d.rate = info.rate();
                    d.channels = info.channels() as usize;
                    d.status.rate.store(info.rate(), Ordering::Relaxed);
                    d.status.channels.store(info.channels(), Ordering::Relaxed);
                    eprintln!("lmx_audio: format {} Hz × {} ch", info.rate(), info.channels());
                }
            })
            .process(|stream, d| process(stream, d))
            .register()
            .map_err(|e| e.to_string())?;

        let pod_bytes = format_pod(&config);
        let mut params = [Pod::from_bytes(&pod_bytes).ok_or("format pod")?];
        stream
            .connect(
                spa::utils::Direction::Output,
                None,
                pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS | pw::stream::StreamFlags::RT_PROCESS,
                &mut params,
            )
            .map_err(|e| e.to_string())?;
        drop(lock);
        thread_loop.start();

        Ok(Self { thread_loop, _context: context, _core: core, stream, _listener: listener, status, config })
    }

    pub fn status(&self) -> &Arc<AudioStatus> {
        &self.status
    }

    pub fn config(&self) -> &AudioConfig {
        &self.config
    }

    pub fn set_active(&self, active: bool) {
        let _lock = self.thread_loop.lock();
        let _ = self.stream.set_active(active);
    }
}

impl Drop for AudioHost {
    fn drop(&mut self) {
        {
            let _lock = self.thread_loop.lock();
            let _ = self.stream.disconnect();
        }
        self.thread_loop.stop();
    }
}

fn process(stream: &pw::stream::Stream, d: &mut RtData) {
    if !d.ftz_done {
        set_flush_denormals();
        d.ftz_done = true;
    }
    let now = Instant::now();
    d.status.callbacks.fetch_add(1, Ordering::Relaxed);
    let Some(mut buffer) = stream.dequeue_buffer() else {
        d.status.starved.fetch_add(1, Ordering::Relaxed);
        return;
    };
    let requested = buffer.requested() as usize;
    let datas = buffer.datas_mut();
    let Some(data) = datas.first_mut() else { return };
    let channels = d.channels.max(1);
    let stride = channels * 4;
    let Some(bytes) = data.data() else { return };
    let max_frames = bytes.len() / stride;
    let frames = if requested > 0 { requested.min(max_frames) } else { max_frames };
    if frames == 0 {
        return;
    }
    let n = frames * channels;
    if d.scratch.len() < n {
        // Only on a block larger than anything seen so far (should never happen
        // past the first callback); accept the one-off allocation over a glitch.
        d.scratch.resize(n, 0.0);
    }
    let out = &mut d.scratch[..n];
    out.fill(0.0);
    d.render.render(out, channels, frames, d.rate);
    for (dst, src) in bytes[..n * 4].chunks_exact_mut(4).zip(out.iter()) {
        dst.copy_from_slice(&src.to_le_bytes());
    }
    let chunk = data.chunk_mut();
    *chunk.offset_mut() = 0;
    *chunk.stride_mut() = stride as i32;
    *chunk.size_mut() = (frames * stride) as u32;

    d.status.block.store(frames as u32, Ordering::Relaxed);
    if let (Some(last), true) = (d.last, d.last_frames > 0) {
        let expect = d.last_frames as f64 / d.rate.max(1) as f64;
        if (now - last).as_secs_f64() > expect * 1.5 + 0.001 {
            d.status.late.fetch_add(1, Ordering::Relaxed);
        }
    }
    d.last = Some(now);
    d.last_frames = frames;
}

/// SPA `EnumFormat` pod for interleaved f32 at the configured rate/channels.
fn format_pod(config: &AudioConfig) -> Vec<u8> {
    let mut info = AudioInfoRaw::new();
    info.set_format(AudioFormat::F32LE);
    info.set_rate(config.rate);
    info.set_channels(config.channels);
    let mut pos = [0u32; MAX_CHANNELS];
    match config.channels {
        2 => {
            pos[0] = spa::sys::SPA_AUDIO_CHANNEL_FL;
            pos[1] = spa::sys::SPA_AUDIO_CHANNEL_FR;
        }
        n => {
            // Pro-audio style: raw AUX channels, mapped by index on the device.
            for (i, p) in pos.iter_mut().take(n as usize).enumerate() {
                *p = spa::sys::SPA_AUDIO_CHANNEL_AUX0 + i as u32;
            }
        }
    }
    info.set_position(pos);
    spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &spa::pod::Value::Object(spa::pod::Object {
            type_: spa::sys::SPA_TYPE_OBJECT_Format,
            id: spa::sys::SPA_PARAM_EnumFormat,
            properties: info.into(),
        }),
    )
    .expect("serialize format pod")
    .0
    .into_inner()
}

/// Flush-to-zero + denormals-are-zero on this (RT) thread: denormal floats in
/// filter tails otherwise cost 100× per op.
#[cfg(target_arch = "x86_64")]
fn set_flush_denormals() {
    #[allow(deprecated)]
    // SAFETY: only changes this thread's MXCSR rounding-control bits.
    unsafe {
        use std::arch::x86_64::{_mm_getcsr, _mm_setcsr};
        _mm_setcsr(_mm_getcsr() | 0x8040);
    }
}

#[cfg(not(target_arch = "x86_64"))]
fn set_flush_denormals() {}
