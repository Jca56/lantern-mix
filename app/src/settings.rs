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

/// Browser column widths (logical px). TITLE and ARTIST share the flexible
/// space by `title_frac`; the others are fixed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Columns {
    pub title_frac: f32,
    pub bpm: f32,
    pub key: f32,
    pub time: f32,
}

impl Default for Columns {
    fn default() -> Self {
        Self { title_frac: 0.6, bpm: 120.0, key: 100.0, time: 120.0 }
    }
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub wave_order: WaveOrder,
    /// Library folders, scanned at launch.
    pub roots: Vec<PathBuf>,
    pub columns: Columns,
}

impl Default for Settings {
    fn default() -> Self {
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
        Self { wave_order: WaveOrder::Deck3124, roots: vec![home.join("Music/DJ")], columns: Columns::default() }
    }
}

impl Settings {
    fn path() -> PathBuf {
        lmx_core::paths::config_dir().join("settings.conf")
    }

    pub fn load() -> Self {
        let mut s = Settings::default();
        let Ok(text) = std::fs::read_to_string(Self::path()) else { return s };
        let mut roots = Vec::new();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            match k.trim() {
                "wave_order" => {
                    if let Some(o) = WaveOrder::parse(v) {
                        s.wave_order = o;
                    }
                }
                "root" => roots.push(PathBuf::from(v.trim())),
                "columns" => {
                    let nums: Vec<f32> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                    if nums.len() == 4 {
                        s.columns = Columns { title_frac: nums[0].clamp(0.1, 0.9), bpm: nums[1].max(60.0), key: nums[2].max(60.0), time: nums[3].max(60.0) };
                    }
                }
                _ => {}
            }
        }
        if !roots.is_empty() {
            s.roots = roots;
        }
        s
    }

    pub fn save(&self) {
        let mut out = String::new();
        let _ = writeln!(out, "wave_order = {}", self.wave_order.key());
        for r in &self.roots {
            let _ = writeln!(out, "root = {}", r.display());
        }
        let c = self.columns;
        let _ = writeln!(out, "columns = {:.3},{:.0},{:.0},{:.0}", c.title_frac, c.bpm, c.key, c.time);
        let p = Self::path();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&p, out) {
            eprintln!("lantern-mix: could not save settings to {}: {e}", p.display());
        }
    }
}
