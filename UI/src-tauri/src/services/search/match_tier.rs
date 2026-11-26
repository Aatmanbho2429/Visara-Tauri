// Exact-match tiers: pHash classification and the one result ordering the backend and the UI share.

use crate::{
    config::{EXACT_COLOR_MIN_SIM, EXACT_HASH_MAX_DIST, MATCH_COPY_MIN_INLIERS, MATCH_COPY_MIN_RATIO},
    models::response::SearchResult,
};
use std::cmp::Ordering;

// What the exact-match engine decided about one file; the order here is the display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchKind {
    Exact,
    ExactTransformed,
    Colourway,
    Copy,
    None,
}

impl MatchKind {
    // Wire string carried in `SearchResult::match_kind`.
    pub fn as_str(self) -> &'static str {
        match self {
            MatchKind::Exact => "exact",
            MatchKind::ExactTransformed => "exactTransformed",
            MatchKind::Colourway => "colourway",
            MatchKind::Copy => "copy",
            MatchKind::None => "none",
        }
    }

    // Parses the wire string; anything unknown reads as `None` so an old row can never outrank a real match.
    pub fn from_wire(s: &str) -> MatchKind {
        match s {
            "exact" => MatchKind::Exact,
            "exactTransformed" => MatchKind::ExactTransformed,
            "colourway" => MatchKind::Colourway,
            "copy" => MatchKind::Copy,
            _ => MatchKind::None,
        }
    }

    // 0 = best tier; the sort key before any within-tier ordering.
    fn tier(self) -> u8 {
        match self {
            MatchKind::Exact => 0,
            MatchKind::ExactTransformed => 1,
            MatchKind::Colourway => 2,
            MatchKind::Copy => 3,
            MatchKind::None => 4,
        }
    }

    // True for the three pHash-decided tiers, which order by hash distance.
    fn is_hash_tier(self) -> bool {
        matches!(self, MatchKind::Exact | MatchKind::ExactTransformed | MatchKind::Colourway)
    }
}

// Smallest Hamming distance between `hash` and any query variant, with the index of the variant that won (first wins ties).
pub fn min_hamming(variants: &[u64], hash: u64) -> Option<(u32, usize)> {
    variants
        .iter()
        .enumerate()
        .map(|(i, v)| ((v ^ hash).count_ones(), i))
        .min_by_key(|&(d, i)| (d, i))
}

// pHash verdict: close enough in hash and colour is exact (transformed if a rotated/mirrored variant won), close in hash but not colour is a colourway.
pub fn classify_hash(dist: u32, variant: usize, color_sim: f32) -> MatchKind {
    if dist > EXACT_HASH_MAX_DIST {
        MatchKind::None
    } else if color_sim >= EXACT_COLOR_MIN_SIM {
        if variant == 0 { MatchKind::Exact } else { MatchKind::ExactTransformed }
    } else {
        MatchKind::Colourway
    }
}

// A verified match is a "copy" when its inlier count and ratio both clear the engine's bar.
pub fn is_copy(matched: bool, inliers: u32, inlier_ratio: f32) -> bool {
    matched && inliers >= MATCH_COPY_MIN_INLIERS && inlier_ratio >= MATCH_COPY_MIN_RATIO
}

// The pre-engine ordering: verified rows first, then similarity descending (ties keep their incoming order).
pub fn legacy_compare(a: &SearchResult, b: &SearchResult) -> Ordering {
    b.verified.cmp(&a.verified).then_with(|| b.similarity.partial_cmp(&a.similarity).unwrap_or(Ordering::Equal))
}

// The tiered ordering used in primary mode (exact > transformed > colourway > copy > legacy order), path ascending as the last tie-break.
pub fn compare_results(a: &SearchResult, b: &SearchResult) -> Ordering {
    let (ka, kb) = (MatchKind::from_wire(&a.match_kind), MatchKind::from_wire(&b.match_kind));
    ka.tier()
        .cmp(&kb.tier())
        .then_with(|| match ka {
            k if k.is_hash_tier() => a
                .hash_distance
                .unwrap_or(u32::MAX)
                .cmp(&b.hash_distance.unwrap_or(u32::MAX))
                .then_with(|| b.similarity.partial_cmp(&a.similarity).unwrap_or(Ordering::Equal)),
            MatchKind::Copy => b.match_points.cmp(&a.match_points),
            _ => legacy_compare(a, b),
        })
        .then_with(|| a.path.cmp(&b.path))
}

// Sets `rank` (1-based) on every row from the ordering `cmp`, without reordering the slice.
pub fn assign_ranks(results: &mut [SearchResult], cmp: impl Fn(&SearchResult, &SearchResult) -> Ordering) {
    let mut order: Vec<usize> = (0..results.len()).collect();
    order.sort_by(|&i, &j| cmp(&results[i], &results[j]));
    for (pos, &i) in order.iter().enumerate() {
        results[i].rank = pos + 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::response::MatchRegion;

    fn row(path: &str, kind: &str, dist: Option<u32>, sim: f32, verified: bool, points: u32) -> SearchResult {
        SearchResult {
            rank: 0,
            path: path.into(),
            name: path.into(),
            similarity: sim,
            pattern_match: sim,
            color_match: 0.0,
            folder: String::new(),
            verified,
            verification: if verified { "verified" } else { "unchecked" }.into(),
            partial: false,
            match_region: MatchRegion { x: 0.0, y: 0.0, w: 1.0, h: 1.0 },
            match_points: points,
            mirrored: false,
            match_kind: kind.into(),
            hash_distance: dist,
        }
    }

    // The minimum is taken over every variant and the winning index is reported (first wins ties).
    #[test]
    fn min_hamming_picks_the_best_variant() {
        let h = 0xF0F0_F0F0_F0F0_F0F0u64;
        let variants = [!h, h ^ 0b111, h ^ 0b1, h ^ 0b1, !h, !h, !h, !h];
        assert_eq!(min_hamming(&variants, h), Some((1, 2)));
        assert_eq!(min_hamming(&[], h), None);
        assert_eq!(min_hamming(&[u64::MAX], 0), Some((64, 0)));
    }

    // The distance and colour thresholds decide exact vs colourway vs none; a non-zero variant means transformed.
    #[test]
    fn classify_hash_applies_both_thresholds() {
        assert_eq!(classify_hash(0, 0, 0.99), MatchKind::Exact);
        assert_eq!(classify_hash(EXACT_HASH_MAX_DIST, 0, EXACT_COLOR_MIN_SIM), MatchKind::Exact);
        assert_eq!(classify_hash(2, 3, 0.99), MatchKind::ExactTransformed);
        assert_eq!(classify_hash(2, 5, 0.5), MatchKind::Colourway);
        assert_eq!(classify_hash(EXACT_HASH_MAX_DIST + 1, 0, 1.0), MatchKind::None);
    }

    // Copy needs a match plus both the inlier floor and the ratio floor.
    #[test]
    fn is_copy_needs_inliers_and_ratio() {
        assert!(is_copy(true, MATCH_COPY_MIN_INLIERS, MATCH_COPY_MIN_RATIO));
        assert!(!is_copy(true, MATCH_COPY_MIN_INLIERS - 1, 1.0));
        assert!(!is_copy(true, 500, MATCH_COPY_MIN_RATIO - 0.01));
        assert!(!is_copy(false, 500, 1.0));
    }

    // Tiers rank exact < transformed < colourway < copy < the rest, with the documented within-tier orders.
    #[test]
    fn compare_results_orders_tiers() {
        let mut rows = vec![
            row("z_none_verified", "none", None, 71.0, true, 12),
            row("copy_30", "copy", None, 80.0, true, 30),
            row("copy_90", "copy", None, 75.0, true, 90),
            row("colour_d4", "colourway", Some(4), 99.0, false, 0),
            row("colour_d1", "colourway", Some(1), 90.0, false, 0),
            row("trans", "exactTransformed", Some(0), 90.0, false, 0),
            row("exact_b", "exact", Some(2), 97.0, false, 0),
            row("exact_a", "exact", Some(2), 99.0, false, 0),
            row("exact_d0", "exact", Some(0), 60.0, false, 0),
            row("none_sim95", "none", None, 95.0, false, 0),
        ];
        rows.sort_by(compare_results);
        let order: Vec<&str> = rows.iter().map(|r| r.path.as_str()).collect();
        assert_eq!(
            order,
            ["exact_d0", "exact_a", "exact_b", "trans", "colour_d1", "colour_d4", "copy_90", "copy_30", "z_none_verified", "none_sim95"]
        );
    }

    // Equal in every key falls back to the path, so the order is deterministic.
    #[test]
    fn compare_results_breaks_final_ties_by_path() {
        let a = row("a.png", "none", None, 80.0, false, 0);
        let b = row("b.png", "none", None, 80.0, false, 0);
        assert_eq!(compare_results(&b, &a), Ordering::Greater);
    }

    // With every row "none" the tier sort is exactly the legacy order (verified first, then similarity), modulo the path tie-break.
    #[test]
    fn all_none_rows_sort_like_the_legacy_comparator() {
        let mut rows = Vec::new();
        for i in 0..60u32 {
            let sim = 50.0 + ((i * 37) % 11) as f32;
            rows.push(row(&format!("f{i:02}.png"), "none", None, sim, i % 5 == 0, if i % 5 == 0 { 12 + i } else { 0 }));
        }
        let mut legacy = rows.clone();
        legacy.sort_by(|a, b| legacy_compare(a, b).then_with(|| a.path.cmp(&b.path)));
        let mut tiered = rows.clone();
        tiered.sort_by(compare_results);
        let l: Vec<&str> = legacy.iter().map(|r| r.path.as_str()).collect();
        let t: Vec<&str> = tiered.iter().map(|r| r.path.as_str()).collect();
        assert_eq!(l, t);
    }

    // Ranks are 1-based, follow the comparator, and leave the slice order alone.
    #[test]
    fn assign_ranks_ranks_without_reordering() {
        let mut rows = vec![row("a", "none", None, 10.0, false, 0), row("b", "exact", Some(0), 10.0, false, 0)];
        assign_ranks(&mut rows, compare_results);
        assert_eq!(rows[0].path, "a");
        assert_eq!((rows[0].rank, rows[1].rank), (2, 1));
    }
}
