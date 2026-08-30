//! Immediate-mode widget kit: ids, widget memory, input, layout, theme (big text),
//! DJ widgets. Headless-testable; draws through lmx_gpu.
//!
//! Every frame the app builds a `UiFrame` (borrowing the painter, the text
//! renderer and this frame's input) and calls widget methods on it. Widgets draw
//! immediately and return what happened. The only state that survives a frame is
//! `Ui`: hot/active ids and a small per-widget memory map.
//!
//! Design: `docs/05-UI.md`.
#![forbid(unsafe_code)]

pub mod deck;
pub mod id;
pub mod input;
pub mod layout;
pub mod table;
pub mod theme;
pub mod ui;
pub mod waveform;
pub mod widgets;

pub use id::Id;
pub use input::{Input, Key, MouseButton};
pub use lmx_gpu::{Color, Gradient, Painter, Rect, Text, Vec2};
pub use theme::Theme;
pub use ui::{Interaction, Ui, UiFrame};
