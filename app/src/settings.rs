//! User settings load/save: a plain `key = value` file in the config dir.

use std::fmt::Write as _;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaveOrder {
    Deck1234,
    Deck3124,
}

impl WaveOrder {
    /// Deck indices top to bottom.
    pub fn decks(self) -> [usize; 4] {
        match self {
            WaveOrder::Deck1234 => [0, 1, 2, 3],
            WaveOrder::Deck3124 => [2, 0, 1, 3],
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            WaveOrder::Deck1234 => "WAVEFORMS  1 · 2 · 3 · 4",
            WaveOrder::Deck3124 => "WAVEFORMS  3 · 1 · 2 · 4",
        }
    }
    fn key(self) -> &'static str {
        match self {
            WaveOrder::Deck1234 => "1234",
            WaveOrder::Deck3124 => "3124",
        }
    }
    fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "1234" => Some(WaveOrder::Deck1234),
            "3124" => Some(WaveOrder::Deck3124),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub wave_order: WaveOrder,
}

impl Default for Settings {
    fn default() -> Self {
        Self { wave_order: WaveOrder::Deck3124 }
    }
}

impl Settings {
    fn path() -> PathBuf {
        lmx_core::paths::config_dir().join("settings.conf")
    }

    pub fn load() -> Self {
        let mut s = Settings::default();
        let Ok(text) = std::fs::read_to_string(Self::path()) else { return s };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            match k.trim() {
                "wave_order" => {
                    if let Some(o) = WaveOrder::parse(v) {
                        s.wave_order = o;
                    }
                }
                _ => {}
            }
        }
        s
    }

    pub fn save(&self) {
        let mut out = String::new();
        let _ = writeln!(out, "wave_order = {}", self.wave_order.key());
        let p = Self::path();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&p, out) {
            eprintln!("lantern-mix: could not save settings to {}: {e}", p.display());
        }
    }
}
