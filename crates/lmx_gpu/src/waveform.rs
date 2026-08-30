//! Summary textures (peak/rms + 3 bands) and the waveform shader.
//!
//! A summary is uploaded once per track as an RGBA8 texture with two texels per
//! column — `[peak_l, peak_r, rms_l, rms_r]` and `[low, mid, high, 0]` — laid out
//! row-major `TEX_W` wide. Drawing is one instance per strip: the fragment shader
//! maps pixel x → column, height from peak, a brighter RMS core, and color from
//! the band mix. Scrolling and zooming only change instance parameters.

use crate::Color;
use std::collections::HashMap;

pub const TEX_W: u32 = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WaveId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaveLevel {
    Fine,
    Overview,
}

/// One strip to draw, physical px. Produced by the painter, consumed here.
#[derive(Clone, Copy, Debug)]
pub struct WaveDraw {
    pub id: WaveId,
    pub level: WaveLevel,
    pub rect: [f32; 4],
    pub clip: [f32; 4],
    pub first_col: f32,
    pub cols_per_px: f32,
    pub alpha: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Instance {
    rect: [f32; 4],
    clip: [f32; 4],
    /// first_col, cols_per_px, columns, alpha
    params: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Uniforms {
    viewport: [f32; 4],
    low: [f32; 4],
    mid: [f32; 4],
    high: [f32; 4],
}

struct Entry {
    fine: (wgpu::BindGroup, u32),
    overview: (wgpu::BindGroup, u32),
}

pub struct WaveformRenderer {
    pipeline: wgpu::RenderPipeline,
    tex_layout: wgpu::BindGroupLayout,
    uniforms: wgpu::Buffer,
    uniform_bg: wgpu::BindGroup,
    instances: wgpu::Buffer,
    capacity: u64,
    entries: HashMap<WaveId, Entry>,
    next_id: u32,
    pub colors: [Color; 3],
}

impl WaveformRenderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("lmx waveform"),
            source: wgpu::ShaderSource::Wgsl(WGSL.into()),
        });
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("lmx waveform uniforms"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let tex_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("lmx waveform texture"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lmx waveform uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lmx waveform uniforms"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniforms.as_entire_binding() }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("lmx waveform"),
            bind_group_layouts: &[&uniform_layout, &tex_layout],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("lmx waveform"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 0, shader_location: 0 },
                        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 16, shader_location: 1 },
                        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 32, shader_location: 2 },
                    ],
                }],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let capacity = 64;
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lmx waveform instances"),
            size: capacity * std::mem::size_of::<Instance>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            tex_layout,
            uniforms,
            uniform_bg,
            instances,
            capacity,
            entries: HashMap::new(),
            next_id: 1,
            colors: [Color::hex(0xFF1414), Color::hex(0x10D030), Color::hex(0x1A5CFF)],
        }
    }

    fn upload_level(&self, device: &wgpu::Device, queue: &wgpu::Queue, cols: &[[u8; 8]]) -> (wgpu::BindGroup, u32) {
        let texels = (cols.len() as u32 * 2).max(1);
        let rows = texels.div_ceil(TEX_W).max(1);
        let mut data = vec![0u8; (TEX_W * rows * 4) as usize];
        for (i, c) in cols.iter().enumerate() {
            data[i * 8..i * 8 + 8].copy_from_slice(c);
        }
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("lmx waveform summary"),
            size: wgpu::Extent3d { width: TEX_W, height: rows, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &data,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(TEX_W * 4), rows_per_image: Some(rows) },
            wgpu::Extent3d { width: TEX_W, height: rows, depth_or_array_layers: 1 },
        );
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lmx waveform texture"),
            layout: &self.tex_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }],
        });
        (bg, cols.len() as u32)
    }

    /// Upload a track's summaries. Returns the id the painter draws with.
    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, fine: &[[u8; 8]], overview: &[[u8; 8]]) -> WaveId {
        let id = WaveId(self.next_id);
        self.next_id += 1;
        let entry = Entry { fine: self.upload_level(device, queue, fine), overview: self.upload_level(device, queue, overview) };
        self.entries.insert(id, entry);
        id
    }

    pub fn remove(&mut self, id: WaveId) {
        self.entries.remove(&id);
    }

    /// Draw queued strips into `view` (loads what's there; no clear).
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        viewport: (u32, u32),
        draws: &[WaveDraw],
    ) {
        let draws: Vec<&WaveDraw> = draws.iter().filter(|d| self.entries.contains_key(&d.id)).collect();
        if draws.is_empty() {
            return;
        }
        let u = Uniforms {
            viewport: [viewport.0 as f32, viewport.1 as f32, TEX_W as f32, 0.0],
            low: self.colors[0].to_linear(),
            mid: self.colors[1].to_linear(),
            high: self.colors[2].to_linear(),
        };
        queue.write_buffer(&self.uniforms, 0, as_bytes(std::slice::from_ref(&u)));
        let inst: Vec<Instance> = draws
            .iter()
            .map(|d| {
                let e = &self.entries[&d.id];
                let cols = match d.level {
                    WaveLevel::Fine => e.fine.1,
                    WaveLevel::Overview => e.overview.1,
                } as f32;
                Instance { rect: d.rect, clip: d.clip, params: [d.first_col, d.cols_per_px, cols, d.alpha] }
            })
            .collect();
        if inst.len() as u64 > self.capacity {
            self.capacity = (inst.len() as u64).next_power_of_two();
            self.instances = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("lmx waveform instances"),
                size: self.capacity * std::mem::size_of::<Instance>() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        queue.write_buffer(&self.instances, 0, as_bytes(&inst));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("lmx waveforms"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.uniform_bg, &[]);
        pass.set_vertex_buffer(0, self.instances.slice(..));
        for (i, d) in draws.iter().enumerate() {
            let e = &self.entries[&d.id];
            let bg = match d.level {
                WaveLevel::Fine => &e.fine.0,
                WaveLevel::Overview => &e.overview.0,
            };
            pass.set_bind_group(1, bg, &[]);
            pass.draw(0..6, i as u32..i as u32 + 1);
        }
    }
}

fn as_bytes<T: Copy>(v: &[T]) -> &[u8] {
    // SAFETY: plain repr(C) float arrays, byte length from the slice.
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) }
}

const WGSL: &str = r#"
struct U { viewport: vec4<f32>, low: vec4<f32>, mid: vec4<f32>, high: vec4<f32> };
@group(0) @binding(0) var<uniform> u: U;
@group(1) @binding(0) var wave: texture_2d<f32>;

struct Inst {
    @location(0) rect: vec4<f32>,
    @location(1) clip: vec4<f32>,
    @location(2) params: vec4<f32>,
};
struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) p: vec2<f32>,
    @location(1) @interpolate(flat) rect: vec4<f32>,
    @location(2) @interpolate(flat) clip: vec4<f32>,
    @location(3) @interpolate(flat) params: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: Inst) -> VOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0));
    let c = corners[vi];
    let p = inst.rect.xy + inst.rect.zw * c;
    var out: VOut;
    out.pos = vec4<f32>(p.x / u.viewport.x * 2.0 - 1.0, 1.0 - p.y / u.viewport.y * 2.0, 0.0, 1.0);
    out.p = p;
    out.rect = inst.rect;
    out.clip = inst.clip;
    out.params = inst.params;
    return out;
}

fn fetch(col: i32, which: i32) -> vec4<f32> {
    let t = col * 2 + which;
    let w = i32(u.viewport.z);
    return textureLoad(wave, vec2<i32>(t % w, t / w), 0);
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    let p = in.p;
    let clip = in.clip;
    if (p.x < clip.x || p.y < clip.y || p.x > clip.x + clip.z || p.y > clip.y + clip.w) { discard; }
    let columns = in.params.z;
    let cpp = max(in.params.y, 1e-4);
    let cx = in.params.x + (p.x - in.rect.x) * cpp;
    if (cx < 0.0 || cx >= columns) { discard; }
    let n = clamp(i32(ceil(cpp)), 1, 4);
    var peak = 0.0;
    var rms = 0.0;
    var bands = vec3<f32>(0.0);
    for (var i = 0; i < n; i = i + 1) {
        let c = i32(cx + f32(i) * cpp / f32(n));
        if (f32(c) >= columns) { break; }
        let a = fetch(c, 0);
        let b = fetch(c, 1);
        peak = max(peak, max(a.x, a.y));
        rms = max(rms, max(a.z, a.w));
        bands = max(bands, b.xyz);
    }
    let half = in.rect.w * 0.5;
    let cy = in.rect.y + half;
    let d = abs(p.y - cy);
    let cov = clamp(peak * half - d + 0.5, 0.0, 1.0);
    if (cov <= 0.0) { discard; }
    let w = pow(bands, vec3<f32>(4.0));
    let s = max(w.x + w.y + w.z, 1e-3);
    let col = (u.low.rgb * w.x + u.mid.rgb * w.y + u.high.rgb * w.z) / s;
    let core = clamp(rms * half - d + 0.5, 0.0, 1.0);
    let rgb = mix(col * 0.9, mix(col, vec3<f32>(1.0), 0.18), core);
    return vec4<f32>(rgb, in.params.w * cov);
}
"#;
