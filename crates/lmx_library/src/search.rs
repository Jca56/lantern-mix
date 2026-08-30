//! Token index, numeric filters (BPM ±%, key compatibility), sort.
//!
//! MVP: case-folded token prefix matching over title/artist/album/file name,
//! plus sorting. The persistent index comes with the store.

use crate::model::{SortBy, Track};

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

/// Indices into `tracks` that match `query`, sorted by `by` (ascending unless
/// `desc`). Stable, so equal keys keep path order.
pub fn view(tracks: &[Track], query: &str, by: SortBy, desc: bool) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..tracks.len()).filter(|i| matches(&tracks[*i], query)).collect();
    idx.sort_by(|&a, &b| {
        let (ta, tb) = (&tracks[a], &tracks[b]);
        let ord = match by {
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
        Track { id: TrackId(title.len() as u64), title: title.into(), artist: artist.into(), bpm_tag: Some(bpm), ..Default::default() }
    }

    #[test]
    fn prefix_tokens_and_sorting() {
        let tracks = vec![t("Bangarang", "Skrillex", 110.0), t("Inferno", "Someone", 140.0), t("Rapture", "Alva", 150.0)];
        assert_eq!(view(&tracks, "skr bang", SortBy::Title, false), vec![0]);
        assert_eq!(view(&tracks, "zzz", SortBy::Title, false), Vec::<usize>::new());
        assert_eq!(view(&tracks, "", SortBy::Bpm, true), vec![2, 1, 0]);
        assert_eq!(view(&tracks, "", SortBy::Artist, false), vec![2, 0, 1]);
    }
}
