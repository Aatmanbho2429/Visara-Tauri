//! Custom flat inner-product vector store — a pure-Rust replacement for the
//! Python FAISS `IndexIDMap(IndexFlatIP(768))`.
//!
//! ## Why not FAISS?
//! The FAISS C++ library requires a non-trivial build step and cannot be
//! sandboxed by macOS App Store.  For our workload (<=500 k images, 768-dim
//! L2-normalised embeddings) a brute-force dot-product search over a rayon
//! thread pool is fast enough: ~5 ms per query at 100 k vectors on a modern
//! desktop CPU.
//!
//! ## File format  (`vectors.bin`)
//! ```text
//! [ magic:   8 bytes  "VISARA\x00\x01" ]
//! [ version: 4 bytes  u32 little-endian ]
//! [ dim:     4 bytes  u32 little-endian ]  always 768
//! [ count:   8 bytes  u64 little-endian ]  live (non-tombstone) entries
//! [ padding: 8 bytes                    ]  reserved, must be 0
//! [ entries: (id: i64, emb: f32 x dim)* ]
//! ```
//! Tombstoned entries have `id == i64::MIN`.
//! Compaction rewrites the file removing tombstones when the tombstone ratio
//! exceeds 20 %.

use crate::{config::EMB_DIM, error::{Result, VisaraError}};
use once_cell::sync::Lazy;
use rayon::prelude::*;
use std::{
    collections::HashSet,
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

const MAGIC:      &[u8; 8] = b"VISARA\x00\x01";
const VERSION:    u32      = 1;
const HEADER_LEN: usize    = 32;
const TOMBSTONE:  i64      = i64::MIN;

/// In-memory representation of the vector store.
pub struct VectorStore {
    ids:        Vec<i64>,
    embeddings: Vec<f32>,
    dim:        usize,
}

impl VectorStore {
    // ── Construction ──────────────────────────────────────────────────

    pub fn new() -> Self {
        Self { ids: Vec::new(), embeddings: Vec::new(), dim: EMB_DIM }
    }

    /// Load from disk, or create a fresh store if the file does not exist.
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
            log::warn!("vector store version {version} unsupported — starting fresh");
            return Ok(Self::new());
        }

        let dim = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let body = &bytes[HEADER_LEN..];
        let entry_bytes = 8 + dim * 4;

        if body.len() % entry_bytes != 0 {
            log::warn!("vector store body not aligned — starting fresh");
            return Ok(Self::new());
        }

        let n = body.len() / entry_bytes;
        let mut ids        = Vec::with_capacity(n);
        let mut embeddings = Vec::with_capacity(n * dim);

        for i in 0..n {
            let off = i * entry_bytes;
            let id  = i64::from_le_bytes(body[off..off + 8].try_into().unwrap());
            ids.push(id);
            for j in 0..dim {
                let f_off = off + 8 + j * 4;
                embeddings.push(f32::from_le_bytes(
                    body[f_off..f_off + 4].try_into().unwrap(),
                ));
            }
        }

        Ok(Self { ids, embeddings, dim })
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
            w.write_all(&(self.dim as u32).to_le_bytes())?;
            w.write_all(&live_count.to_le_bytes())?;
            w.write_all(&[0u8; 8])?;

            let entry_bytes = 8 + self.dim * 4;
            let mut buf = vec![0u8; entry_bytes];

            for (idx, &id) in self.ids.iter().enumerate() {
                buf[..8].copy_from_slice(&id.to_le_bytes());
                let slice = &self.embeddings[idx * self.dim..(idx + 1) * self.dim];
                for (j, &v) in slice.iter().enumerate() {
                    buf[8 + j * 4..8 + (j + 1) * 4].copy_from_slice(&v.to_le_bytes());
                }
                w.write_all(&buf)?;
            }
            w.flush()?;
        }

        fs::rename(&tmp, path)?;
        Ok(())
    }

    // ── Mutation ──────────────────────────────────────────────────────

    pub fn add(&mut self, id: i64, emb: &[f32]) -> Result<()> {
        if emb.len() != self.dim {
            return Err(VisaraError::Fatal(format!(
                "embedding dim mismatch: expected {}, got {}",
                self.dim, emb.len()
            )));
        }
        self.ids.push(id);
        self.embeddings.extend_from_slice(emb);
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

    /// Return top-`k` `(id, score)` by inner product, descending.
    /// `query` must be L2-normalised (same convention as CLIP embeddings).
    pub fn search(&self, query: &[f32], k: usize) -> Vec<(i64, f32)> {
        if self.ids.is_empty() || k == 0 {
            return Vec::new();
        }

        let mut scores: Vec<(i64, f32)> = self
            .ids
            .par_iter()
            .enumerate()
            .filter_map(|(i, &id)| {
                if id == TOMBSTONE { return None; }
                let emb = &self.embeddings[i * self.dim..(i + 1) * self.dim];
                let dot: f32 = query.iter().zip(emb).map(|(a, b)| a * b).sum();
                Some((id, dot))
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
        let total     = self.ids.len();
        let tombstones = self.ids.iter().filter(|&&id| id == TOMBSTONE).count();
        if total == 0 || tombstones * 5 < total { return; }

        let mut new_ids  = Vec::with_capacity(total - tombstones);
        let mut new_embs = Vec::with_capacity((total - tombstones) * self.dim);

        for (i, &id) in self.ids.iter().enumerate() {
            if id != TOMBSTONE {
                new_ids.push(id);
                new_embs.extend_from_slice(
                    &self.embeddings[i * self.dim..(i + 1) * self.dim],
                );
            }
        }

        self.ids        = new_ids;
        self.embeddings = new_embs;
        log::debug!(
            "vector store compacted: {} -> {} entries",
            total, self.ids.len()
        );
    }
}

impl Default for VectorStore {
    fn default() -> Self { Self::new() }
}
