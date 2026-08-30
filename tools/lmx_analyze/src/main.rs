//! CLI: analyze a file, print tempo candidates, grid, key, loudness; dump
//! ODF/chroma as CSV. For tuning the detectors on real tracks.
//!
//! Phase 0: probe + decode timing only (the analyzers don't exist yet).

use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: lmx_analyze <file>...");
        std::process::exit(2);
    }
    let mut failed = 0;
    for a in &args {
        let path = PathBuf::from(a);
        println!("── {}", path.display());
        match lmx_codec::probe(&path) {
            Ok(p) => {
                println!(
                    "   {:?}  {} Hz  {} ch  {} bit  {:.3} s ({} frames)",
                    p.format, p.sample_rate, p.channels, p.bits, p.duration_secs(), p.duration_frames
                );
                let m = &p.metadata;
                let field = |k: &str, v: &Option<String>| {
                    if let Some(v) = v {
                        println!("   {k:<8} {v}");
                    }
                };
                field("title", &m.title);
                field("artist", &m.artist);
                field("album", &m.album);
                field("genre", &m.genre);
                field("label", &m.label);
                field("comment", &m.comment);
                field("key", &m.key_tag);
                if let Some(y) = m.year {
                    println!("   year     {y}");
                }
                if let Some(b) = m.bpm_tag {
                    println!("   bpm tag  {b}");
                }
                if let Some(art) = &m.artwork {
                    println!("   artwork  {} bytes", art.len());
                }
            }
            Err(e) => {
                println!("   probe failed: {e}");
                failed += 1;
                continue;
            }
        }
        let t0 = Instant::now();
        match lmx_codec::decode_all(&path, |_| {}) {
            Ok((audio, _)) => {
                let dt = t0.elapsed().as_secs_f64();
                let peak = audio.frames.iter().fold(0.0f32, |m, s| m.max(s.abs()));
                let peak_db = if peak > 0.0 { 20.0 * peak.log10() } else { -120.0 };
                println!(
                    "   decoded {} frames in {:.3} s ({:.0}× realtime), peak {:.2} dBFS, {:.1} MB",
                    audio.frame_count(),
                    dt,
                    audio.duration_secs() / dt.max(1e-9),
                    peak_db,
                    (audio.frames.len() * 4) as f64 / 1e6
                );
            }
            Err(e) => {
                println!("   decode failed: {e}");
                failed += 1;
            }
        }
    }
    if failed > 0 {
        std::process::exit(1);
    }
}
