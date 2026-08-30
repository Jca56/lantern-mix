//! winit ApplicationHandler: window, events, repaint policy (Continuous / OnEvent +
//! 1 Hz heartbeat).

use std::sync::Arc;
use std::time::Instant;

use lmx_gpu::{Gpu, Painter, Text, Vec2};
use lmx_ui::{Input, Key as UiKey, MouseButton, Theme, Ui};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use crate::db::Db;
use crate::screens::{DeckView, PerformanceScreen};
use crate::settings::Settings;
use crate::titlebar::{self, TitleAction, TitleBar};
use crate::wiring::Audio;
use crate::workers::{Loader, UserEvent, WorkerMsg};
use lmx_gpu::WaveformRenderer;
use lmx_ui::Rect;
use std::path::PathBuf;
use winit::window::CursorIcon;

struct Gfx {
    gpu: Gpu,
    painter: Painter,
    text: Text,
    waves: WaveformRenderer,
}

pub struct App {
    window: Option<Arc<Window>>,
    gfx: Option<Gfx>,
    input: Input,
    ui: Ui,
    screen: PerformanceScreen,
    audio: Audio,
    db: Db,
    loader: Loader,
    /// Files from the command line, loaded onto decks 1.. once the loop runs.
    startup_paths: Vec<PathBuf>,
    titlebar: TitleBar,
    settings: Settings,
    cursor: CursorIcon,
    quit: bool,
    last_frame: Instant,
}

impl App {
    pub fn new(startup_paths: Vec<PathBuf>, proxy: winit::event_loop::EventLoopProxy<UserEvent>) -> Self {
        Self {
            window: None,
            gfx: None,
            input: Input::default(),
            ui: Ui::new(Theme::default()),
            screen: PerformanceScreen::default(),
            audio: Audio::start(proxy),
            db: Db::open(),
            loader: Loader::new(),
            startup_paths,
            titlebar: TitleBar::default(),
            settings: Settings::load(),
            cursor: CursorIcon::Default,
            quit: false,
            last_frame: Instant::now(),
        }
    }

    pub fn run(paths: Vec<PathBuf>) -> Result<(), String> {
        let event_loop = EventLoop::<UserEvent>::with_user_event().build().map_err(|e| e.to_string())?;
        // Sleep between events; redraws are requested explicitly.
        event_loop.set_control_flow(ControlFlow::Wait);
        let mut app = App::new(paths, event_loop.create_proxy());
        app.loader.set_proxy(event_loop.create_proxy());
        event_loop.run_app(&mut app).map_err(|e| e.to_string())
    }

    fn rescan(&mut self) {
        self.screen.browser.scanning = true;
        self.loader.scan(self.db.lib.roots.clone(), self.db.known_files());
    }

    /// Collect finished loads: upload the waveform, hand the audio to the
    /// engine, update the deck view.
    fn collect_loads(&mut self) {
        while let Some(msg) = self.loader.try_recv() {
            let l = match msg {
                WorkerMsg::Loaded(l) => l,
                WorkerMsg::Scanned { roots, files } => {
                    self.db.merge_scan(&roots, files);
                    self.screen.browser.scanning = false;
                    continue;
                }
            };
            match l.result {
                Ok((audio, meta, probe, summary)) => {
                    let Some(gfx) = &mut self.gfx else { continue };
                    if let Some(old) = self.screen.decks[l.deck].wave.take() {
                        gfx.waves.remove(old);
                    }
                    let id = gfx.waves.upload(&gfx.gpu.device, &gfx.gpu.queue, &summary.fine, &summary.overview);
                    let title = meta.title.clone().unwrap_or_else(|| {
                        l.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
                    });
                    let track = self.db.ensure_track(&l.path);
                    let grid = track.and_then(|t| self.db.lib.get(t)).map(|t| t.grid).unwrap_or_default();
                    self.screen.decks[l.deck] = DeckView {
                        track,
                        title,
                        artist: meta.artist.clone().unwrap_or_default(),
                        wave: Some(id),
                        columns: summary.fine.len() as u32,
                        sample_rate: probe.sample_rate,
                        frames: probe.duration_frames,
                        bpm_tag: meta.bpm_tag,
                        key_tag: meta.key_tag.clone(),
                        bpm: if grid.bpm > 0.0 {
                            grid.bpm
                        } else {
                            meta.bpm_tag.filter(|b| *b >= 20.0 && *b <= 400.0).unwrap_or(crate::screens::DEFAULT_BPM)
                        },
                        anchor_frame: grid.anchor_frame,
                        scrub: None,
                        bpm_edit: None,
                        bpm_focus: false,
                        grid_dirty: false,
                    };
                    eprintln!(
                        "lantern-mix: deck {} ← {} ({} Hz, {:.1} s, {} columns)",
                        l.deck + 1,
                        l.path.display(),
                        probe.sample_rate,
                        probe.duration_secs(),
                        summary.fine.len()
                    );
                    self.audio.load(l.deck, audio);
                }
                Err(e) => eprintln!("lantern-mix: load {} failed: {e}", l.path.display()),
            }
        }
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
        gfx.painter.begin(scale, (w, h));
        gfx.text.begin(scale, w, h);
        let maximized = window.is_maximized();
        let snap = self.audio.poll();
        let (action, cursor, browser_actions) = {
            let mut f = self.ui.frame(&mut gfx.painter, &mut gfx.text, &self.input, dt);
            let (action, cursor, bar_free) = self.titlebar.draw(&mut f, maximized, &self.settings);
            let area = Rect::new(0.0, titlebar::HEIGHT, f.size.x, f.size.y - titlebar::HEIGHT);
            let ba = self.screen.draw(&mut f, &mut self.audio, &self.loader, &mut self.db, &self.settings, &snap, area, bar_free);
            (action, cursor, ba)
        };
        self.input.begin_frame();
        for (deck, id) in browser_actions.load {
            if let Some(t) = self.db.lib.get(id) {
                self.loader.load(deck, t.path.clone());
            }
        }
        if !browser_actions.add_roots.is_empty() {
            for r in browser_actions.add_roots {
                self.db.add_root(r);
            }
            self.screen.browser.scanning = true;
            self.loader.scan(self.db.lib.roots.clone(), self.db.known_files());
        }
        if cursor != self.cursor {
            self.cursor = cursor;
            window.set_cursor(cursor);
        }
        let window = window.clone();
        match action {
            TitleAction::None => {}
            TitleAction::Minimize => window.set_minimized(true),
            TitleAction::ToggleMaximize => window.set_maximized(!maximized),
            TitleAction::Close => self.quit = true,
            TitleAction::Drag => {
                let _ = window.drag_window();
            }
            TitleAction::Resize(dir) => {
                let _ = window.drag_resize_window(dir);
            }
            TitleAction::SetWaveOrder(o) => {
                self.settings.wave_order = o;
                self.settings.save();
            }
        }
        let bg = self.ui.theme.bg;
        gfx.painter.upload(&gfx.gpu.device, &gfx.gpu.queue);
        gfx.painter.draw_layers(&mut frame.encoder, &frame.view, 0..1, Some(bg));
        gfx.waves.render(&gfx.gpu.device, &gfx.gpu.queue, &mut frame.encoder, &frame.view, (w, h), gfx.painter.waves());
        gfx.painter.draw_layers(&mut frame.encoder, &frame.view, 1..lmx_gpu::LAYERS, None);
        gfx.text.render(&mut frame.encoder, &frame.view);
        window.pre_present_notify();
        gfx.gpu.end_frame(frame);
        if self.ui.wants_continuous() {
            window.request_redraw();
        }
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, _event: UserEvent) {
        self.collect_loads();
        self.redraw();
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("Lantern Mix")
            .with_decorations(false)
            .with_inner_size(LogicalSize::new(1600.0, 1000.0));
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        let size = window.inner_size();
        let gpu = Gpu::new(window.clone(), size.width, size.height).expect("gpu");
        eprintln!("lantern-mix: gpu = {} | scale = {}", gpu.adapter_name, window.scale_factor());
        let painter = Painter::new(&gpu.device, gpu.format);
        let text = Text::new(&gpu);
        let waves = WaveformRenderer::new(&gpu.device, gpu.format);
        self.gfx = Some(Gfx { gpu, painter, text, waves });
        self.window = Some(window);
        for (i, p) in std::mem::take(&mut self.startup_paths).into_iter().take(4).enumerate() {
            self.loader.load(i, p);
        }
        if self.db.lib.roots.is_empty() {
            for r in self.settings.roots.clone() {
                self.db.add_root(r);
            }
        }
        self.rescan();
        self.last_frame = Instant::now();
        self.redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.db.snapshot();
                event_loop.exit();
            }
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
                if event.state == ElementState::Pressed {
                    let named = match &event.logical_key {
                        Key::Named(NamedKey::Backspace) => Some(UiKey::Backspace),
                        Key::Named(NamedKey::Delete) => Some(UiKey::Delete),
                        Key::Named(NamedKey::Enter) => Some(UiKey::Enter),
                        Key::Named(NamedKey::Escape) => Some(UiKey::Escape),
                        Key::Named(NamedKey::Tab) => Some(UiKey::Tab),
                        Key::Named(NamedKey::ArrowUp) => Some(UiKey::Up),
                        Key::Named(NamedKey::ArrowDown) => Some(UiKey::Down),
                        Key::Named(NamedKey::ArrowLeft) => Some(UiKey::Left),
                        Key::Named(NamedKey::ArrowRight) => Some(UiKey::Right),
                        Key::Named(NamedKey::Home) => Some(UiKey::Home),
                        Key::Named(NamedKey::End) => Some(UiKey::End),
                        Key::Named(NamedKey::PageUp) => Some(UiKey::PageUp),
                        Key::Named(NamedKey::PageDown) => Some(UiKey::PageDown),
                        Key::Named(NamedKey::Space) => Some(UiKey::Space),
                        _ => None,
                    };
                    if let Some(k) = named {
                        self.input.on_key(k);
                    } else if let Some(t) = &event.text {
                        self.input.on_text(t);
                    }
                }
                self.redraw();
            }
            WindowEvent::RedrawRequested => {
                self.draw();
                if self.quit {
                    self.db.snapshot();
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }
}
