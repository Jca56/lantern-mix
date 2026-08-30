//! Decoded audio as every crate sees it: interleaved stereo f32.

/// A whole decoded track: interleaved stereo f32 in −1..1.
#[derive(Clone, Debug, Default)]
pub struct TrackAudio {
    pub sample_rate: u32,
    /// Always 2 after decoding.
    pub channels: u16,
    pub frames: Vec<f32>,
}

impl TrackAudio {
    pub fn frame_count(&self) -> usize {
        self.frames.len() / self.channels.max(1) as usize
    }
    pub fn duration_secs(&self) -> f64 {
        self.frame_count() as f64 / self.sample_rate.max(1) as f64
    }
}
