//! WGSL for the SDF shape pipeline + the per-instance record it consumes.
//!
//! One instanced draw per layer: every shape is a screen-space quad whose fragment
//! shader evaluates a signed distance (rounded rect / circle / ring / line / arc),
//! applies stroke, gradient, clip and 1-px anti-aliasing.

/// Shape kinds (`Instance::params.x`).
pub const KIND_RECT: f32 = 0.0;
pub const KIND_CIRCLE: f32 = 1.0;
pub const KIND_RING: f32 = 2.0;
pub const KIND_LINE: f32 = 3.0;
pub const KIND_ARC: f32 = 4.0;

/// Gradient modes (`Instance::params.w`).
pub const GRAD_NONE: f32 = 0.0;
pub const GRAD_H: f32 = 1.0;
pub const GRAD_V: f32 = 2.0;
pub const GRAD_RADIAL: f32 = 3.0;

/// Per-instance vertex data, physical pixels, linear colors.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Instance {
    /// x, y, w, h — for `KIND_LINE`: x0, y0, x1, y1.
    pub rect: [f32; 4],
    /// Clip rect x, y, w, h.
    pub clip: [f32; 4],
    pub color0: [f32; 4],
    pub color1: [f32; 4],
    /// kind, corner radius, stroke width (0 = filled), gradient mode.
    pub params: [f32; 4],
    /// ring/arc: thickness, a0, a1 ; line: width.
    pub extra: [f32; 4],
}

pub const INSTANCE_SIZE: u64 = std::mem::size_of::<Instance>() as u64;

pub const ATTRIBUTES: [wgpu::VertexAttribute; 6] = [
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 0, shader_location: 0 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 16, shader_location: 1 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 32, shader_location: 2 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 48, shader_location: 3 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 64, shader_location: 4 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 80, shader_location: 5 },
];

pub const WGSL: &str = r#"
struct Uniforms {
    viewport: vec2<f32>,
    _pad: vec2<f32>,
};
@group(0) @binding(0) var<uniform> u: Uniforms;

struct Inst {
    @location(0) rect: vec4<f32>,
    @location(1) clip: vec4<f32>,
    @location(2) color0: vec4<f32>,
    @location(3) color1: vec4<f32>,
    @location(4) params: vec4<f32>,
    @location(5) extra: vec4<f32>,
};

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) p: vec2<f32>,
    @location(1) @interpolate(flat) rect: vec4<f32>,
    @location(2) @interpolate(flat) clip: vec4<f32>,
    @location(3) @interpolate(flat) color0: vec4<f32>,
    @location(4) @interpolate(flat) color1: vec4<f32>,
    @location(5) @interpolate(flat) params: vec4<f32>,
    @location(6) @interpolate(flat) extra: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: Inst) -> VOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let c = corners[vi];
    var bb_min: vec2<f32>;
    var bb_max: vec2<f32>;
    let kind = u32(inst.params.x);
    if (kind == 3u) {
        let hw = inst.extra.x * 0.5 + 1.5;
        bb_min = min(inst.rect.xy, inst.rect.zw) - vec2<f32>(hw, hw);
        bb_max = max(inst.rect.xy, inst.rect.zw) + vec2<f32>(hw, hw);
    } else {
        bb_min = inst.rect.xy - vec2<f32>(1.5, 1.5);
        bb_max = inst.rect.xy + inst.rect.zw + vec2<f32>(1.5, 1.5);
    }
    let p = mix(bb_min, bb_max, c);
    var out: VOut;
    out.pos = vec4<f32>(p.x / u.viewport.x * 2.0 - 1.0, 1.0 - p.y / u.viewport.y * 2.0, 0.0, 1.0);
    out.p = p;
    out.rect = inst.rect;
    out.clip = inst.clip;
    out.color0 = inst.color0;
    out.color1 = inst.color1;
    out.params = inst.params;
    out.extra = inst.extra;
    return out;
}

fn sd_rrect(p: vec2<f32>, half: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - half + vec2<f32>(r, r);
    return length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - r;
}

fn sd_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
    return length(pa - ba * h);
}

const TAU: f32 = 6.28318530718;

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    let p = in.p;
    let clip = in.clip;
    if (p.x < clip.x || p.y < clip.y || p.x > clip.x + clip.z || p.y > clip.y + clip.w) {
        discard;
    }
    let kind = u32(in.params.x);
    let center = in.rect.xy + in.rect.zw * 0.5;
    let half = in.rect.zw * 0.5;
    let r_outer = min(half.x, half.y);
    var d: f32 = 0.0;
    var t: f32 = 0.0;

    if (kind == 0u) {
        let radius = min(in.params.y, r_outer);
        d = sd_rrect(p - center, half, radius);
    } else if (kind == 1u) {
        d = length(p - center) - r_outer;
    } else if (kind == 2u) {
        let th = in.extra.x;
        let r_mid = r_outer - th * 0.5;
        d = abs(length(p - center) - r_mid) - th * 0.5;
    } else if (kind == 3u) {
        d = sd_segment(p, in.rect.xy, in.rect.zw) - in.extra.x * 0.5;
    } else {
        let th = in.extra.x;
        let a0 = in.extra.y;
        let a1 = in.extra.z;
        let r_mid = r_outer - th * 0.5;
        let dv = p - center;
        var ang = atan2(dv.y, dv.x) - a0;
        ang = ang - floor(ang / TAU) * TAU;      // into [0, TAU)
        if (ang <= a1 - a0) {
            d = abs(length(dv) - r_mid) - th * 0.5;
        } else {
            let e0 = center + vec2<f32>(cos(a0), sin(a0)) * r_mid;
            let e1 = center + vec2<f32>(cos(a1), sin(a1)) * r_mid;
            d = min(length(p - e0), length(p - e1)) - th * 0.5;
        }
    }

    let stroke = in.params.z;
    if (stroke > 0.0) {
        d = abs(d + stroke * 0.5) - stroke * 0.5;
    }

    var color = in.color0;
    let grad = u32(in.params.w);
    if (grad == 1u) {
        t = clamp((p.x - in.rect.x) / max(in.rect.z, 1e-6), 0.0, 1.0);
        color = mix(in.color0, in.color1, t);
    } else if (grad == 2u) {
        t = clamp((p.y - in.rect.y) / max(in.rect.w, 1e-6), 0.0, 1.0);
        color = mix(in.color0, in.color1, t);
    } else if (grad == 3u) {
        t = clamp(length(p - center) / max(r_outer, 1e-6), 0.0, 1.0);
        color = mix(in.color0, in.color1, t);
    }

    let cov = clamp(0.5 - d, 0.0, 1.0);
    return vec4<f32>(color.rgb, color.a * cov);
}
"#;
