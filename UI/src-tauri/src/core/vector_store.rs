//! Custom flat vector store — a pure-Rust replacement for the Python FAISS
//! `IndexIDMap(IndexFlatIP(768))`.
//!
//! ## Several vectors per image (v5)
//! Each file contributes one entry per **region**: the whole frame, plus the
//! overlapping windows planned by [`crate::core::regions`].  A file is scored by
//! its single best-matching region rather than by its whole-frame vector, which
//! is what lets a small motif find the large composite design it appears inside.
//! See `core::regions` for why one vector per image cannot answer that question.
//!
//! Every entry also carries a colour histogram (`COLOR_DIM` dims) for its own
//! region.  Colour is stored for display and Browse filtering only — ranking is
//! pure design similarity (`COLOR_WEIGHT` is 0).
//!
//! ## Why not FAISS?
//! The FAISS C++ library requires a non-trivial build step and cannot be
//! sandboxed by macOS App Store.  For our workload (<=500 k images) a
//! brute-force dot-product over a rayon thread pool is fast enough.
//!
//! ## File format  (`vectors.bin`, version 5)
//! ```text
//! [ magic:      8 bytes  "PICTOR\x00\x01" ]
//! [ version:    4 bytes  u32 little-endian ]  == 5
//! [ design_dim: 4 bytes  u32 little-endian ]  always 1536  (DINOv2 CLS+patch)
//! [ count:      8 bytes  u64 little-endian ]  live (non-tombstone) entries
//! [ color_dim:  4 bytes  u32 little-endian ]  always COLOR_DIM
//! [ padding:    4 bytes                    ]  reserved, must be 0
//! [ entries: (
//!       id:     i64,
//!       region: f32 x 4   (x, y, w, h — normalised 0..1; whole frame = 0,0,1,1),
//!       design: f32 x design_dim,
//!       colour: f32 x color_dim
//!   )* ]
//! ```
//! Several entries share one `id` — they are regions of the same file.
//! Tombstoned entries have `id == i64::MIN`.
//! A version mismatch makes `load` start fresh, which the startup migration
//! turns into a one-time re-index (see `core::migrate`).

use crate::{
    config::EMB_DIM,
    core::{color::COLOR_DIM, regions::Region},
    error::{Result, PictoriaError},
};
use once_cell::sync::Lazy;
use rayon::prelude::*;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, OpenOptions},
    io::{BufWriter, Write},
    path::Path,
    sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

/// Serialises concurrent load → modify → save cycles so two sync workers
/// starting simultaneously cannot race to write stale data (lost-update).
/// Held by `sync_one` for its full duration.
static SYNC_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

/// Separates readers (search) from the brief file-write step (save).
/// Many read guards can be held at once; a write guard is exclusive and is
/// only held for the 1–2 seconds of `store.save()`.
static STORE_RW_LOCK: Lazy<RwLock<()>> = Lazy::new(|| RwLock::new(()));

/// Serialise concurrent `sync_one` runs.  Hold for the entire load → embed
/// → save sequence so two watcher threads never produce a lost update.
pub fn store_io_guard() -> MutexGuard<'static, ()> {
    SYNC_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Shared read lock for search.  Multiple searches can hold this at the same
/// time.  Only blocked for the ~2 seconds while a save is in progress.
pub fn store_io_read_guard() -> RwLockReadGuard<'static, ()> {
    STORE_RW_LOCK.read().unwrap_or_else(|e| e.into_inner())
}

/// Exclusive write lock held only during `store.save()`.  Prevents a search
/// from reading a half-written file while a save is in progress.
pub fn store_io_write_guard() -> RwLockWriteGuard<'static, ()> {
    STORE_RW_LOCK.write().unwrap_or_else(|e| e.into_inner())
}

const MAGIC:      &[u8; 8] = b"PICTOR\x00\x01";
const VERSION:    u32      = 5;
const HEADER_LEN: usize    = 32;
const TOMBSTONE:  i64      = i64::MIN;
/// id + region(x, y, w, h)
const REGION_F32: usize    = 4;

/// Search ranks purely on design pattern similarity (DINOv2 embedding).
/// Color is computed and stored for display purposes (color_match % in UI and
/// Browse page filtering) but does not affect result ordering.
pub const DESIGN_WEIGHT: f32 = 1.0;
pub const COLOR_WEIGHT:  f32 = 0.0;

/// One search hit: a file, and the region of it that matched best.
#[derive(Debug, Clone, Copy)]
pub struct Match {
    pub id:         i64,
    pub design_sim: f32,
    pub color_sim:  f32,
    /// Which part of the file matched.  `Region::WHOLE` means the full frame.
    pub region:     Region,
    /// Best score over this file's **whole-frame** entry alone.
    ///
    /// Reported separately because `design_sim` is a maximum over every region,
    /// and a maximum over many samples drifts upward on its own: measured on a
    /// real library, most files had some region edging past their whole frame by
    /// a few tenths of a point purely by chance.  Genuine containment looks
    /// different — the whole frame scores *poorly* while one region scores well.
    /// Callers need both numbers to tell those apart.
    pub whole_sim:  f32,
}

/// In-memory representation of the vector store.
pub struct VectorStore {
    ids:        Vec<i64>,
    /// Parallel to `ids`: which part of the file each entry describes.
    regions:    Vec<Region>,
    design:     Vec<f32>,
    color:      Vec<f32>,
    design_dim: usize,
    color_dim:  usize,
}

impl VectorStore {
    // ── Construction ──────────────────────────────────────────────────

    pub fn new() -> Self {
        Self {
            ids:        Vec::new(),
            regions:    Vec::new(),
            design:     Vec::new(),
            color:      Vec::new(),
            design_dim: EMB_DIM,
            color_dim:  COLOR_DIM,
        }
    }

    /// Load from disk, or create a fresh store if the file does not exist, is
    /// corrupt, or was written by an older format version.  Returning a fresh
    /// store on a version mismatch is intentional: the startup migration then
    /// re-indexes every folder against the current format.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }

        let bytes = fs::read(path)?;
        if bytes.len() < HEADER_LEN {
            log::warn!("vector store file too short — starting fresh");
            return Ok(Self::new());
        }

        if &bytes[..8] != MAGIC {
            log::warn!("vector store magic mismatch — starting fresh");
            return Ok(Self::new());
        }

        let version = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        if version != VERSION {
            log::warn!("vector store version {version} != {VERSION} — starting fresh (re-index)");
            return Ok(Self::new());
        }

        let design_dim = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let color_dim  = u32::from_le_bytes(bytes[24..28].try_into().unwrap()) as usize;
        let body = &bytes[HEADER_LEN..];
        let entry_bytes = 8 + REGION_F32 * 4 + design_dim * 4 + color_dim * 4;

        if design_dim == 0 || body.len() % entry_bytes != 0 {
            log::warn!("vector store body not aligned — starting fresh");
            return Ok(Self::new());
        }

        let n = body.len() / entry_bytes;
        let mut ids     = Vec::with_capacity(n);
        let mut regions = Vec::with_capacity(n);
        let mut design  = Vec::with_capacity(n * design_dim);
        let mut color   = Vec::with_capacity(n * color_dim);

        let read_f32 = |b: &[u8], at: usize| -> f32 {
            f32::from_le_bytes(b[at..at + 4].try_into().unwrap())
        };

        for i in 0..n {
            let mut off = i * entry_bytes;
            let id = i64::from_le_bytes(body[off..off + 8].try_into().unwrap());
            ids.push(id);
            off += 8;

            regions.push(Region {
                x: read_f32(body, off),
                y: read_f32(body, off + 4),
                w: read_f32(body, off + 8),
                h: read_f32(body, off + 12),
            });
            off += REGION_F32 * 4;

            for j in 0..design_dim {
                design.push(read_f32(body, off + j * 4));
            }
            off += design_dim * 4;
            for j in 0..color_dim {
                color.push(read_f32(body, off + j * 4));
            }
        }

        // Defensive de-duplication.  A corrupt or interrupted write can leave the
        // same entry stored more than once (which surfaces as duplicate search
        // results).  The key is (id, region) — NOT id alone, since one file
        // legitimately owns many entries now, one per region.  Keep only the last
        // occurrence of each and drop tombstones so the store self-heals on load
        // and is persisted clean on the next save.
        let live   = ids.iter().filter(|&&id| id != TOMBSTONE).count();
        let unique = ids
            .iter()
            .zip(regions.iter())
            .filter(|(&id, _)| id != TOMBSTONE)
            .map(|(&id, r)| entry_key(id, r))
            .collect::<HashSet<_>>()
            .len();

        if unique != live || live != ids.len() {
            let mut last: HashMap<(i64, [u32; 4]), usize> = HashMap::with_capacity(unique);
            for (i, (&id, r)) in ids.iter().zip(regions.iter()).enumerate() {
                if id != TOMBSTONE {
                    last.insert(entry_key(id, r), i);
                }
            }
            let mut keep: Vec<usize> = last.into_values().collect();
            keep.sort_unstable();

            let mut nids     = Vec::with_capacity(keep.len());
            let mut nregions = Vec::with_capacity(keep.len());
            let mut ndesign  = Vec::with_capacity(keep.len() * design_dim);
            let mut ncolor   = Vec::with_capacity(keep.len() * color_dim);
            for &i in &keep {
                nids.push(ids[i]);
                nregions.push(regions[i]);
                ndesign.extend_from_slice(&design[i * design_dim..(i + 1) * design_dim]);
                ncolor.extend_from_slice(&color[i * color_dim..(i + 1) * color_dim]);
            }
            log::warn!(
                "vector store: {} raw entries deduped to {} unique on load",
                ids.len(), nids.len()
            );
            ids = nids; regions = nregions; design = ndesign; color = ncolor;
        }

        Ok(Self { ids, regions, design, color, design_dim, color_dim })
    }

    /// Persist to disk atomically (write to `.tmp`, then rename).
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let tmp = path.with_extension("bin.tmp");
        {
            let file = OpenOptions::new()
                .write(true).create(true).truncate(true)
                .open(&tmp)?;
            let mut w = BufWriter::new(file);

            let live_count =
                self.ids.iter().filter(|&&id| id != TOMBSTONE).count() as u64;

            w.write_all(MAGIC)?;
            w.write_all(&VERSION.to_le_bytes())?;
            w.write_all(&(self.design_dim as u32).to_le_bytes())?;
            w.write_all(&live_count.to_le_bytes())?;
            w.write_all(&(self.color_dim as u32).to_le_bytes())?;
            w.write_all(&[0u8; 4])?;

            let entry_bytes = 8 + REGION_F32 * 4 + self.design_dim * 4 + self.color_dim * 4;
            let mut buf = vec![0u8; entry_bytes];

            for (idx, &id) in self.ids.iter().enumerate() {
                buf[..8].copy_from_slice(&id.to_le_bytes());
                let mut o = 8;

                let r = self.regions[idx];
                for v in [r.x, r.y, r.w, r.h] {
                    buf[o..o + 4].copy_from_slice(&v.to_le_bytes());
                    o += 4;
                }

                let d = &self.design[idx * self.design_dim..(idx + 1) * self.design_dim];
                for &v in d {
                    buf[o..o + 4].copy_from_slice(&v.to_le_bytes());
                    o += 4;
                }
                let c = &self.color[idx * self.color_dim..(idx + 1) * self.color_dim];
                for &v in c {
                    buf[o..o + 4].copy_from_slice(&v.to_le_bytes());
                    o += 4;
                }
                w.write_all(&buf)?;
            }
            w.flush()?;
        }

        fs::rename(&tmp, path)?;
        Ok(())
    }

    // ── Mutation ──────────────────────────────────────────────────────

    /// Append one region of one file.  `design` must be `EMB_DIM` long and
    /// `color` `COLOR_DIM` long; both should already be L2-normalised.
    ///
    /// Called once per region, so the same `id` is added several times — that is
    /// expected, not a duplicate.
    pub fn add(&mut self, id: i64, region: Region, design: &[f32], color: &[f32]) -> Result<()> {
        if design.len() != self.design_dim {
            return Err(PictoriaError::Fatal(format!(
                "design dim mismatch: expected {}, got {}",
                self.design_dim, design.len()
            )));
        }
        if color.len() != self.color_dim {
            return Err(PictoriaError::Fatal(format!(
                "colour dim mismatch: expected {}, got {}",
                self.color_dim, color.len()
            )));
        }
        self.ids.push(id);
        self.regions.push(region);
        self.design.extend_from_slice(design);
        self.color.extend_from_slice(color);
        Ok(())
    }

    /// Tombstone **every** entry belonging to the given file IDs — a file owns
    /// one entry per region, so removing by id has to sweep all of them or the
    /// orphaned regions keep turning up in search results.
    /// Compacts automatically when tombstones > 20 %.
    pub fn remove(&mut self, ids: &[i64]) {
        if ids.is_empty() { return; }
        let id_set: HashSet<i64> = ids.iter().copied().collect();
        for slot in self.ids.iter_mut() {
            if id_set.contains(slot) {
                *slot = TOMBSTONE;
            }
        }
        self.maybe_compact();
    }

    // ── Query ─────────────────────────────────────────────────────────

    /// Return the top-`k` **files** by best-matching region.
    ///
    /// Every region of every file is scored, then collapsed to one hit per file
    /// keeping its highest-scoring region (max-pooling).  Collapsing before
    /// truncation is essential: a large composite owns many regions, and taking
    /// the top-`k` *entries* first would let a single file fill the whole result
    /// set while other files fell off the end.
    ///
    /// `query_designs` holds one or more query vectors — the reference image as a
    /// whole plus, for an elongated reference, windows along its long axis (see
    /// [`crate::core::regions::plan_query`]).  An entry scores as its best match
    /// against any of them.
    ///
    /// All vectors must be L2-normalised.  Callers receive the individual
    /// sub-scores so the UI can display pattern match % and color match %
    /// separately; blending for ranking is done internally.
    pub fn search(&self, query_designs: &[Vec<f32>], query_color: &[f32], k: usize) -> Vec<Match> {
        if self.ids.is_empty() || k == 0 || query_designs.is_empty() {
            return Vec::new();
        }

        let use_color = query_color.len() == self.color_dim;

        // Score every live region.  This is the expensive part, so it stays a
        // flat parallel scan; the grouping below is cheap by comparison.
        let scored: Vec<(i64, f32, f32, f32, Region)> = self
            .ids
            .par_iter()
            .enumerate()
            .filter_map(|(i, &id)| {
                if id == TOMBSTONE { return None; }

                let d = &self.design[i * self.design_dim..(i + 1) * self.design_dim];
                let design_sim: f32 = query_designs
                    .iter()
                    .map(|q| q.iter().zip(d).map(|(a, b)| a * b).sum::<f32>())
                    .fold(f32::NEG_INFINITY, f32::max);

                let color_sim: f32 = if use_color {
                    let c = &self.color[i * self.color_dim..(i + 1) * self.color_dim];
                    query_color.iter().zip(c).map(|(a, b)| a * b).sum()
                } else {
                    0.0
                };

                let blended = DESIGN_WEIGHT * design_sim + COLOR_WEIGHT * color_sim;
                Some((id, blended, design_sim, color_sim, self.regions[i]))
            })
            .collect();

        // Collapse to the best region per file, tracking the whole-frame score
        // alongside it so callers can judge whether a region genuinely won.
        let mut best: HashMap<i64, (f32, Match)> = HashMap::new();
        for (id, blended, design_sim, color_sim, region) in scored {
            let m = Match { id, design_sim, color_sim, region, whole_sim: f32::NEG_INFINITY };
            let entry = best.entry(id).or_insert((f32::NEG_INFINITY, m));
            if blended > entry.0 {
                let carried = entry.1.whole_sim;
                *entry = (blended, Match { whole_sim: carried, ..m });
            }
            if region.is_whole() && design_sim > entry.1.whole_sim {
                entry.1.whole_sim = design_sim;
            }
        }

        let mut hits: Vec<(f32, Match)> = best.into_values().collect();
        hits.sort_unstable_by(|a, b| {
            b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal)
        });
        hits.truncate(k);
        hits.into_iter().map(|(_, m)| m).collect()
    }

    /// Number of live entries (regions), not files.
    pub fn live_count(&self) -> usize {
        self.ids.iter().filter(|&&id| id != TOMBSTONE).count()
    }

    // ── Compaction ────────────────────────────────────────────────────

    fn maybe_compact(&mut self) {
        let total      = self.ids.len();
        let tombstones = self.ids.iter().filter(|&&id| id == TOMBSTONE).count();
        if total == 0 || tombstones * 5 < total { return; }

        let keep = total - tombstones;
        let mut new_ids     = Vec::with_capacity(keep);
        let mut new_regions = Vec::with_capacity(keep);
        let mut new_design  = Vec::with_capacity(keep * self.design_dim);
        let mut new_color   = Vec::with_capacity(keep * self.color_dim);

        for (i, &id) in self.ids.iter().enumerate() {
            if id != TOMBSTONE {
                new_ids.push(id);
                new_regions.push(self.regions[i]);
                new_design.extend_from_slice(
                    &self.design[i * self.design_dim..(i + 1) * self.design_dim],
                );
                new_color.extend_from_slice(
                    &self.color[i * self.color_dim..(i + 1) * self.color_dim],
                );
            }
        }

        self.ids     = new_ids;
        self.regions = new_regions;
        self.design  = new_design;
        self.color   = new_color;
        log::debug!("vector store compacted: {} -> {} entries", total, self.ids.len());
    }
}

/// Hashable identity of an entry.  `f32` is not `Hash`/`Eq`, and the values are
/// round-tripped verbatim through the file, so comparing raw bits is exact.
fn entry_key(id: i64, r: &Region) -> (i64, [u32; 4]) {
    (id, [r.x.to_bits(), r.y.to_bits(), r.w.to_bits(), r.h.to_bits()])
}

impl Default for VectorStore {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(dim: usize, seed: f32) -> Vec<f32> {
        let mut v: Vec<f32> = (0..dim).map(|i| ((i as f32) * seed).sin()).collect();
        let n = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
        for x in v.iter_mut() { *x /= n; }
        v
    }

    fn region(x: f32, y: f32) -> Region {
        Region { x, y, w: 0.5, h: 0.5 }
    }

    #[test]
    fn round_trips_many_regions_per_file() {
        let dir = std::env::temp_dir().join(format!("vs_test_{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("vectors.bin");

        let mut s = VectorStore::new();
        let color = vec![0.0; COLOR_DIM];
        s.add(7, Region::WHOLE, &unit(EMB_DIM, 0.1), &color).unwrap();
        s.add(7, region(0.0, 0.0), &unit(EMB_DIM, 0.2), &color).unwrap();
        s.add(7, region(0.5, 0.5), &unit(EMB_DIM, 0.3), &color).unwrap();
        s.save(&path).unwrap();

        let back = VectorStore::load(&path).unwrap();
        assert_eq!(back.live_count(), 3, "all regions of a file must survive");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn remove_sweeps_every_region_of_a_file() {
        let mut s = VectorStore::new();
        let color = vec![0.0; COLOR_DIM];
        s.add(1, Region::WHOLE, &unit(EMB_DIM, 0.1), &color).unwrap();
        s.add(1, region(0.0, 0.0), &unit(EMB_DIM, 0.2), &color).unwrap();
        s.add(2, Region::WHOLE, &unit(EMB_DIM, 0.3), &color).unwrap();

        s.remove(&[1]);
        assert_eq!(s.live_count(), 1);
        assert!(s.ids.iter().filter(|&&i| i != TOMBSTONE).all(|&i| i == 2));
    }

    #[test]
    fn search_returns_each_file_once_with_its_best_region() {
        let mut s = VectorStore::new();
        let color = vec![0.0; COLOR_DIM];
        let target = unit(EMB_DIM, 0.7);

        // File 1: a poor whole-frame vector but one region that matches exactly.
        s.add(1, Region::WHOLE, &unit(EMB_DIM, 0.1), &color).unwrap();
        s.add(1, region(0.5, 0.5), &target, &color).unwrap();
        // File 2: mediocre everywhere.
        s.add(2, Region::WHOLE, &unit(EMB_DIM, 0.2), &color).unwrap();

        let hits = s.search(std::slice::from_ref(&target), &[], 10);
        assert_eq!(hits.len(), 2, "one hit per file, not per region");
        assert_eq!(hits[0].id, 1, "the file with the matching region ranks first");
        assert!(!hits[0].region.is_whole(), "the matching region is reported");
        assert!(hits[0].design_sim > 0.99);
    }

    /// `whole_sim` must survive a later region overtaking the whole frame — the
    /// carry logic in the collapse loop is easy to get wrong, and if it reports
    /// NEG_INFINITY the caller sees an infinite margin and badges everything.
    #[test]
    fn whole_frame_score_is_reported_alongside_the_winning_region() {
        let color = vec![0.0; COLOR_DIM];
        let target = unit(EMB_DIM, 0.7);
        let mediocre = unit(EMB_DIM, 0.11);

        // Whole frame added FIRST, then a region that beats it.
        let mut a = VectorStore::new();
        a.add(1, Region::WHOLE, &mediocre, &color).unwrap();
        a.add(1, region(0.5, 0.5), &target, &color).unwrap();

        // Same file, entries in the opposite order.
        let mut b = VectorStore::new();
        b.add(1, region(0.5, 0.5), &target, &color).unwrap();
        b.add(1, Region::WHOLE, &mediocre, &color).unwrap();

        for (store, label) in [(&a, "whole first"), (&b, "region first")] {
            let hit = store.search(std::slice::from_ref(&target), &[], 5)[0];
            assert!(!hit.region.is_whole(), "{label}: region should win");
            assert!(hit.design_sim > 0.99, "{label}: winning score");
            assert!(
                hit.whole_sim.is_finite() && (hit.whole_sim - mediocre.iter().zip(&target).map(|(x, y)| x * y).sum::<f32>()).abs() < 1e-5,
                "{label}: whole_sim must report the whole frame's own score, got {}",
                hit.whole_sim
            );
        }
    }

    #[test]
    fn one_file_cannot_crowd_out_the_result_set() {
        let mut s = VectorStore::new();
        let color = vec![0.0; COLOR_DIM];
        let target = unit(EMB_DIM, 0.7);

        // A composite with many decent regions, plus one other file.
        for i in 0..12 {
            s.add(1, region(i as f32 * 0.01, 0.0), &target, &color).unwrap();
        }
        s.add(2, Region::WHOLE, &unit(EMB_DIM, 0.5), &color).unwrap();

        let hits = s.search(std::slice::from_ref(&target), &[], 2);
        assert_eq!(hits.len(), 2);
        assert_ne!(hits[0].id, hits[1].id, "the same file must not appear twice");
    }
}
