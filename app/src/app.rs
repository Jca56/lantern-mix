//! winit ApplicationHandler: window, events, repaint policy (Continuous / OnEvent +
//! 1 Hz heartbeat).

use std::sync::Arc;
use std::time::Instant;

use lmx_gpu::{Gpu, Painter, Text, Vec2};
use lmx_ui::{Input, MouseButton, Theme, Ui};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use crate::screens::{DemoScreen, Stats};

struct Gfx {
    gpu: Gpu,
    painter: Painter,
    text: Text,
}

pub struct App {
    window: Option<Arc<Window>>,
    gfx: Option<Gfx>,
    input: Input,
    ui: Ui,
    screen: DemoScreen,
    last_frame: Instant,
    frame_ms: f32,
    frames: u64,
}

impl App {
    pub fn new() -> Self {
        Self {
            window: None,
            gfx: None,
            input: Input::default(),
            ui: Ui::new(Theme::default()),
            screen: DemoScreen::default(),
            last_frame: Instant::now(),
            frame_ms: 0.0,
            frames: 0,
        }
    }

    pub fn run() -> Result<(), String> {
        let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
        // Sleep between events; redraws are requested explicitly.
        event_loop.set_control_flow(ControlFlow::Wait);
        let mut app = App::new();
        event_loop.run_app(&mut app).map_err(|e| e.to_string())
    }

    fn scale(&self) -> f32 {
        self.window.as_ref().map(|w| w.scale_factor() as f32).unwrap_or(1.0) * self.ui.theme.scale
    }

    fn redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn draw(&mut self) {
        let (Some(window), Some(gfx)) = (&self.window, &mut self.gfx) else { return };
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.25);
        self.last_frame = now;
        let (w, h) = gfx.gpu.size();
        if w == 0 || h == 0 {
            return;
        }
        let scale = window.scale_factor() as f32 * self.ui.theme.scale;
        let Some(mut frame) = gfx.gpu.begin_frame() else {
            window.request_redraw();
            return;
        };
        let t0 = Instant::now();
        gfx.painter.begin(scale, (w, h));
        gfx.text.begin(scale, w, h);
        {
            let stats = Stats {
                adapter: gfx.gpu.adapter_name.clone(),
                frame_ms: self.frame_ms,
                frames: self.frames,
                scale,
                continuous: self.ui.wants_continuous(),
            };
            let mut f = self.ui.frame(&mut gfx.painter, &mut gfx.text, &self.input, dt);
            self.screen.draw(&mut f, &stats);
        }
        self.input.begin_frame();
        let bg = self.ui.theme.bg;
        gfx.painter.render(&gfx.gpu.device, &gfx.gpu.queue, &mut frame.encoder, &frame.view, Some(bg));
        gfx.text.render(&mut frame.encoder, &frame.view);
        window.pre_present_notify();
        gfx.gpu.end_frame(frame);
        self.frame_ms = t0.elapsed().as_secs_f32() * 1000.0;
        self.frames += 1;
        if self.ui.wants_continuous() {
            window.request_redraw();
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("Lantern Mix")
            .with_inner_size(LogicalSize::new(1600.0, 1000.0));
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        let size = window.inner_size();
        let gpu = Gpu::new(window.clone(), size.width, size.height).expect("gpu");
        eprintln!("lantern-mix: gpu = {} | scale = {}", gpu.adapter_name, window.scale_factor());
        let painter = Painter::new(&gpu.device, gpu.format);
        let text = Text::new(&gpu);
        self.gfx = Some(Gfx { gpu, painter, text });
        self.window = Some(window);
        self.last_frame = Instant::now();
        self.redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(s) => {
                if let Some(g) = &mut self.gfx {
                    g.gpu.resize(s.width, s.height);
                }
                self.redraw();
            }
            WindowEvent::ScaleFactorChanged { .. } => self.redraw(),
            WindowEvent::CursorMoved { position, .. } => {
                let s = self.scale();
                self.input.on_cursor(Vec2::new(position.x as f32 / s, position.y as f32 / s));
                self.redraw();
            }
            WindowEvent::CursorLeft { .. } => {
                self.input.on_cursor_left();
                self.redraw();
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let b = match button {
                    winit::event::MouseButton::Left => MouseButton::Left,
                    winit::event::MouseButton::Right => MouseButton::Right,
                    winit::event::MouseButton::Middle => MouseButton::Middle,
                    _ => return,
                };
                self.input.on_button(b, state == ElementState::Pressed);
                self.redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (x * 40.0, y * 40.0),
                    MouseScrollDelta::PixelDelta(p) => {
                        let s = self.scale();
                        (p.x as f32 / s, p.y as f32 / s)
                    }
                };
                self.input.on_wheel(dx, dy);
                self.redraw();
            }
            WindowEvent::ModifiersChanged(m) => {
                let s = m.state();
                self.input.on_modifiers(s.shift_key(), s.control_key(), s.alt_key());
            }
            WindowEvent::DroppedFile(path) => {
                self.input.on_drop(path);
                self.redraw();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed && event.logical_key == Key::Named(NamedKey::Escape) {
                    event_loop.exit();
                }
                if let Some(t) = &event.text {
                    if event.state == ElementState::Pressed {
                        self.input.on_text(t);
                    }
                }
                self.redraw();
            }
            WindowEvent::RedrawRequested => self.draw(),
            _ => {}
        }
    }
}
