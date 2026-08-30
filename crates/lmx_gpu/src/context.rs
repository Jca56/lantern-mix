//! Device/queue/surface setup and resize; sRGB swapchain; VSync.
//!
//! `Gpu::new` takes anything wgpu can make a surface from (an `Arc<winit::Window>`
//! qualifies) so this crate never depends on winit itself.

use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

pub struct Gpu {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub format: wgpu::TextureFormat,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    _instance: wgpu::Instance,
    pub adapter_name: String,
}

/// One swapchain frame: acquire in `begin_frame`, draw into `view` through
/// `encoder`, hand back to `end_frame`.
pub struct Frame {
    pub texture: wgpu::SurfaceTexture,
    pub view: wgpu::TextureView,
    pub encoder: wgpu::CommandEncoder,
}

impl Gpu {
    pub fn new(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN | wgpu::Backends::GL,
            ..Default::default()
        });
        let surface = instance.create_surface(target).map_err(|e| format!("create_surface: {e}"))?;
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .map_err(|e| format!("request_adapter: {e}"))?;
        let info = adapter.get_info();
        let adapter_name = format!("{} ({:?})", info.name, info.backend);
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("lmx"),
            ..Default::default()
        }))
        .map_err(|e| format!("request_device: {e}"))?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let mut config = surface
            .get_default_config(&adapter, width.max(1), height.max(1))
            .ok_or("surface not supported by adapter")?;
        config.format = format;
        config.present_mode = wgpu::PresentMode::Fifo; // vsync: the repaint policy relies on it
        config.view_formats = vec![];
        surface.configure(&device, &config);

        Ok(Self {
            device: Arc::new(device),
            queue: Arc::new(queue),
            format,
            surface,
            config,
            _instance: instance,
            adapter_name,
        })
    }

    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return; // minimized: keep the old config, skip frames
        }
        if (width, height) == (self.config.width, self.config.height) {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    /// Acquire the next swapchain image. `None` means "skip this frame" (the
    /// surface was lost/outdated and has been reconfigured, or timed out).
    pub fn begin_frame(&mut self) -> Option<Frame> {
        let texture = match self.surface.get_current_texture() {
            Ok(t) => t,
            Err(wgpu::SurfaceError::Lost) | Err(wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                return None;
            }
            Err(wgpu::SurfaceError::Timeout) => return None,
            Err(wgpu::SurfaceError::OutOfMemory) => panic!("GPU out of memory"),
            Err(wgpu::SurfaceError::Other) => return None,
        };
        if texture.suboptimal {
            // Draw anyway; reconfigure on the next resize.
        }
        let view = texture.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("lmx frame") });
        Some(Frame { texture, view, encoder })
    }

    pub fn end_frame(&self, frame: Frame) {
        self.queue.submit(std::iter::once(frame.encoder.finish()));
        frame.texture.present();
    }
}

struct ThreadWaker(std::thread::Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

/// Drive a future to completion on this thread. wgpu's setup calls are async only
/// for the web; on native they resolve after a poll or two.
pub fn block_on<F: Future>(fut: F) -> F::Output {
    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut fut = std::pin::pin!(fut);
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => std::thread::park(),
        }
    }
}
