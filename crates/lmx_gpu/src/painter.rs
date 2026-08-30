//! Per-frame instance list: rect/rrect/circle/line/arc/ring with gradients and
//! clip, by layer. API in logical px; one instanced draw per layer.

use crate::shapes::{self, Instance, INSTANCE_SIZE};
use crate::{Color, Rect, Vec2, LAYERS};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Gradient {
    Horizontal,
    Vertical,
    Radial,
}

pub struct Painter {
    layers: [Vec<Instance>; LAYERS],
    layer: usize,
    clip_stack: Vec<Rect>,
    scale: f32,
    viewport: (u32, u32),
    pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    instances: wgpu::Buffer,
    capacity: u64,
}

impl Painter {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("lmx shapes"),
            source: wgpu::ShaderSource::Wgsl(shapes::WGSL.into()),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("lmx shapes uniforms"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lmx shapes viewport"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lmx shapes uniforms"),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniforms.as_entire_binding() }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("lmx shapes"),
            bind_group_layouts: &[&bgl],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("lmx shapes"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: INSTANCE_SIZE,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &shapes::ATTRIBUTES,
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
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let capacity = 4096;
        let instances = Self::make_buffer(device, capacity);
        Self {
            layers: std::array::from_fn(|_| Vec::new()),
            layer: 0,
            clip_stack: Vec::new(),
            scale: 1.0,
            viewport: (1, 1),
            pipeline,
            uniforms,
            bind_group,
            instances,
            capacity,
        }
    }

    fn make_buffer(device: &wgpu::Device, capacity: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lmx shape instances"),
            size: capacity * INSTANCE_SIZE,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Start a frame. `viewport` is the physical surface size.
    pub fn begin(&mut self, scale: f32, viewport: (u32, u32)) {
        for l in &mut self.layers {
            l.clear();
        }
        self.layer = 0;
        self.clip_stack.clear();
        self.scale = scale;
        self.viewport = viewport;
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Logical size of the surface.
    pub fn logical_size(&self) -> Vec2 {
        Vec2::new(self.viewport.0 as f32 / self.scale, self.viewport.1 as f32 / self.scale)
    }

    pub fn set_layer(&mut self, layer: usize) {
        self.layer = layer.min(LAYERS - 1);
    }

    pub fn layer(&self) -> usize {
        self.layer
    }

    pub fn push_clip(&mut self, r: Rect) {
        let r = match self.clip_stack.last() {
            Some(c) => c.intersect(&r),
            None => r,
        };
        self.clip_stack.push(r);
    }

    pub fn pop_clip(&mut self) {
        self.clip_stack.pop();
    }

    fn clip_phys(&self) -> [f32; 4] {
        match self.clip_stack.last() {
            Some(c) => [c.x * self.scale, c.y * self.scale, c.w * self.scale, c.h * self.scale],
            None => [-1.0e6, -1.0e6, 2.0e6, 2.0e6],
        }
    }

    fn push(&mut self, rect: [f32; 4], kind: f32, radius: f32, stroke: f32, grad: f32, c0: Color, c1: Color, extra: [f32; 4]) {
        let k = self.scale;
        let inst = Instance {
            rect: [rect[0] * k, rect[1] * k, rect[2] * k, rect[3] * k],
            clip: self.clip_phys(),
            color0: c0.to_linear(),
            color1: c1.to_linear(),
            params: [kind, radius * k, stroke * k, grad],
            extra: [extra[0] * k, extra[1], extra[2], extra[3]],
        };
        self.layers[self.layer].push(inst);
    }

    // ── shapes (logical px) ──────────────────────────────────────────────

    pub fn fill_rect(&mut self, r: Rect, c: Color) {
        self.push([r.x, r.y, r.w, r.h], shapes::KIND_RECT, 0.0, 0.0, shapes::GRAD_NONE, c, c, [0.0; 4]);
    }

    pub fn fill_rrect(&mut self, r: Rect, radius: f32, c: Color) {
        self.push([r.x, r.y, r.w, r.h], shapes::KIND_RECT, radius, 0.0, shapes::GRAD_NONE, c, c, [0.0; 4]);
    }

    /// Outline drawn *inside* the rect bounds.
    pub fn stroke_rrect(&mut self, r: Rect, radius: f32, width: f32, c: Color) {
        self.push([r.x, r.y, r.w, r.h], shapes::KIND_RECT, radius, width, shapes::GRAD_NONE, c, c, [0.0; 4]);
    }

    pub fn gradient_rrect(&mut self, r: Rect, radius: f32, c0: Color, c1: Color, dir: Gradient) {
        let g = match dir {
            Gradient::Horizontal => shapes::GRAD_H,
            Gradient::Vertical => shapes::GRAD_V,
            Gradient::Radial => shapes::GRAD_RADIAL,
        };
        self.push([r.x, r.y, r.w, r.h], shapes::KIND_RECT, radius, 0.0, g, c0, c1, [0.0; 4]);
    }

    pub fn circle(&mut self, center: Vec2, radius: f32, c: Color) {
        let r = Rect::from_center(center, radius * 2.0, radius * 2.0);
        self.push([r.x, r.y, r.w, r.h], shapes::KIND_CIRCLE, 0.0, 0.0, shapes::GRAD_NONE, c, c, [0.0; 4]);
    }

    /// Radial glow: `c` at the center fading to transparent at `radius`.
    pub fn glow(&mut self, center: Vec2, radius: f32, c: Color) {
        let r = Rect::from_center(center, radius * 2.0, radius * 2.0);
        self.push([r.x, r.y, r.w, r.h], shapes::KIND_CIRCLE, 0.0, 0.0, shapes::GRAD_RADIAL, c, c.with_alpha(0.0), [0.0; 4]);
    }

    pub fn circle_stroke(&mut self, center: Vec2, radius: f32, width: f32, c: Color) {
        let r = Rect::from_center(center, radius * 2.0, radius * 2.0);
        self.push([r.x, r.y, r.w, r.h], shapes::KIND_CIRCLE, 0.0, width, shapes::GRAD_NONE, c, c, [0.0; 4]);
    }

    /// Ring whose *outer* edge is at `radius`, `thickness` thick.
    pub fn ring(&mut self, center: Vec2, radius: f32, thickness: f32, c: Color) {
        let r = Rect::from_center(center, radius * 2.0, radius * 2.0);
        self.push([r.x, r.y, r.w, r.h], shapes::KIND_RING, 0.0, 0.0, shapes::GRAD_NONE, c, c, [thickness, 0.0, 0.0, 0.0]);
    }

    /// Arc of the same ring from angle `a0` to `a1` (radians, screen space: 0 =
    /// right, increasing clockwise on screen), round caps.
    pub fn arc(&mut self, center: Vec2, radius: f32, thickness: f32, a0: f32, a1: f32, c: Color) {
        if a1 <= a0 {
            return;
        }
        let r = Rect::from_center(center, radius * 2.0, radius * 2.0);
        let sweep = (a1 - a0).min(std::f32::consts::TAU - 1e-4);
        self.push([r.x, r.y, r.w, r.h], shapes::KIND_ARC, 0.0, 0.0, shapes::GRAD_NONE, c, c, [thickness, a0, a0 + sweep, 0.0]);
    }

    pub fn line(&mut self, a: Vec2, b: Vec2, width: f32, c: Color) {
        self.push([a.x, a.y, b.x, b.y], shapes::KIND_LINE, 0.0, 0.0, shapes::GRAD_NONE, c, c, [width, 0.0, 0.0, 0.0]);
    }

    /// Horizontal hairline that fades out at both ends.
    pub fn divider(&mut self, r: Rect, c: Color) {
        let mid = Rect::new(r.x, r.y, r.w, r.h);
        let (l, rr) = (Rect::new(mid.x, mid.y, mid.w * 0.5, mid.h), Rect::new(mid.x + mid.w * 0.5, mid.y, mid.w * 0.5, mid.h));
        self.gradient_rrect(l, 0.0, c.with_alpha(0.0), c, Gradient::Horizontal);
        self.gradient_rrect(rr, 0.0, c, c.with_alpha(0.0), Gradient::Horizontal);
    }

    pub fn instance_count(&self) -> usize {
        self.layers.iter().map(Vec::len).sum()
    }

    // ── GPU ──────────────────────────────────────────────────────────────

    /// Upload and draw all layers in order. `clear` fills the target first.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        clear: Option<Color>,
    ) {
        let total = self.instance_count() as u64;
        if total > self.capacity {
            self.capacity = total.next_power_of_two();
            self.instances = Self::make_buffer(device, self.capacity);
        }
        let vp = [self.viewport.0 as f32, self.viewport.1 as f32, 0.0, 0.0];
        queue.write_buffer(&self.uniforms, 0, as_bytes(&vp));
        let mut offset = 0u64;
        let mut ranges = [(0u32, 0u32); LAYERS];
        for (i, l) in self.layers.iter().enumerate() {
            if !l.is_empty() {
                queue.write_buffer(&self.instances, offset * INSTANCE_SIZE, as_bytes(l.as_slice()));
            }
            ranges[i] = (offset as u32, (offset + l.len() as u64) as u32);
            offset += l.len() as u64;
        }
        let load = match clear {
            Some(c) => {
                let [r, g, b, a] = c.to_linear();
                wgpu::LoadOp::Clear(wgpu::Color { r: r as f64, g: g as f64, b: b as f64, a: a as f64 })
            }
            None => wgpu::LoadOp::Load,
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("lmx shapes"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        if total > 0 {
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.instances.slice(..));
            for (a, b) in ranges {
                if b > a {
                    pass.draw(0..6, a..b);
                }
            }
        }
    }
}

/// View a slice of plain-old-data as bytes. `T` must be `#[repr(C)]` with no
/// padding-sensitive invariants (all our instance/uniform types are float arrays).
fn as_bytes<T: Copy>(v: &[T]) -> &[u8] {
    // SAFETY: T is Copy, plain floats; length is the slice's byte length.
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) }
}
