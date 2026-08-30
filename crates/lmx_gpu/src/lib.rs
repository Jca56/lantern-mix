//! wgpu 28 painter: surface/device, SDF shape pipeline, textures, waveform pipeline,
//! layers, clipping, and text via lntrn-text.
//!
//! Everything public here speaks **logical pixels**; the window scale factor is
//! applied once, inside the painter and the text wrapper. Coordinates are y-down,
//! origin top-left. Colors are sRGB in the API and converted to linear before the
//! GPU sees them (the swapchain is an sRGB format).
//!
//! Design: `docs/05-UI.md` (§ `lmx_gpu` — the painter).

pub mod color;
pub mod context;
pub mod geom;
pub mod painter;
pub mod shapes;
pub mod text;
pub mod texture;
pub mod waveform;

pub use color::Color;
pub use context::{Frame, Gpu};
pub use geom::{Rect, Vec2};
pub use painter::{Gradient, Painter};
pub use text::Text;

/// Number of draw layers the painter keeps (0 = base, 1 = overlays, 2 = modals).
pub const LAYERS: usize = 3;
