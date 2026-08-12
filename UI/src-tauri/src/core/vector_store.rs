//! Custom flat vector store for the Gabor-rose + Gram-matrix design
//! descriptor, plus a colour histogram per file.
//!
//! ## v6: one entry per file, not per region
//! The old DINOv2 scheme stored several vectors per file (whole frame plus
//! sliced windows) because a single 224x224 embedding couldn't otherwise
//! answer "does this design appear *inside* that one". SIFT/RANSAC
//! (`core::sidecar::verify`) answers that question directly against the full
//! image instead, so there's nothing left to slice for — one descriptor per
//! file, looked up by id.
//!
//! ## Why not FAISS
//! The FAISS C++ library needs a non-trivial build step and can't be
//! sandboxed by the macOS App Store. Brute-force cosine over a rayon thread
//! pool is fast enough for our workload (<=500k images).
//!
//! ## File format (`vectors.bin`, version 6)
//! ```text
//! [ magic:      8 bytes  "PICTOR\x00\x01" ]
//! [ version:    4 bytes  u32 LE ]  == 6
//! [ rose_dim:   4 bytes  u32 LE ]  == ROSE_DIM
//! [ count:      8 bytes  u64 LE ]  live (non-tombstone) entries
//! [ gram_dim:   4 bytes  u32 LE ]  == GRAM_ZOOM_LEVELS * GRAM_DIM_PER_ZOOM
//! [ color_dim:  4 bytes  u32 LE ]  == COLOR_DIM
//! [ entries: ( id: i64, rose: f32 x rose_dim, gram: f32 x gram_dim,
//!              colour: f32 x color_dim )* ]
//! ```
//! Tombstoned entries have `id == i64::MIN`. A version mismatch makes `load`
//! start fresh — the startup migration (`core::migrate`) turns that into a
//! one-time re-index.

use crate::{
    config::{GRAM_DIM_PER_ZOOM, GRAM_ZOOM_LEVELS, GRAM_WEIGHT, ROSE_DIM, ROSE_WEIGHT},
    core::color::COLOR_DIM,
    error::{PictoriaError, Result},
};
use once_cell::sync::Lazy;
use rayon::prelude::*;
use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::{BufWriter, Write},
    path::Path,
    sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

/// Serialises concurrent load -> modify -> save cycles across sync workers.
static SYNC_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

/// Separates readers (search) from the brief file-write step (save).
static STORE_RW_LOCK: Lazy<RwLock<()>> = Lazy::new(|| RwLock::new(()));

pub fn store_io_guard() -> MutexGuard<'static, ()> {
    SYNC_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}
pub fn store_io_read_guard() -> RwLockReadGuard<'static, ()> {
    STORE_RW_LOCK.read().unwrap_or_else(|e| e.into_inner())
}
pub fn store_io_write_guard() -> RwLockWriteGuard<'static, ()> {
    STORE_RW_LOCK.write().unwrap_or_else(|e| e.into_inner())
}

const MAGIC: &[u8; 8] = b"PICTOR\x00\x01";
const VERSION: u32 = 6;
const HEADER_LEN: usize = 32;
const TOMBSTONE: i64 = i64::MIN;
const GRAM_DIM: usize = GRAM_ZOOM_LEVELS * GRAM_DIM_PER_ZOOM;

/// One search hit.
#[derive(Debug, Clone, Copy)]
pub struct Match {
    pub id: i64,
    pub score: f32,
    pub rose_sim: f32,
    pub gram_sim: f32,
    pub color_sim: f32,
}

pub struct VectorStore {
    ids: Vec<i64>,
    rose: Vec<f32>,
    gram: Vec<f32>,
    color: Vec<f32>,
}

impl VectorStore {
    pub fn new() -> Self {
        Self { ids: Vec::new(), rose: Vec::new(), gram: Vec::new(), color: Vec::new() }
    }

    // ── Construction ──────────────────────────────────────────────────

    /// Load from disk, or a fresh store if the file is missing, corrupt, or
    /// an older format version — a version mismatch is what arms the
    /// startup re-index in `core::migrate`.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let bytes = fs::read(path)?;
        if bytes.len() < HEADER_LEN || &bytes[..8] != MAGIC {
            log::warn!("vector store missing/corrupt header — starting fresh");
            return Ok(Self::new());
        }
        let version = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        if version != VERSION {
            log::warn!("vector store version {version} != {VERSION} — starting fresh (re-index)");
            return Ok(Self::new());
        }
        // Header layout (see module doc): magic(8) version(4) rose_dim(4)
        // count(8) gram_dim(4) color_dim(4) — count sits between rose_dim and
        // gram_dim, so gram_dim/color_dim start at 24/28, not 20/24.
        let rose_dim = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let gram_dim = u32::from_le_bytes(bytes[24..28].try_into().unwrap()) as usize;
        let color_dim = u32::from_le_bytes(bytes[28..32].try_into().unwrap()) as usize;
        if rose_dim != ROSE_DIM || gram_dim != GRAM_DIM || color_dim != COLOR_DIM {
            log::warn!("vector store dims changed — starting fresh (re-index)");
            return Ok(Self::new());
        }

        let body = &bytes[HEADER_LEN..];
        let entry_bytes = 8 + (rose_dim + gram_dim + color_dim) * 4;
        if entry_bytes == 0 || body.len() % entry_bytes != 0 {
            log::warn!("vector store body not aligned — starting fresh");
            return Ok(Self::new());
        }

        let n = body.len() / entry_bytes;
        let mut ids = Vec::with_capacity(n);
        let mut rose = Vec::with_capacity(n * rose_dim);
        let mut gram = Vec::with_capacity(n * gram_dim);
        let mut color = Vec::with_capacity(n * color_dim);

        let read_f32 = |b: &[u8], at: usize| f32::from_le_bytes(b[at..at + 4].try_into().unwrap());

        for i in 0..n {
            let mut off = i * entry_bytes;
            ids.push(i64::from_le_bytes(body[off..off + 8].try_into().unwrap()));
            off += 8;
            for j in 0..rose_dim { rose.push(read_f32(body, off + j * 4)); }
            off += rose_dim * 4;
            for j in 0..gram_dim { gram.push(read_f32(body, off + j * 4)); }
            off += gram_dim * 4;
            for j in 0..color_dim { color.push(read_f32(body, off + j * 4)); }
        }

        // Defensive de-dup: a corrupt/interrupted write can leave one id stored
        // twice. Keep the last occurrence of each and drop tombstones.
        let live = ids.iter().filter(|&&id| id != TOMBSTONE).count();
        let unique: HashSet<i64> = ids.iter().copied().filter(|&id| id != TOMBSTONE).collect();
        if unique.len() != live || live != ids.len() {
            let mut last: std::collections::HashMap<i64, usize> = std::collections::HashMap::with_capacity(unique.len());
            for (i, &id) in ids.iter().enumerate() {
                if id != TOMBSTONE { last.insert(id, i); }
            }
            let mut keep: Vec<usize> = last.into_values().collect();
            keep.sort_unstable();
            let mut nids = Vec::with_capacity(keep.len());
            let mut nrose = Vec::with_capacity(keep.len() * rose_dim);
            let mut ngram = Vec::with_capacity(keep.len() * gram_dim);
            let mut ncolor = Vec::with_capacity(keep.len() * color_dim);
            for &i in &keep {
                nids.push(ids[i]);
                nrose.extend_from_slice(&rose[i * rose_dim..(i + 1) * rose_dim]);
                ngram.extend_from_slice(&gram[i * gram_dim..(i + 1) * gram_dim]);
                ncolor.extend_from_slice(&color[i * color_dim..(i + 1) * color_dim]);
            }
            log::warn!("vector store: {} raw entries deduped to {} unique on load", ids.len(), nids.len());
            ids = nids; rose = nrose; gram = ngram; color = ncolor;
        }

        Ok(Self { ids, rose, gram, color })
    }

    /// Persist to disk atomically (write to `.tmp`, then rename).
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("bin.tmp");
        {
            let file = OpenOptions::new().write(true).create(true).truncate(true).open(&tmp)?;
            let mut w = BufWriter::new(file);
            let live_count = self.ids.iter().filter(|&&id| id != TOMBSTONE).count() as u64;

            w.write_all(MAGIC)?;
            w.write_all(&VERSION.to_le_bytes())?;
            w.write_all(&(ROSE_DIM as u32).to_le_bytes())?;
            w.write_all(&live_count.to_le_bytes())?;
            w.write_all(&(GRAM_DIM as u32).to_le_bytes())?;
            w.write_all(&(COLOR_DIM as u32).to_le_bytes())?;

            let entry_bytes = 8 + (ROSE_DIM + GRAM_DIM + COLOR_DIM) * 4;
            let mut buf = vec![0u8; entry_bytes];
            for (idx, &id) in self.ids.iter().enumerate() {
                buf[..8].copy_from_slice(&id.to_le_bytes());
                let mut o = 8;
                for &v in &self.rose[idx * ROSE_DIM..(idx + 1) * ROSE_DIM] {
                    buf[o..o + 4].copy_from_slice(&v.to_le_bytes()); o += 4;
                }
                for &v in &self.gram[idx * GRAM_DIM..(idx + 1) * GRAM_DIM] {
                    buf[o..o + 4].copy_from_slice(&v.to_le_bytes()); o += 4;
                }
                for &v in &self.color[idx * COLOR_DIM..(idx + 1) * COLOR_DIM] {
                    buf[o..o + 4].copy_from_slice(&v.to_le_bytes()); o += 4;
                }
                w.write_all(&buf)?;
            }
            w.flush()?;
        }
        fs::rename(&tmp, path)?;
        Ok(())
    }

    // ── Mutation ──────────────────────────────────────────────────────

    /// Insert or replace one file's descriptor (replaces any existing entry
    /// for `id` in place, so a re-embed doesn't leave a stale duplicate).
    pub fn upsert(&mut self, id: i64, rose: &[f32], gram: &[f32], color: &[f32]) -> Result<()> {
        if rose.len() != ROSE_DIM || gram.len() != GRAM_DIM || color.len() != COLOR_DIM {
            return Err(PictoriaError::Fatal(format!(
                "descriptor dim mismatch: rose {}/{ROSE_DIM} gram {}/{GRAM_DIM} color {}/{COLOR_DIM}",
                rose.len(), gram.len(), color.len()
            )));
        }
        if let Some(pos) = self.ids.iter().position(|&x| x == id) {
            self.rose[pos * ROSE_DIM..(pos + 1) * ROSE_DIM].copy_from_slice(rose);
            self.gram[pos * GRAM_DIM..(pos + 1) * GRAM_DIM].copy_from_slice(gram);
            self.color[pos * COLOR_DIM..(pos + 1) * COLOR_DIM].copy_from_slice(color);
        } else {
            self.ids.push(id);
            self.rose.extend_from_slice(rose);
            self.gram.extend_from_slice(gram);
            self.color.extend_from_slice(color);
        }
        Ok(())
    }

    /// Tombstone every id in `ids`. Compacts automatically past 20% dead.
    pub fn remove(&mut self, ids: &[i64]) {
        if ids.is_empty() { return; }
        let id_set: HashSet<i64> = ids.iter().copied().collect();
        for slot in self.ids.iter_mut() {
            if id_set.contains(slot) { *slot = TOMBSTONE; }
        }
        self.maybe_compact();
    }

    // ── Query ─────────────────────────────────────────────────────────

    /// Stage-1 ranking: every live entry scored by weighted Gabor-rose +
    /// Gram-matrix cosine similarity against the query, best `k` returned.
    /// This is the cheap pre-filter — `core::sidecar::verify` does the real
    /// proof afterwards on however many of these the caller shortlists.
    pub fn search(&self, query_rose: &[f32], query_gram: &[f32], query_color: &[f32], k: usize) -> Vec<Match> {
        if self.ids.is_empty() || k == 0 {
            return Vec::new();
        }
        let use_color = query_color.len() == COLOR_DIM;

        let mut scored: Vec<Match> = self
            .ids
            .par_iter()
            .enumerate()
            .filter_map(|(i, &id)| {
                if id == TOMBSTONE { return None; }
                let r = &self.rose[i * ROSE_DIM..(i + 1) * ROSE_DIM];
                let g = &self.gram[i * GRAM_DIM..(i + 1) * GRAM_DIM];
                let rose_sim = cos_sim(query_rose, r);
                let gram_sim = best_gram_sim(query_gram, g);
                let color_sim = if use_color {
                    cos_sim(query_color, &self.color[i * COLOR_DIM..(i + 1) * COLOR_DIM])
                } else {
                    0.0
                };
                let score = ROSE_WEIGHT * rose_sim + GRAM_WEIGHT * gram_sim;
                Some(Match { id, score, rose_sim, gram_sim, color_sim })
            })
            .collect();

        scored.sort_unstable_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);
        scored
    }

    pub fn live_count(&self) -> usize {
        self.ids.iter().filter(|&&id| id != TOMBSTONE).count()
    }

    // ── Compaction ────────────────────────────────────────────────────

    fn maybe_compact(&mut self) {
        let total = self.ids.len();
        let tombstones = self.ids.iter().filter(|&&id| id == TOMBSTONE).count();
        if total == 0 || tombstones * 5 < total { return; }

        let keep = total - tombstones;
        let mut nids = Vec::with_capacity(keep);
        let mut nrose = Vec::with_capacity(keep * ROSE_DIM);
        let mut ngram = Vec::with_capacity(keep * GRAM_DIM);
        let mut ncolor = Vec::with_capacity(keep * COLOR_DIM);
        for (i, &id) in self.ids.iter().enumerate() {
            if id != TOMBSTONE {
                nids.push(id);
                nrose.extend_from_slice(&self.rose[i * ROSE_DIM..(i + 1) * ROSE_DIM]);
                ngram.extend_from_slice(&self.gram[i * GRAM_DIM..(i + 1) * GRAM_DIM]);
                ncolor.extend_from_slice(&self.color[i * COLOR_DIM..(i + 1) * COLOR_DIM]);
            }
        }
        log::debug!("vector store compacted: {} -> {} entries", total, nids.len());
        self.ids = nids; self.rose = nrose; self.gram = ngram; self.color = ncolor;
    }
}

impl Default for VectorStore {
    fn default() -> Self { Self::new() }
}

// ── Similarity helpers ───────────────────────────────────────────────────

fn cos_sim(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na <= 0.0 || nb <= 0.0 { 0.0 } else { dot / (na * nb) }
}

/// Gram vectors carry `GRAM_ZOOM_LEVELS` concatenated zoom levels; best score
/// across every (query zoom, candidate zoom) pair, matching
/// `sidecar/pipeline.py`'s `best_gram_sim` so ranking is scale-robust — two
/// related images photographed at different pixel scales still match.
fn best_gram_sim(query_gram: &[f32], candidate_gram: &[f32]) -> f32 {
    let mut best = f32::NEG_INFINITY;
    for qz in query_gram.chunks(GRAM_DIM_PER_ZOOM) {
        for cz in candidate_gram.chunks(GRAM_DIM_PER_ZOOM) {
            let s = cos_sim(qz, cz);
            if s > best { best = s; }
        }
    }
    if best.is_finite() { best } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_rose(seed: f32) -> Vec<f32> {
        normalize((0..ROSE_DIM).map(|i| ((i as f32) * seed).sin()).collect())
    }
    fn unit_gram(seed: f32) -> Vec<f32> {
        (0..GRAM_ZOOM_LEVELS)
            .flat_map(|z| normalize((0..GRAM_DIM_PER_ZOOM).map(|i| ((i as f32 + z as f32) * seed).sin()).collect()))
            .collect()
    }
    fn normalize(v: Vec<f32>) -> Vec<f32> {
        let n = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
        v.into_iter().map(|x| x / n).collect()
    }
    fn color(seed: f32) -> Vec<f32> {
        normalize((0..COLOR_DIM).map(|i| ((i as f32) * seed).sin()).collect())
    }

    #[test]
    fn round_trips_to_disk() {
        let dir = std::env::temp_dir().join(format!("vs_test_{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("vectors.bin");

        let mut s = VectorStore::new();
        s.upsert(7, &unit_rose(0.1), &unit_gram(0.1), &color(0.1)).unwrap();
        s.upsert(8, &unit_rose(0.2), &unit_gram(0.2), &color(0.2)).unwrap();
        s.save(&path).unwrap();

        let back = VectorStore::load(&path).unwrap();
        assert_eq!(back.live_count(), 2);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn upsert_replaces_not_duplicates() {
        let mut s = VectorStore::new();
        s.upsert(1, &unit_rose(0.1), &unit_gram(0.1), &color(0.1)).unwrap();
        s.upsert(1, &unit_rose(0.9), &unit_gram(0.9), &color(0.9)).unwrap();
        assert_eq!(s.live_count(), 1);
    }

    #[test]
    fn remove_tombstones() {
        let mut s = VectorStore::new();
        s.upsert(1, &unit_rose(0.1), &unit_gram(0.1), &color(0.1)).unwrap();
        s.upsert(2, &unit_rose(0.2), &unit_gram(0.2), &color(0.2)).unwrap();
        s.remove(&[1]);
        assert_eq!(s.live_count(), 1);
    }

    #[test]
    fn search_ranks_the_closest_match_first() {
        let mut s = VectorStore::new();
        let target_rose = unit_rose(0.7);
        let target_gram = unit_gram(0.7);
        s.upsert(1, &unit_rose(0.11), &unit_gram(0.11), &color(0.1)).unwrap();
        s.upsert(2, &target_rose, &target_gram, &color(0.2)).unwrap();
        let hits = s.search(&target_rose, &target_gram, &[], 10);
        assert_eq!(hits[0].id, 2);
        assert!(hits[0].score > hits[1].score);
    }

    #[test]
    fn gram_zoom_levels_are_matched_independently_not_flattened() {
        // A candidate whose zoom-1 vector equals the query's zoom-0 vector
        // should still score near-perfectly, because best_gram_sim tries
        // every (query zoom, candidate zoom) pair.
        let qz0 = unit_gram(0.3)[..GRAM_DIM_PER_ZOOM].to_vec();
        let mut query_gram = qz0.clone();
        query_gram.extend(vec![0.0; GRAM_DIM_PER_ZOOM * (GRAM_ZOOM_LEVELS - 1)]);

        let mut candidate_gram = vec![0.0; GRAM_DIM_PER_ZOOM];
        candidate_gram.extend(qz0);
        candidate_gram.extend(vec![0.0; GRAM_DIM_PER_ZOOM * (GRAM_ZOOM_LEVELS - 2).max(0)]);
        candidate_gram.truncate(GRAM_DIM);

        let sim = best_gram_sim(&query_gram, &candidate_gram);
        assert!(sim > 0.99, "expected near-1.0 cross-zoom match, got {sim}");
    }
}
