//! Token index, numeric filters (BPM ±%, key compatibility), sort.
//!
//! MVP: case-folded token prefix matching over title/artist/album/file name,
//! plus sorting. The persistent index comes with the store.

use crate::model::{Library, PlaylistId, SortBy, Track};

fn fold(s: &str) -> String {
    s.to_lowercase()
}

/// Every token of `query` must prefix-match some token of the track's text.
/// "skr bang" matches "Skrillex — Bangarang".
pub fn matches(t: &Track, query: &str) -> bool {
    let q = fold(query);
    let terms: Vec<&str> = q.split_whitespace().collect();
    if terms.is_empty() {
        return true;
    }
    let hay = fold(&format!(
        "{} {} {} {}",
        t.title,
        t.artist,
        t.album,
        t.path.file_stem().and_then(|s| s.to_str()).unwrap_or("")
    ));
    let words: Vec<&str> = hay.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    terms.iter().all(|term| words.iter().any(|w| w.starts_with(term)))
}

/// Indices into `lib.tracks` that match `query`, sorted by `by` (ascending
/// unless `desc`). `Manual` follows the library's own order.
pub fn view(lib: &Library, query: &str, by: SortBy, desc: bool) -> Vec<usize> {
    let idx: Vec<usize> = (0..lib.tracks.len()).collect();
    sort_indices(lib, idx, &lib.positions(), query, by, desc)
}

/// Same, restricted to a playlist; `Manual` is the playlist's own order.
pub fn playlist_view(lib: &Library, pl: PlaylistId, query: &str, by: SortBy, desc: bool) -> Vec<usize> {
    let Some(p) = lib.playlist(pl) else { return Vec::new() };
    let pos: std::collections::HashMap<_, _> = p.tracks.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    let idx: Vec<usize> = p.tracks.iter().filter_map(|id| lib.tracks.iter().position(|t| t.id == *id)).collect();
    sort_indices(lib, idx, &pos, query, by, desc)
}

fn sort_indices(lib: &Library, idx: Vec<usize>, pos: &std::collections::HashMap<crate::model::TrackId, usize>, query: &str, by: SortBy, desc: bool) -> Vec<usize> {
    let tracks = &lib.tracks;
    let mut idx: Vec<usize> = idx.into_iter().filter(|i| matches(&tracks[*i], query)).collect();
    idx.sort_by(|&a, &b| {
        let (ta, tb) = (&tracks[a], &tracks[b]);
        let ord = match by {
            SortBy::Manual => pos.get(&ta.id).cmp(&pos.get(&tb.id)),
            SortBy::Title => fold(ta.display_title()).cmp(&fold(tb.display_title())),
            SortBy::Artist => fold(&ta.artist).cmp(&fold(&tb.artist)),
            SortBy::Bpm => ta.bpm().unwrap_or(0.0).partial_cmp(&tb.bpm().unwrap_or(0.0)).unwrap_or(std::cmp::Ordering::Equal),
            SortBy::Key => ta.key_tag.clone().unwrap_or_default().cmp(&tb.key_tag.clone().unwrap_or_default()),
            SortBy::Time => ta.duration_secs.partial_cmp(&tb.duration_secs).unwrap_or(std::cmp::Ordering::Equal),
        };
        if desc { ord.reverse() } else { ord }
    });
    idx
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TrackId;

    fn t(title: &str, artist: &str, bpm: f32) -> Track {
        Track { id: TrackId(lmx_core::hash::hash64(title.as_bytes())), title: title.into(), artist: artist.into(), bpm_tag: Some(bpm), ..Default::default() }
    }

    #[test]
    fn prefix_tokens_and_sorting() {
        let mut lib = Library::default();
        for tr in [t("Bangarang", "Skrillex", 110.0), t("Inferno", "Someone", 140.0), t("Rapture", "Alva", 150.0)] {
            lib.upsert(tr);
        }
        assert_eq!(view(&lib, "skr bang", SortBy::Title, false), vec![0]);
        assert_eq!(view(&lib, "zzz", SortBy::Title, false), Vec::<usize>::new());
        assert_eq!(view(&lib, "", SortBy::Bpm, true), vec![2, 1, 0]);
        assert_eq!(view(&lib, "", SortBy::Artist, false), vec![2, 0, 1]);
        lib.move_before(lib.tracks[2].id, Some(lib.tracks[0].id));
        assert_eq!(view(&lib, "", SortBy::Manual, false), vec![2, 0, 1]);
    }
}
