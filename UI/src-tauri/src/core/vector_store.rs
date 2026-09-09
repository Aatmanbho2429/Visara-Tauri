// Custom flat vector store for the DINO embedding that drives retrieval,
// the Gabor-rose + Gram-matrix descriptor kept alongside it, and a colour
// histogram per file.
//
// ## v7: retrieval is a threshold over the DINO embedding
// There is no top-N shortlist any more. `near_family` returns *every* entry
// whose embedding cosine clears `NEAR_FAMILY_MIN_SIM`, and SIFT/RANSAC
// verification runs over all of them — so the embedding decides membership
// and the geometry decides truth. Rose/gram are still stored and scored, but
// only as a tiebreak and for diagnostics.
//
// ## v6: one entry per file, not per region
// The old DINOv2 scheme stored several vectors per file (whole frame plus
// sliced windows) because a single 224x224 embedding couldn't otherwise
// answer "does this design appear *inside* that one". SIFT/RANSAC
// (`core::sidecar::verify`) answers that question directly against the full
// image instead, so there's nothing left to slice for — one descriptor per
// file, looked up by id.
//
// ## Why not FAISS
// The FAISS C++ library needs a non-trivial build step and can't be
// sandboxed by the macOS App Store. Brute-force cosine over a rayon thread
// pool is fast enough for our workload (<=500k images).
//
// ## File format (`vectors.bin`, version 7)
// ```text
// [ magic:      8 bytes  "PICTOR\x00\x01" ]
// [ version:    4 bytes  u32 LE ]  == 7
// [ rose_dim:   4 bytes  u32 LE ]  == ROSE_DIM
// [ count:      8 bytes  u64 LE ]  live (non-tombstone) entries
// [ gram_dim:   4 bytes  u32 LE ]  == GRAM_ZOOM_LEVELS * GRAM_DIM_PER_ZOOM
// [ color_dim:  4 bytes  u32 LE ]  == COLOR_DIM
// [ embed_dim:  4 bytes  u32 LE ]  == EMBED_ZOOM_LEVELS * EMBED_DIM
// [ entries: ( id: i64, embed: f32 x embed_dim, rose: f32 x rose_dim,
//              gram: f32 x gram_dim, colour: f32 x color_dim )* ]
// ```
// `embed_dim` is appended after `color_dim` so the header grew rather than
// being reshuffled — the v6 fields keep their offsets, which makes the two
// layouts easy to compare when debugging a store by hand.
// Tombstoned entries have `id == i64::MIN`. A version mismatch makes `load`
// start fresh — the startup migration (`core::migrate`) turns that into a
// one-time re-index.

use crate::{
    config::{
        EMBED_DIM, EMBED_ZOOM_LEVELS, GRAM_DIM_PER_ZOOM, GRAM_WEIGHT, GRAM_ZOOM_LEVELS, ROSE_DIM,
        ROSE_WEIGHT,
    },
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
    sync::{Arc, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

// Serialises concurrent load -> modify -> save cycles across sync workers.
static SYNC_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

// Separates readers (search) from the brief file-write step (save).
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

// ── Resident cache (SEARCH-LATENCY-PLAN.md Phase 5a) ───────────────────────
//
// There is no long-lived store otherwise: `VectorStore::load` re-reads and
// re-parses all of `vectors.bin` through a per-`f32` `from_le_bytes` loop on
// every single search. Entry stride is 25,448 bytes (4608 embed + 8 rose +
// 1728 gram + 16 color floats, plus an i64 id) — ~1.19 GB at 50k files, so
// this caches the parsed result instead of redoing that work per query.
static RESIDENT: Lazy<RwLock<Option<Arc<VectorStore>>>> = Lazy::new(|| RwLock::new(None));

// Returns the cached store, loading it from disk on a cache miss (under
// `store_io_read_guard`, same as any other disk read of this file — a
// concurrent `save()` must not tear it). Cloning the `Arc` and dropping the
// lock immediately is what lets `services::search` avoid holding any guard
// across the multi-second verify phase that follows (Phase 1d).
pub fn resident(path: &Path) -> Result<Arc<VectorStore>> {
    if let Some(store) = RESIDENT.read().unwrap_or_else(|e| e.into_inner()).clone() {
        return Ok(store);
    }
    let mut slot = RESIDENT.write().unwrap_or_else(|e| e.into_inner());
    // Re-check under the write lock — another thread may have populated it
    // while this one was waiting.
    if let Some(store) = slot.clone() {
        return Ok(store);
    }
    let loaded = {
        let _read_guard = store_io_read_guard();
        VectorStore::load(path)?
    };
    log::info!(
        "[vector_store] resident store loaded: {} live entries (~{:.1} MB)",
        loaded.live_count(),
        loaded.approx_bytes() as f64 / 1_048_576.0,
    );
    let loaded = Arc::new(loaded);
    *slot = Some(loaded.clone());
    Ok(loaded)
}

// Drops the cached copy so the next `resident()` call reloads from disk.
// Call this right after a successful `store.save()` — see `core::watcher` —
// inside the same `store_io_write_guard()` scope, so no search can observe a
// stale resident copy alongside a freshly-written file.
pub fn invalidate_resident() {
    *RESIDENT.write().unwrap_or_else(|e| e.into_inner()) = None;
}

const MAGIC: &[u8; 8] = b"PICTOR\x00\x01";
const VERSION: u32 = 7;
const HEADER_LEN: usize = 36;
const TOMBSTONE: i64 = i64::MIN;
const GRAM_DIM: usize = GRAM_ZOOM_LEVELS * GRAM_DIM_PER_ZOOM;
// Full stored embedding width: every zoom level, concatenated.
const EMBED_TOTAL: usize = EMBED_ZOOM_LEVELS * EMBED_DIM;

// One search hit.
#[derive(Debug, Clone, Copy)]
pub struct Match {
    pub id: i64,
    // Embedding cosine — what decided membership of the near family, and what
    // the UI shows as the pattern match.
    pub embed_sim: f32,
    // Weighted rose+gram similarity. Tiebreak between entries at the same
    // embedding cosine, and a diagnostic in the search log; never a filter.
    pub design_sim: f32,
    pub rose_sim: f32,
    pub gram_sim: f32,
    pub color_sim: f32,
}

pub struct VectorStore {
    ids: Vec<i64>,
    embed: Vec<f32>,
    rose: Vec<f32>,
    gram: Vec<f32>,
    color: Vec<f32>,
}

impl VectorStore {
    pub fn new() -> Self {
        Self {
            ids: Vec::new(),
            embed: Vec::new(),
            rose: Vec::new(),
            gram: Vec::new(),
            color: Vec::new(),
        }
    }

    // ── Construction ──────────────────────────────────────────────────

    // Load from disk, or a fresh store if the file is missing, corrupt, or
    // an older format version — a version mismatch is what arms the
    // startup re-index in `core::migrate`.
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
        let embed_dim = u32::from_le_bytes(bytes[32..36].try_into().unwrap()) as usize;
        if rose_dim != ROSE_DIM
            || gram_dim != GRAM_DIM
            || color_dim != COLOR_DIM
            || embed_dim != EMBED_TOTAL
        {
            log::warn!("vector store dims changed — starting fresh (re-index)");
            return Ok(Self::new());
        }

        let body = &bytes[HEADER_LEN..];
        let entry_bytes = 8 + (embed_dim + rose_dim + gram_dim + color_dim) * 4;
        if entry_bytes == 0 || body.len() % entry_bytes != 0 {
            log::warn!("vector store body not aligned — starting fresh");
            return Ok(Self::new());
        }

        let n = body.len() / entry_bytes;
        let mut ids = Vec::with_capacity(n);
        let mut embed = Vec::with_capacity(n * embed_dim);
        let mut rose = Vec::with_capacity(n * rose_dim);
        let mut gram = Vec::with_capacity(n * gram_dim);
        let mut color = Vec::with_capacity(n * color_dim);

        let read_f32 = |b: &[u8], at: usize| f32::from_le_bytes(b[at..at + 4].try_into().unwrap());

        for i in 0..n {
            let mut off = i * entry_bytes;
            ids.push(i64::from_le_bytes(body[off..off + 8].try_into().unwrap()));
            off += 8;
            for j in 0..embed_dim { embed.push(read_f32(body, off + j * 4)); }
            off += embed_dim * 4;
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
            let mut nembed = Vec::with_capacity(keep.len() * embed_dim);
            let mut nrose = Vec::with_capacity(keep.len() * rose_dim);
            let mut ngram = Vec::with_capacity(keep.len() * gram_dim);
            let mut ncolor = Vec::with_capacity(keep.len() * color_dim);
            for &i in &keep {
                nids.push(ids[i]);
                nembed.extend_from_slice(&embed[i * embed_dim..(i + 1) * embed_dim]);
                nrose.extend_from_slice(&rose[i * rose_dim..(i + 1) * rose_dim]);
                ngram.extend_from_slice(&gram[i * gram_dim..(i + 1) * gram_dim]);
                ncolor.extend_from_slice(&color[i * color_dim..(i + 1) * color_dim]);
            }
            log::warn!("vector store: {} raw entries deduped to {} unique on load", ids.len(), nids.len());
            ids = nids; embed = nembed; rose = nrose; gram = ngram; color = ncolor;
        }

        // `best_zoom_dot` (Phase 5b) is only correct when every stored zoom
        // level is already a unit vector, which `upsert` guarantees for
        // anything written from here on. This just confirms it on the data
        // actually on disk — debug-only since it's an O(entries) scan with
        // no effect on behaviour, only a caught assumption.
        #[cfg(debug_assertions)]
        {
            for i in 0..ids.len() {
                if ids[i] == TOMBSTONE { continue; }
                let e = &embed[i * embed_dim..(i + 1) * embed_dim];
                for zoom in e.chunks(EMBED_DIM) {
                    let norm: f32 = zoom.iter().map(|x| x * x).sum::<f32>().sqrt();
                    debug_assert!(
                        (norm - 1.0).abs() < 0.05 || norm < 1e-6,
                        "vector store: embed zoom level not unit-normalized (norm={norm}) — \
                         best_zoom_dot will silently under/over-score this entry",
                    );
                }
            }
        }

        Ok(Self { ids, embed, rose, gram, color })
    }

    // Persist to disk atomically (write to `.tmp`, then rename).
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
            w.write_all(&(EMBED_TOTAL as u32).to_le_bytes())?;

            let entry_bytes = 8 + (EMBED_TOTAL + ROSE_DIM + GRAM_DIM + COLOR_DIM) * 4;
            let mut buf = vec![0u8; entry_bytes];
            for (idx, &id) in self.ids.iter().enumerate() {
                buf[..8].copy_from_slice(&id.to_le_bytes());
                let mut o = 8;
                for &v in &self.embed[idx * EMBED_TOTAL..(idx + 1) * EMBED_TOTAL] {
                    buf[o..o + 4].copy_from_slice(&v.to_le_bytes()); o += 4;
                }
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

    // Insert or replace one file's descriptor (replaces any existing entry
    // for `id` in place, so a re-embed doesn't leave a stale duplicate).
    pub fn upsert(
        &mut self,
        id: i64,
        embed: &[f32],
        rose: &[f32],
        gram: &[f32],
        color: &[f32],
    ) -> Result<()> {
        if embed.len() != EMBED_TOTAL
            || rose.len() != ROSE_DIM
            || gram.len() != GRAM_DIM
            || color.len() != COLOR_DIM
        {
            return Err(PictoriaError::Fatal(format!(
                "descriptor dim mismatch: embed {}/{EMBED_TOTAL} rose {}/{ROSE_DIM} \
                 gram {}/{GRAM_DIM} color {}/{COLOR_DIM}",
                embed.len(), rose.len(), gram.len(), color.len()
            )));
        }
        // Guarantee the precondition `best_zoom_dot` needs (SEARCH-LATENCY-PLAN.md
        // Phase 5b) rather than trusting the producer: each zoom level must be
        // its own unit vector for a dot product to equal that level's cosine.
        // `pipeline.py::embed_descriptor` already normalises this way, but
        // doing it again here is nearly free (once per file) and makes the
        // store correct even against a future producer that doesn't. Rose,
        // gram, and color are untouched — they keep full `cos_sim` (gram
        // isn't normalised at all, and rose is L1- not L2-normalised).
        let mut embed = embed.to_vec();
        normalize_zoom_levels(&mut embed, EMBED_DIM);
        let embed = embed.as_slice();

        if let Some(pos) = self.ids.iter().position(|&x| x == id) {
            self.embed[pos * EMBED_TOTAL..(pos + 1) * EMBED_TOTAL].copy_from_slice(embed);
            self.rose[pos * ROSE_DIM..(pos + 1) * ROSE_DIM].copy_from_slice(rose);
            self.gram[pos * GRAM_DIM..(pos + 1) * GRAM_DIM].copy_from_slice(gram);
            self.color[pos * COLOR_DIM..(pos + 1) * COLOR_DIM].copy_from_slice(color);
        } else {
            self.ids.push(id);
            self.embed.extend_from_slice(embed);
            self.rose.extend_from_slice(rose);
            self.gram.extend_from_slice(gram);
            self.color.extend_from_slice(color);
        }
        Ok(())
    }

    // Tombstone every id in `ids`. Compacts automatically past 20% dead.
    pub fn remove(&mut self, ids: &[i64]) {
        if ids.is_empty() { return; }
        let id_set: HashSet<i64> = ids.iter().copied().collect();
        for slot in self.ids.iter_mut() {
            if id_set.contains(slot) { *slot = TOMBSTONE; }
        }
        self.maybe_compact();
    }

    // ── Query ─────────────────────────────────────────────────────────

    // Every live entry whose DINO embedding cosine reaches `min_sim`, sorted
    // best first. No `k` — the caller verifies all of them.
    //
    // This replaced the old fixed top-N shortlist. The difference matters: a
    // top-N cut silently dropped real matches once a library held more than N
    // similar tiles, whereas a threshold returns the whole family and lets
    // SIFT/RANSAC decide which members are genuine. The cost is that the
    // returned set is unbounded by construction — `services::search` logs its
    // size for exactly that reason.
    //
    // Rose/gram are still scored here, but only to fill `design_sim` (a
    // tiebreak between entries at the same embedding cosine, and a diagnostic
    // in the search timing log). They cannot add or remove a candidate.
    pub fn near_family(
        &self,
        query_embed: &[f32],
        query_rose: &[f32],
        query_gram: &[f32],
        query_color: &[f32],
        min_sim: f32,
    ) -> Vec<Match> {
        if self.ids.is_empty() {
            return Vec::new();
        }
        let use_color = query_color.len() == COLOR_DIM;

        // `best_zoom_dot`, not `best_zoom_sim`, for the embedding — the only
        // descriptor evaluated against all N entries (rose/gram/color only
        // run for entries that already passed the floor). `cos_sim` was
        // recomputing both operands' norms on every one of those N calls;
        // normalizing the query here, once, is what lets a plain dot product
        // substitute for it (SEARCH-LATENCY-PLAN.md Phase 5b). Stored
        // entries are already normalised this way by `upsert`.
        let mut query_embed_n = query_embed.to_vec();
        normalize_zoom_levels(&mut query_embed_n, EMBED_DIM);
        let query_embed = query_embed_n.as_slice();

        let mut scored: Vec<Match> = self
            .ids
            .par_iter()
            .enumerate()
            .filter_map(|(i, &id)| {
                if id == TOMBSTONE { return None; }
                let e = &self.embed[i * EMBED_TOTAL..(i + 1) * EMBED_TOTAL];
                let embed_sim = best_zoom_dot(query_embed, e, EMBED_DIM);
                if embed_sim < min_sim {
                    return None;
                }
                let r = &self.rose[i * ROSE_DIM..(i + 1) * ROSE_DIM];
                let g = &self.gram[i * GRAM_DIM..(i + 1) * GRAM_DIM];
                let rose_sim = cos_sim(query_rose, r);
                let gram_sim = best_zoom_sim(query_gram, g, GRAM_DIM_PER_ZOOM);
                let color_sim = if use_color {
                    cos_sim(query_color, &self.color[i * COLOR_DIM..(i + 1) * COLOR_DIM])
                } else {
                    0.0
                };
                let design_sim = ROSE_WEIGHT * rose_sim + GRAM_WEIGHT * gram_sim;
                Some(Match { id, embed_sim, design_sim, rose_sim, gram_sim, color_sim })
            })
            .collect();

        scored.sort_unstable_by(|a, b| {
            b.embed_sim
                .partial_cmp(&a.embed_sim)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    b.design_sim.partial_cmp(&a.design_sim).unwrap_or(std::cmp::Ordering::Equal)
                })
        });
        scored
    }

    pub fn live_count(&self) -> usize {
        self.ids.iter().filter(|&&id| id != TOMBSTONE).count()
    }

    // Approximate resident-memory footprint, for the one-time log line at
    // `resident()` load — makes RSS visible in the field without needing a
    // profiler (SEARCH-LATENCY-PLAN.md Phase 5a).
    pub fn approx_bytes(&self) -> usize {
        self.ids.len() * 8
            + self.embed.len() * 4
            + self.rose.len() * 4
            + self.gram.len() * 4
            + self.color.len() * 4
    }

    // Diagnostic only — counts of live entries whose embedding cosine
    // reaches each of `thresholds`, over the *whole* population. Unlike
    // `near_family`, which only ever reports entries already at or above its
    // own floor, this is what the `NEAR_FAMILY_MIN_SIM` calibration
    // procedure needs: what a lower (or higher) floor would have admitted.
    // See SEARCH-LATENCY-PLAN.md Phase 3.
    pub fn cosine_histogram(&self, query_embed: &[f32], thresholds: &[f32]) -> Vec<usize> {
        if self.ids.is_empty() {
            return vec![0; thresholds.len()];
        }
        let mut query_embed_n = query_embed.to_vec();
        normalize_zoom_levels(&mut query_embed_n, EMBED_DIM);
        let query_embed = query_embed_n.as_slice();

        self.ids
            .par_iter()
            .enumerate()
            .filter(|&(_, &id)| id != TOMBSTONE)
            .map(|(i, _)| {
                let e = &self.embed[i * EMBED_TOTAL..(i + 1) * EMBED_TOTAL];
                best_zoom_dot(query_embed, e, EMBED_DIM)
            })
            .fold(
                || vec![0usize; thresholds.len()],
                |mut acc, sim| {
                    for (j, &t) in thresholds.iter().enumerate() {
                        if sim >= t { acc[j] += 1; }
                    }
                    acc
                },
            )
            .reduce(
                || vec![0usize; thresholds.len()],
                |mut a, b| {
                    for j in 0..a.len() { a[j] += b[j]; }
                    a
                },
            )
    }

    // ── Compaction ────────────────────────────────────────────────────

    fn maybe_compact(&mut self) {
        let total = self.ids.len();
        let tombstones = self.ids.iter().filter(|&&id| id == TOMBSTONE).count();
        if total == 0 || tombstones * 5 < total { return; }

        let keep = total - tombstones;
        let mut nids = Vec::with_capacity(keep);
        let mut nembed = Vec::with_capacity(keep * EMBED_TOTAL);
        let mut nrose = Vec::with_capacity(keep * ROSE_DIM);
        let mut ngram = Vec::with_capacity(keep * GRAM_DIM);
        let mut ncolor = Vec::with_capacity(keep * COLOR_DIM);
        for (i, &id) in self.ids.iter().enumerate() {
            if id != TOMBSTONE {
                nids.push(id);
                nembed.extend_from_slice(&self.embed[i * EMBED_TOTAL..(i + 1) * EMBED_TOTAL]);
                nrose.extend_from_slice(&self.rose[i * ROSE_DIM..(i + 1) * ROSE_DIM]);
                ngram.extend_from_slice(&self.gram[i * GRAM_DIM..(i + 1) * GRAM_DIM]);
                ncolor.extend_from_slice(&self.color[i * COLOR_DIM..(i + 1) * COLOR_DIM]);
            }
        }
        log::debug!("vector store compacted: {} -> {} entries", total, nids.len());
        self.ids = nids; self.embed = nembed; self.rose = nrose; self.gram = ngram; self.color = ncolor;
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

// Embedding and gram vectors both carry several concatenated zoom levels;
// score is the best over every (query zoom, candidate zoom) pair. That is
// what makes matching scale-robust — two related images at different pixel
// scales still match, because some pair of zoom levels lines them up.
//
// `per_zoom` is the width of one level, so one function serves both
// (`EMBED_DIM` and `GRAM_DIM_PER_ZOOM`).
fn best_zoom_sim(query: &[f32], candidate: &[f32], per_zoom: usize) -> f32 {
    let mut best = f32::NEG_INFINITY;
    for qz in query.chunks(per_zoom) {
        for cz in candidate.chunks(per_zoom) {
            let s = cos_sim(qz, cz);
            if s > best { best = s; }
        }
    }
    if best.is_finite() { best } else { 0.0 }
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

// Same cross-zoom search as `best_zoom_sim`, but a plain dot product instead
// of a full cosine — correct only when both `query` and `candidate` are
// already unit vectors per zoom level (see `normalize_zoom_levels`). Used
// only for the embedding (SEARCH-LATENCY-PLAN.md Phase 5b); rose, gram, and
// color keep `best_zoom_sim`/`cos_sim` because they are not both normalised
// the same way (gram not at all, rose L1 not L2).
fn best_zoom_dot(query: &[f32], candidate: &[f32], per_zoom: usize) -> f32 {
    let mut best = f32::NEG_INFINITY;
    for qz in query.chunks(per_zoom) {
        for cz in candidate.chunks(per_zoom) {
            let s = dot(qz, cz);
            if s > best { best = s; }
        }
    }
    if best.is_finite() { best } else { 0.0 }
}

// L2-normalizes each contiguous `per_zoom` chunk of `v` independently, in
// place — each zoom level becomes its own unit vector rather than the whole
// concatenation being normalised together. That mirrors how
// `pipeline.py::embed_descriptor` already produces the embedding (each zoom
// level is meant to stand alone), and is the precondition `best_zoom_dot`
// needs to equal `best_zoom_sim`.
fn normalize_zoom_levels(v: &mut [f32], per_zoom: usize) {
    for chunk in v.chunks_mut(per_zoom) {
        let norm: f32 = chunk.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-12 {
            for x in chunk.iter_mut() { *x /= norm; }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_embed(seed: f32) -> Vec<f32> {
        (0..EMBED_ZOOM_LEVELS)
            .flat_map(|z| normalize((0..EMBED_DIM).map(|i| ((i as f32 + z as f32) * seed).sin()).collect()))
            .collect()
    }
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
        s.upsert(7, &unit_embed(0.1), &unit_rose(0.1), &unit_gram(0.1), &color(0.1)).unwrap();
        s.upsert(8, &unit_embed(0.2), &unit_rose(0.2), &unit_gram(0.2), &color(0.2)).unwrap();
        s.save(&path).unwrap();

        let back = VectorStore::load(&path).unwrap();
        assert_eq!(back.live_count(), 2);
        // Round-tripping must preserve the embedding itself, not just the row
        // count — the entry stride grew in v7 and an off-by-one there would
        // still load two entries, just misaligned ones.
        let hits = back.near_family(&unit_embed(0.1), &unit_rose(0.1), &unit_gram(0.1), &[], 0.99);
        assert_eq!(hits.len(), 1, "exactly the id-7 entry should clear 0.99");
        assert_eq!(hits[0].id, 7);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn upsert_replaces_not_duplicates() {
        let mut s = VectorStore::new();
        s.upsert(1, &unit_embed(0.1), &unit_rose(0.1), &unit_gram(0.1), &color(0.1)).unwrap();
        s.upsert(1, &unit_embed(0.9), &unit_rose(0.9), &unit_gram(0.9), &color(0.9)).unwrap();
        assert_eq!(s.live_count(), 1);
    }

    #[test]
    fn remove_tombstones() {
        let mut s = VectorStore::new();
        s.upsert(1, &unit_embed(0.1), &unit_rose(0.1), &unit_gram(0.1), &color(0.1)).unwrap();
        s.upsert(2, &unit_embed(0.2), &unit_rose(0.2), &unit_gram(0.2), &color(0.2)).unwrap();
        s.remove(&[1]);
        assert_eq!(s.live_count(), 1);
    }

    #[test]
    fn near_family_ranks_the_closest_match_first() {
        let mut s = VectorStore::new();
        let target_embed = unit_embed(0.7);
        s.upsert(1, &unit_embed(0.11), &unit_rose(0.11), &unit_gram(0.11), &color(0.1)).unwrap();
        s.upsert(2, &target_embed, &unit_rose(0.7), &unit_gram(0.7), &color(0.2)).unwrap();
        let hits = s.near_family(&target_embed, &unit_rose(0.7), &unit_gram(0.7), &[], -1.0);
        assert_eq!(hits[0].id, 2);
        assert!(hits[0].embed_sim > hits[1].embed_sim);
    }

    #[test]
    fn near_family_excludes_everything_below_the_threshold() {
        let mut s = VectorStore::new();
        let target_embed = unit_embed(0.7);
        s.upsert(1, &unit_embed(0.11), &unit_rose(0.11), &unit_gram(0.11), &color(0.1)).unwrap();
        s.upsert(2, &target_embed, &unit_rose(0.7), &unit_gram(0.7), &color(0.2)).unwrap();

        // A threshold just under a perfect self-match admits only the exact
        // entry; the point of the threshold is that it is a floor on
        // membership, not a ranking cut, so nothing below it survives however
        // few candidates that leaves.
        let hits = s.near_family(&target_embed, &unit_rose(0.7), &unit_gram(0.7), &[], 0.999);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, 2);

        // Nothing can clear a cosine above 1.0 — an empty family is a valid
        // outcome, not an error.
        let none = s.near_family(&target_embed, &unit_rose(0.7), &unit_gram(0.7), &[], 1.01);
        assert!(none.is_empty());
    }

    #[test]
    fn near_family_is_unbounded_by_count() {
        // No top-N cut: every entry over the floor comes back, even when that
        // is the entire library. This is the behaviour the fixed shortlist
        // used to silently violate.
        let mut s = VectorStore::new();
        let target_embed = unit_embed(0.5);
        for id in 1..=50i64 {
            s.upsert(id, &target_embed, &unit_rose(0.5), &unit_gram(0.5), &color(0.5)).unwrap();
        }
        let hits = s.near_family(&target_embed, &unit_rose(0.5), &unit_gram(0.5), &[], 0.9);
        assert_eq!(hits.len(), 50);
    }

    #[test]
    fn gram_zoom_levels_are_matched_independently_not_flattened() {
        // A candidate whose zoom-1 vector equals the query's zoom-0 vector
        // should still score near-perfectly, because best_zoom_sim tries
        // every (query zoom, candidate zoom) pair.
        let qz0 = unit_gram(0.3)[..GRAM_DIM_PER_ZOOM].to_vec();
        let mut query_gram = qz0.clone();
        query_gram.extend(vec![0.0; GRAM_DIM_PER_ZOOM * (GRAM_ZOOM_LEVELS - 1)]);

        let mut candidate_gram = vec![0.0; GRAM_DIM_PER_ZOOM];
        candidate_gram.extend(qz0);
        candidate_gram.extend(vec![0.0; GRAM_DIM_PER_ZOOM * (GRAM_ZOOM_LEVELS - 2).max(0)]);
        candidate_gram.truncate(GRAM_DIM);

        let sim = best_zoom_sim(&query_gram, &candidate_gram, GRAM_DIM_PER_ZOOM);
        assert!(sim > 0.99, "expected near-1.0 cross-zoom match, got {sim}");
    }

    #[test]
    fn best_zoom_dot_matches_best_zoom_sim_on_unit_inputs() {
        // `unit_embed` already L2-normalizes each zoom level independently
        // (see its own `flat_map` over `normalize`), so this is exactly the
        // precondition `best_zoom_dot` requires — the two must then agree
        // to floating-point precision, per SEARCH-LATENCY-PLAN.md Phase 5b.
        let q = unit_embed(0.3);
        let c = unit_embed(0.9);
        let via_cos = best_zoom_sim(&q, &c, EMBED_DIM);
        let via_dot = best_zoom_dot(&q, &c, EMBED_DIM);
        assert!((via_cos - via_dot).abs() < 1e-5, "cos={via_cos} dot={via_dot}");
    }

    #[test]
    fn embed_zoom_levels_are_matched_independently_not_flattened() {
        // Same cross-zoom property as gram above, at the embedding's own
        // width — this is what lets a query and a candidate shot at different
        // pixel scales still land in one another's near family.
        let qz0 = unit_embed(0.3)[..EMBED_DIM].to_vec();
        let mut query_embed = qz0.clone();
        query_embed.extend(vec![0.0; EMBED_DIM * (EMBED_ZOOM_LEVELS - 1)]);

        let mut candidate_embed = vec![0.0; EMBED_DIM];
        candidate_embed.extend(qz0);
        candidate_embed.resize(EMBED_TOTAL, 0.0);

        let sim = best_zoom_sim(&query_embed, &candidate_embed, EMBED_DIM);
        assert!(sim > 0.99, "expected near-1.0 cross-zoom match, got {sim}");
    }
}
