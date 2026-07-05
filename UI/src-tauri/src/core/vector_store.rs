//! Custom flat vector store — a pure-Rust replacement for the Python FAISS
//! `IndexIDMap(IndexFlatIP(768))`.
//!
//! ## Two vectors per image (v2)
//! Tiles need to match on **design** (pattern/motif) *and* **colour** (palette),
//! so every entry stores two L2-normalised vectors:
//!   * a **design** vector — the CLIP image embedding (`EMB_DIM` dims)
//!   * a **colour** vector — an HSV bucket histogram (`COLOR_DIM` dims)
//! Search ranks by a weighted blend so results are the same design in a
//! compatible palette:  `score = 0.7·design + 0.3·colour`.
//!
//! ## Why not FAISS?
//! The FAISS C++ library requires a non-trivial build step and cannot be
//! sandboxed by macOS App Store.  For our workload (<=500 k images) a
//! brute-force dot-product over a rayon thread pool is fast enough.
//!
//! ## File format  (`vectors.bin`, version 2)
//! ```text
//! [ magic:      8 bytes  "PICTOR\x00\x01" ]
//! [ version:    4 bytes  u32 little-endian ]  == 2
//! [ design_dim: 4 bytes  u32 little-endian ]  always 768
//! [ count:      8 bytes  u64 little-endian ]  live (non-tombstone) entries
//! [ color_dim:  4 bytes  u32 little-endian ]  always COLOR_DIM
//! [ padding:    4 bytes                    ]  reserved, must be 0
//! [ entries: (id: i64, design: f32 x design_dim, colour: f32 x color_dim)* ]
//! ```
//! Tombstoned entries have `id == i64::MIN`.
//! A version mismatch makes `load` start fresh, which the startup migration
//! turns into a one-time re-index (see `core::migrate`).

use crate::{
    config::EMB_DIM,
    core::color::COLOR_DIM,
    error::{Result, PictoriaError},
};
use once_cell::sync::Lazy;
use rayon::prelude::*;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, OpenOptions},
    io::{BufWriter, Write},
    path::Path,
    sync::{Mutex, MutexGuard},
};

/// Global lock around any load → mutate → save sequence touching `vectors.bin`.
/// Both the search pipeline and the watcher's reconciler acquire this to
/// prevent lost-update races when two threads load+modify+save concurrently.
pub static STORE_IO_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

/// Grab the global vector-store I/O lock.  Hold the returned guard for the
/// duration of any load-modify-save sequence.
pub fn store_io_guard() -> MutexGuard<'static, ()> {
    STORE_IO_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

const MAGIC:      &[u8; 8] = b"PICTOR\x00\x01";
const VERSION:    u32      = 2;
const HEADER_LEN: usize    = 32;
const TOMBSTONE:  i64      = i64::MIN;

/// Blend weights for the design (CLIP) and colour (histogram) similarities.
/// Design leads — "same pattern" matters more than "same palette" — with colour
/// as a strong secondary so results stay within a compatible colour family.
const DESIGN_WEIGHT: f32 = 0.7;
const COLOR_WEIGHT:  f32 = 0.3;

/// In-memory representation of the vector store.
pub struct VectorStore {
    ids:       Vec<i64>,
    design:    Vec<f32>,
    color:     Vec<f32>,
    design_dim: usize,
    color_dim:  usize,
}

impl VectorStore {
    // ── Construction ──────────────────────────────────────────────────

    pub fn new() -> Self {
        Self {
            ids:        Vec::new(),
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
        let entry_bytes = 8 + design_dim * 4 + color_dim * 4;

        if entry_bytes == 8 || body.len() % entry_bytes != 0 {
            log::warn!("vector store body not aligned — starting fresh");
            return Ok(Self::new());
        }

        let n = body.len() / entry_bytes;
        let mut ids    = Vec::with_capacity(n);
        let mut design = Vec::with_capacity(n * design_dim);
        let mut color  = Vec::with_capacity(n * color_dim);

        for i in 0..n {
            let mut off = i * entry_bytes;
            let id = i64::from_le_bytes(body[off..off + 8].try_into().unwrap());
            ids.push(id);
            off += 8;
            for j in 0..design_dim {
                let f = off + j * 4;
                design.push(f32::from_le_bytes(body[f..f + 4].try_into().unwrap()));
            }
            off += design_dim * 4;
            for j in 0..color_dim {
                let f = off + j * 4;
                color.push(f32::from_le_bytes(body[f..f + 4].try_into().unwrap()));
            }
        }

        // Defensive de-duplication.  A corrupt or interrupted write can leave the
        // same id stored more than once (which surfaces as duplicate search
        // results).  Keep only the last occurrence of each id and drop tombstones
        // so the store self-heals on load and is persisted clean on the next save.
        let live   = ids.iter().filter(|&&id| id != TOMBSTONE).count();
        let unique = ids.iter().filter(|&&id| id != TOMBSTONE).collect::<HashSet<_>>().len();
        if unique != live || live != ids.len() {
            let mut last: HashMap<i64, usize> = HashMap::with_capacity(unique);
            for (i, &id) in ids.iter().enumerate() {
                if id != TOMBSTONE { last.insert(id, i); }
            }
            let mut keep: Vec<usize> = last.into_values().collect();
            keep.sort_unstable();
            let mut nids = Vec::with_capacity(keep.len());
            let mut ndesign = Vec::with_capacity(keep.len() * design_dim);
            let mut ncolor  = Vec::with_capacity(keep.len() * color_dim);
            for &i in &keep {
                nids.push(ids[i]);
                ndesign.extend_from_slice(&design[i * design_dim..(i + 1) * design_dim]);
                ncolor.extend_from_slice(&color[i * color_dim..(i + 1) * color_dim]);
            }
            log::warn!(
                "vector store: {} raw entries deduped to {} unique on load",
                ids.len(), nids.len()
            );
            ids = nids; design = ndesign; color = ncolor;
        }

        Ok(Self { ids, design, color, design_dim, color_dim })
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

            let entry_bytes = 8 + self.design_dim * 4 + self.color_dim * 4;
            let mut buf = vec![0u8; entry_bytes];

            for (idx, &id) in self.ids.iter().enumerate() {
                buf[..8].copy_from_slice(&id.to_le_bytes());
                let mut o = 8;
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

    /// Append an entry.  `design` must be `EMB_DIM` long and `color` `COLOR_DIM`
    /// long; both should already be L2-normalised.
    pub fn add(&mut self, id: i64, design: &[f32], color: &[f32]) -> Result<()> {
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
        self.design.extend_from_slice(design);
        self.color.extend_from_slice(color);
        Ok(())
    }

    /// Tombstone entries by ID; compacts automatically when tombstones > 20 %.
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

    /// Return top-`k` `(id, score)` by the blended design+colour similarity,
    /// descending.  Both query vectors must be L2-normalised (same convention as
    /// the stored ones).  When `query_color` is all-zero the colour term simply
    /// drops out and ranking falls back to pure design similarity.
    pub fn search(&self, query_design: &[f32], query_color: &[f32], k: usize) -> Vec<(i64, f32)> {
        if self.ids.is_empty() || k == 0 {
            return Vec::new();
        }

        let use_color = query_color.len() == self.color_dim;

        let mut scores: Vec<(i64, f32)> = self
            .ids
            .par_iter()
            .enumerate()
            .filter_map(|(i, &id)| {
                if id == TOMBSTONE { return None; }

                let d = &self.design[i * self.design_dim..(i + 1) * self.design_dim];
                let design_sim: f32 = query_design.iter().zip(d).map(|(a, b)| a * b).sum();

                let color_sim: f32 = if use_color {
                    let c = &self.color[i * self.color_dim..(i + 1) * self.color_dim];
                    query_color.iter().zip(c).map(|(a, b)| a * b).sum()
                } else {
                    0.0
                };

                let score = DESIGN_WEIGHT * design_sim + COLOR_WEIGHT * color_sim;
                Some((id, score))
            })
            .collect();

        scores.sort_unstable_by(|a, b| {
            b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
        });
        scores.truncate(k);
        scores
    }

    pub fn live_count(&self) -> usize {
        self.ids.iter().filter(|&&id| id != TOMBSTONE).count()
    }

    // ── Compaction ────────────────────────────────────────────────────

    fn maybe_compact(&mut self) {
        let total      = self.ids.len();
        let tombstones = self.ids.iter().filter(|&&id| id == TOMBSTONE).count();
        if total == 0 || tombstones * 5 < total { return; }

        let keep = total - tombstones;
        let mut new_ids    = Vec::with_capacity(keep);
        let mut new_design = Vec::with_capacity(keep * self.design_dim);
        let mut new_color  = Vec::with_capacity(keep * self.color_dim);

        for (i, &id) in self.ids.iter().enumerate() {
            if id != TOMBSTONE {
                new_ids.push(id);
                new_design.extend_from_slice(
                    &self.design[i * self.design_dim..(i + 1) * self.design_dim],
                );
                new_color.extend_from_slice(
                    &self.color[i * self.color_dim..(i + 1) * self.color_dim],
                );
            }
        }

        self.ids    = new_ids;
        self.design = new_design;
        self.color  = new_color;
        log::debug!("vector store compacted: {} -> {} entries", total, self.ids.len());
    }
}

impl Default for VectorStore {
    fn default() -> Self { Self::new() }
}
