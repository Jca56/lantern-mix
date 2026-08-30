//! lntrn-text wrapper: queue/measure/clip in logical px with the window scale.
//!
//! lntrn-text works in physical pixels and renders every queued string in one
//! pass, so text always lands above the painter's shapes. (Overlay layers that
//! need text above *other* text get a second `Text` later.)

use crate::{Color, Gpu, Rect};
use lntrn_text::TextRenderer;

pub struct Text {
    r: TextRenderer,
    scale: f32,
    w: u32,
    h: u32,
}

impl Text {
    pub fn new(gpu: &Gpu) -> Self {
        let r = TextRenderer::from_wgpu(gpu.device.clone(), gpu.queue.clone(), gpu.format, false);
        Self { r, scale: 1.0, w: 1, h: 1 }
    }

    /// Call once per frame before queuing. `w`/`h` are the physical surface size.
    pub fn begin(&mut self, scale: f32, w: u32, h: u32) {
        self.scale = scale;
        self.w = w;
        self.h = h;
        self.r.clear();
    }

    /// Load extra font bytes (embedded fonts); the family then resolves by name.
    pub fn load_font(&mut self, data: Vec<u8>) {
        self.r.load_font_data(data);
    }

    /// Queue `s` with its top-left at (`x`,`y`) logical px, `size` logical px.
    pub fn draw(&mut self, s: &str, size: f32, x: f32, y: f32, color: Color) {
        let k = self.scale;
        self.r.queue(s, size * k, x * k, y * k, lc(color), f32::MAX, self.w, self.h);
    }

    pub fn draw_bold(&mut self, s: &str, size: f32, x: f32, y: f32, color: Color) {
        let k = self.scale;
        self.r.queue_styled(
            s,
            size * k,
            x * k,
            y * k,
            lc(color),
            f32::MAX,
            lntrn_text::FontWeight::Bold,
            lntrn_text::FontStyle::Normal,
            self.w,
            self.h,
        );
    }

    /// Queue in a specific font family (e.g. a display face for readouts).
    pub fn draw_family(&mut self, s: &str, size: f32, x: f32, y: f32, color: Color, family: &str) {
        let k = self.scale;
        self.r.queue_family(s, size * k, x * k, y * k, lc(color), f32::MAX, family, self.w, self.h);
    }

    /// Advance width of `s` at `size`, in logical px.
    pub fn width(&mut self, s: &str, size: f32) -> f32 {
        self.r.measure_width(s, size * self.scale) / self.scale
    }

    pub fn width_family(&mut self, s: &str, size: f32, family: &str) -> f32 {
        self.r.measure_width_family(s, size * self.scale, family) / self.scale
    }

    pub fn push_clip(&mut self, r: Rect) {
        let k = self.scale;
        self.r.push_clip([r.x * k, r.y * k, r.w * k, r.h * k]);
    }

    pub fn pop_clip(&mut self) {
        self.r.pop_clip();
    }

    /// Hide already-queued text inside `r` (an overlay panel drawn over it).
    pub fn occlude(&mut self, r: Rect) {
        let k = self.scale;
        self.r.occlude_rect([r.x * k, r.y * k, r.w * k, r.h * k]);
    }

    /// Draw everything queued this frame.
    pub fn render(&mut self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        self.r.render(encoder, view, self.w, self.h);
    }
}

fn lc(c: Color) -> lntrn_draw::Color {
    lntrn_draw::Color::rgba(c.r, c.g, c.b, c.a)
}
