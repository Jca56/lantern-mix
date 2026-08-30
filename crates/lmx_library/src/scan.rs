//! Root folder walking, identification, moved-file matching.
//!
//! The walker only lists candidate files; probing them is the caller's job
//! (the codec crate lives above this one).

use std::path::{Path, PathBuf};

pub const AUDIO_EXTENSIONS: [&str; 6] = ["wav", "wave", "aif", "aiff", "flac", "mp3"];

fn is_audio(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(e)))
        .unwrap_or(false)
}

/// Every audio file under `root`, recursively, sorted by path. Hidden
/// directories are skipped; unreadable ones are ignored.
pub fn walk_audio_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name();
            if name.to_string_lossy().starts_with('.') {
                continue;
            }
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(p),
                Ok(t) if t.is_file() && is_audio(&p) => out.push(p),
                _ => {}
            }
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_recursively_and_filters() {
        let dir = std::env::temp_dir().join(format!("lmx_scan_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub/.hidden")).unwrap();
        for f in ["a.wav", "b.FLAC", "c.txt", "sub/d.aiff", "sub/.hidden/e.wav"] {
            std::fs::write(dir.join(f), b"x").unwrap();
        }
        let files: Vec<String> = walk_audio_files(&dir)
            .iter()
            .map(|p| p.strip_prefix(&dir).unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(files, vec!["a.wav", "b.FLAC", "sub/d.aiff"]);
        std::fs::remove_dir_all(&dir).ok();
    }
}
