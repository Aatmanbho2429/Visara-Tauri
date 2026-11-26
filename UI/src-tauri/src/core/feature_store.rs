// Local SIFT features per file for the exact-match engine, plus the visual-word ids derived from them.
// File layouts, tombstones and the version rule: `.claude/rules/search-pipeline.md` ("Engine files").

use crate::{
    config::{FEATURES_PATH, MATCH_FEATURES_VERSION, VOCAB_PATH, WORDS_PATH},
    error::{PictoriaError, Result},
};
use once_cell::sync::Lazy;
use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};

// SIFT descriptor width (bytes per keypoint).
pub const DESC_DIM: usize = 128;

const FEATURES_MAGIC: &[u8; 8] = b"PICTFEAT";
const WORDS_MAGIC: &[u8; 8] = b"PICTWORD";
const VOCAB_MAGIC: &[u8; 8] = b"PICTVOCB";
const HEADER_LEN: u64 = 16;
// id + w + h + n.
const RECORD_HEAD: u64 = 14;
const TOMBSTONE: i64 = i64::MIN;
// Dead-byte fraction of the file past which it is rewritten.
const COMPACT_DEAD_FRAC: f64 = 0.30;

// Marks a word slot with no assignment (no vocabulary yet).
pub const WORD_UNASSIGNED: u32 = u32::MAX;

// One file's stored features.
#[derive(Debug, Clone, PartialEq)]
pub struct Features {
    pub w: u16,
    pub h: u16,
    // Keypoint coordinates, x then y per point, length `2n`.
    pub pts: Vec<f32>,
    // Descriptor rows, length `128n`.
    pub desc: Vec<u8>,
}

impl Features {
    // Keypoint count.
    pub fn n(&self) -> usize {
        self.pts.len() / 2
    }
}

// Where the three engine files live.
#[derive(Debug, Clone)]
pub struct StorePaths {
    pub features: PathBuf,
    pub words: PathBuf,
    pub vocab: PathBuf,
}

impl StorePaths {
    // The user's `~/.pictoria` files.
    pub fn default_paths() -> Self {
        Self { features: FEATURES_PATH.clone(), words: WORDS_PATH.clone(), vocab: VOCAB_PATH.clone() }
    }

    // The three files inside `dir`, for tests.
    pub fn in_dir(dir: &Path) -> Self {
        Self { features: dir.join("features.bin"), words: dir.join("words.bin"), vocab: dir.join("vocab.bin") }
    }
}

// Positioned read that fills `buf` completely, portable across Unix and Windows.
fn read_exact_at(file: &File, buf: &mut [u8], offset: u64) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::FileExt::read_exact_at(file, buf, offset)
    }
    #[cfg(windows)]
    {
        let mut done = 0usize;
        while done < buf.len() {
            let n = std::os::windows::fs::FileExt::seek_read(file, &mut buf[done..], offset + done as u64)?;
            if n == 0 {
                return Err(std::io::ErrorKind::UnexpectedEof.into());
            }
            done += n;
        }
        Ok(())
    }
}

// Positioned write of all of `buf`, portable across Unix and Windows.
fn write_all_at(file: &File, buf: &[u8], offset: u64) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::FileExt::write_all_at(file, buf, offset)
    }
    #[cfg(windows)]
    {
        let mut done = 0usize;
        while done < buf.len() {
            let n = std::os::windows::fs::FileExt::seek_write(file, &buf[done..], offset + done as u64)?;
            done += n;
        }
        Ok(())
    }
}

// Bytes one record occupies for `n` keypoints.
fn record_len(n: u64) -> u64 {
    RECORD_HEAD + n * (8 + DESC_DIM as u64)
}

// Header bytes for a file kind: magic, version, one reserved/dim field.
fn header(magic: &[u8; 8], extra: u32) -> [u8; 16] {
    let mut h = [0u8; 16];
    h[..8].copy_from_slice(magic);
    h[8..12].copy_from_slice(&MATCH_FEATURES_VERSION.to_le_bytes());
    h[12..16].copy_from_slice(&extra.to_le_bytes());
    h
}

// Where a live record sits in features.bin.
#[derive(Debug, Clone, Copy)]
struct Slot {
    offset: u64,
    n: u32,
}

pub struct FeatureStore {
    paths: StorePaths,
    file: File,
    index: HashMap<i64, Slot>,
    file_len: u64,
    dead_bytes: u64,
    words: HashMap<i64, Vec<u32>>,
    words_dirty: bool,
}

impl FeatureStore {
    // Opens (creating if absent) the store at `paths`; a version mismatch or unreadable header drops all three files first.
    pub fn open(paths: StorePaths) -> Result<Self> {
        if let Some(parent) = paths.features.parent() {
            fs::create_dir_all(parent)?;
        }
        if !Self::header_ok(&paths.features, FEATURES_MAGIC) {
            if paths.features.exists() {
                log::warn!("[feature_store] features.bin header/version mismatch — dropping engine files for a rebuild");
            }
            for p in [&paths.features, &paths.words, &paths.vocab] {
                let _ = fs::remove_file(p);
            }
        }
        let fresh = !paths.features.exists();
        let file = OpenOptions::new().read(true).write(true).create(true).truncate(false).open(&paths.features)?;
        if fresh {
            write_all_at(&file, &header(FEATURES_MAGIC, DESC_DIM as u32), 0)?;
        }
        let mut store = Self {
            paths,
            file,
            index: HashMap::new(),
            file_len: HEADER_LEN,
            dead_bytes: 0,
            words: HashMap::new(),
            words_dirty: false,
        };
        store.scan()?;
        store.load_words();
        Ok(store)
    }

    // True when `path` starts with `magic` and the current features version (a missing file counts as fine).
    fn header_ok(path: &Path, magic: &[u8; 8]) -> bool {
        let Ok(file) = File::open(path) else { return true };
        let mut buf = [0u8; 16];
        if read_exact_at(&file, &mut buf, 0).is_err() {
            return false;
        }
        &buf[..8] == magic
            && u32::from_le_bytes(buf[8..12].try_into().unwrap()) == MATCH_FEATURES_VERSION
            && u32::from_le_bytes(buf[12..16].try_into().unwrap()) == DESC_DIM as u32
    }

    // Rebuilds the offset table by reading record headers only; a torn tail is truncated and a duplicate id keeps its later record.
    fn scan(&mut self) -> Result<()> {
        let len = self.file.metadata()?.len();
        let mut off = HEADER_LEN;
        let mut head = [0u8; RECORD_HEAD as usize];
        self.index.clear();
        self.dead_bytes = 0;
        let mut dups: Vec<u64> = Vec::new();
        while off + RECORD_HEAD <= len {
            if read_exact_at(&self.file, &mut head, off).is_err() {
                break;
            }
            let id = i64::from_le_bytes(head[..8].try_into().unwrap());
            let n = u16::from_le_bytes(head[12..14].try_into().unwrap()) as u64;
            let rec = record_len(n);
            if off + rec > len {
                break;
            }
            if id == TOMBSTONE {
                self.dead_bytes += rec;
            } else if let Some(old) = self.index.insert(id, Slot { offset: off, n: n as u32 }) {
                self.dead_bytes += record_len(old.n as u64);
                dups.push(old.offset);
            }
            off += rec;
        }
        if off < len {
            log::warn!("[feature_store] truncating {} torn trailing bytes", len - off);
            self.file.set_len(off)?;
        }
        for d in dups {
            write_all_at(&self.file, &TOMBSTONE.to_le_bytes(), d)?;
        }
        self.file_len = off;
        Ok(())
    }

    // Reads words.bin if present and current; anything else leaves the words empty (the backfill reassigns them).
    fn load_words(&mut self) {
        self.words.clear();
        let Ok(bytes) = fs::read(&self.paths.words) else { return };
        if bytes.len() < 16 || &bytes[..8] != WORDS_MAGIC || u32::from_le_bytes(bytes[8..12].try_into().unwrap()) != MATCH_FEATURES_VERSION {
            let _ = fs::remove_file(&self.paths.words);
            return;
        }
        let mut off = 16usize;
        while off + 10 <= bytes.len() {
            let id = i64::from_le_bytes(bytes[off..off + 8].try_into().unwrap());
            let n = u16::from_le_bytes(bytes[off + 8..off + 10].try_into().unwrap()) as usize;
            off += 10;
            if off + n * 4 > bytes.len() {
                break;
            }
            let words = (0..n).map(|i| u32::from_le_bytes(bytes[off + i * 4..off + i * 4 + 4].try_into().unwrap())).collect();
            off += n * 4;
            // Words of a file whose features are gone are stale.
            if self.index.contains_key(&id) {
                self.words.insert(id, words);
            }
        }
    }

    // Number of files with stored features.
    pub fn len(&self) -> usize {
        self.index.len()
    }

    // True when no file has stored features.
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    // Every id with stored features, ascending.
    pub fn ids(&self) -> Vec<i64> {
        let mut ids: Vec<i64> = self.index.keys().copied().collect();
        ids.sort_unstable();
        ids
    }

    // True when `id` has stored features.
    pub fn contains(&self, id: i64) -> bool {
        self.index.contains_key(&id)
    }

    // Appends a new record for `id` and tombstones the old one; keypoints are x,y pairs and descriptors 128-byte rows.
    pub fn upsert(&mut self, id: i64, w: u16, h: u16, pts: &[f32], desc: &[u8]) -> Result<()> {
        let n = pts.len() / 2;
        if pts.len() % 2 != 0 || desc.len() != n * DESC_DIM || n > u16::MAX as usize {
            return Err(PictoriaError::Fatal(format!("feature shape mismatch: {} pts floats, {} desc bytes", pts.len(), desc.len())));
        }
        let mut buf = Vec::with_capacity(record_len(n as u64) as usize);
        buf.extend_from_slice(&id.to_le_bytes());
        buf.extend_from_slice(&w.to_le_bytes());
        buf.extend_from_slice(&h.to_le_bytes());
        buf.extend_from_slice(&(n as u16).to_le_bytes());
        for v in pts {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        buf.extend_from_slice(desc);
        let offset = self.file_len;
        write_all_at(&self.file, &buf, offset)?;
        self.file_len += buf.len() as u64;
        if let Some(old) = self.index.insert(id, Slot { offset, n: n as u32 }) {
            write_all_at(&self.file, &TOMBSTONE.to_le_bytes(), old.offset)?;
            self.dead_bytes += record_len(old.n as u64);
        }
        // New features invalidate any words computed from the old ones.
        if self.words.remove(&id).is_some() {
            self.words_dirty = true;
        }
        self.maybe_compact()
    }

    // Tombstones every id in `ids` (and drops their words); returns how many had features.
    pub fn remove(&mut self, ids: &[i64]) -> usize {
        let mut removed = 0;
        for id in ids {
            if let Some(slot) = self.index.remove(id) {
                if write_all_at(&self.file, &TOMBSTONE.to_le_bytes(), slot.offset).is_ok() {
                    self.dead_bytes += record_len(slot.n as u64);
                    removed += 1;
                }
            }
            if self.words.remove(id).is_some() {
                self.words_dirty = true;
            }
        }
        if removed > 0 {
            if let Err(e) = self.maybe_compact() {
                log::warn!("[feature_store] compaction failed: {e}");
            }
        }
        removed
    }

    // Reads one file's features with positioned reads; `None` when absent or unreadable.
    pub fn read(&self, id: i64) -> Option<Features> {
        let slot = *self.index.get(&id)?;
        let n = slot.n as usize;
        let mut head = [0u8; RECORD_HEAD as usize];
        read_exact_at(&self.file, &mut head, slot.offset).ok()?;
        let w = u16::from_le_bytes(head[8..10].try_into().unwrap());
        let h = u16::from_le_bytes(head[10..12].try_into().unwrap());
        let mut pts_bytes = vec![0u8; n * 8];
        read_exact_at(&self.file, &mut pts_bytes, slot.offset + RECORD_HEAD).ok()?;
        let mut desc = vec![0u8; n * DESC_DIM];
        read_exact_at(&self.file, &mut desc, slot.offset + RECORD_HEAD + (n * 8) as u64).ok()?;
        let pts = pts_bytes.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
        Some(Features { w, h, pts, desc })
    }

    // Rewrites features.bin without its dead records once they exceed `COMPACT_DEAD_FRAC` of the file.
    fn maybe_compact(&mut self) -> Result<()> {
        if self.file_len == 0 || (self.dead_bytes as f64) <= COMPACT_DEAD_FRAC * self.file_len as f64 {
            return Ok(());
        }
        let tmp = self.paths.features.with_extension("bin.tmp");
        let mut live: Vec<(i64, Slot)> = self.index.iter().map(|(&id, &s)| (id, s)).collect();
        live.sort_by_key(|(_, s)| s.offset);
        let mut new_index = HashMap::with_capacity(live.len());
        {
            let mut out = File::create(&tmp)?;
            out.write_all(&header(FEATURES_MAGIC, DESC_DIM as u32))?;
            let mut pos = HEADER_LEN;
            for (id, slot) in &live {
                let mut rec = vec![0u8; record_len(slot.n as u64) as usize];
                read_exact_at(&self.file, &mut rec, slot.offset)?;
                out.write_all(&rec)?;
                new_index.insert(*id, Slot { offset: pos, n: slot.n });
                pos += rec.len() as u64;
            }
            out.flush()?;
        }
        fs::rename(&tmp, &self.paths.features)?;
        self.file = OpenOptions::new().read(true).write(true).open(&self.paths.features)?;
        self.file_len = self.file.metadata()?.len();
        self.index = new_index;
        self.dead_bytes = 0;
        log::debug!("[feature_store] compacted to {} records, {} bytes", self.index.len(), self.file_len);
        Ok(())
    }

    // ── Words ─────────────────────────────────────────────────────────

    // Ids with stored features but no usable visual words (never assigned, or still `WORD_UNASSIGNED`), ascending.
    pub fn ids_missing_words(&self) -> Vec<i64> {
        let mut ids: Vec<i64> = self
            .index
            .keys()
            .copied()
            .filter(|id| self.words.get(id).is_none_or(|w| w.contains(&WORD_UNASSIGNED)))
            .collect();
        ids.sort_unstable();
        ids
    }

    // Stores `id`'s visual words; ignored when the id has no features.
    pub fn set_words(&mut self, id: i64, words: &[u32]) {
        if self.index.contains_key(&id) {
            self.words.insert(id, words.to_vec());
            self.words_dirty = true;
        }
    }

    // Every file's words, for building the inverted index (files with unassigned slots are left out).
    pub fn words_snapshot(&self) -> HashMap<i64, Vec<u32>> {
        self.words
            .iter()
            .filter(|(_, w)| !w.contains(&WORD_UNASSIGNED))
            .map(|(&id, w)| (id, w.clone()))
            .collect()
    }

    // Drops every file's words, e.g. after the vocabulary is retrained.
    pub fn clear_words(&mut self) {
        if !self.words.is_empty() {
            self.words.clear();
            self.words_dirty = true;
        }
    }

    // Persists words.bin atomically if it changed; returns whether a write happened.
    pub fn save(&mut self) -> Result<bool> {
        if !self.words_dirty {
            return Ok(false);
        }
        let tmp = self.paths.words.with_extension("bin.tmp");
        {
            let mut out = std::io::BufWriter::new(File::create(&tmp)?);
            out.write_all(&header(WORDS_MAGIC, 0))?;
            let mut ids: Vec<&i64> = self.words.keys().collect();
            ids.sort_unstable();
            for id in ids {
                let w = &self.words[id];
                out.write_all(&id.to_le_bytes())?;
                out.write_all(&(w.len() as u16).to_le_bytes())?;
                for v in w {
                    out.write_all(&v.to_le_bytes())?;
                }
            }
            out.flush()?;
        }
        fs::rename(&tmp, &self.paths.words)?;
        self.words_dirty = false;
        Ok(true)
    }

    // ── Vocabulary ────────────────────────────────────────────────────

    // Saves the `k x 128` centroids to vocab.bin atomically.
    pub fn save_vocab(&self, centroids: &[f32]) -> Result<()> {
        if centroids.is_empty() || centroids.len() % DESC_DIM != 0 {
            return Err(PictoriaError::Fatal(format!("vocab length {} is not a multiple of {DESC_DIM}", centroids.len())));
        }
        let tmp = self.paths.vocab.with_extension("bin.tmp");
        {
            let mut out = std::io::BufWriter::new(File::create(&tmp)?);
            out.write_all(&header(VOCAB_MAGIC, (centroids.len() / DESC_DIM) as u32))?;
            for v in centroids {
                out.write_all(&v.to_le_bytes())?;
            }
            out.flush()?;
        }
        fs::rename(&tmp, &self.paths.vocab)?;
        Ok(())
    }

    // Loads the centroids (flat `k x 128`), or `None` when there is no current vocabulary.
    pub fn load_vocab(&self) -> Option<Vec<f32>> {
        let bytes = fs::read(&self.paths.vocab).ok()?;
        if bytes.len() < 16 || &bytes[..8] != VOCAB_MAGIC || u32::from_le_bytes(bytes[8..12].try_into().unwrap()) != MATCH_FEATURES_VERSION {
            return None;
        }
        let k = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        if bytes.len() != 16 + k * DESC_DIM * 4 {
            return None;
        }
        Some(bytes[16..].chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect())
    }

    // Up to `max` descriptor rows (flat bytes), taken evenly from every file so a few big files cannot dominate the vocabulary.
    pub fn sample_descriptors(&self, max: usize) -> Vec<u8> {
        if self.index.is_empty() || max == 0 {
            return Vec::new();
        }
        let per_file = (max / self.index.len()).max(1);
        let mut slots: Vec<(&i64, &Slot)> = self.index.iter().collect();
        slots.sort_by_key(|(_, s)| s.offset);
        let mut out = Vec::with_capacity(max.min(self.index.len() * per_file) * DESC_DIM);
        for (_, slot) in slots {
            let n = slot.n as usize;
            if n == 0 {
                continue;
            }
            let mut desc = vec![0u8; n * DESC_DIM];
            if read_exact_at(&self.file, &mut desc, slot.offset + RECORD_HEAD + (n * 8) as u64).is_err() {
                continue;
            }
            let take = per_file.min(n);
            for j in 0..take {
                let row = j * n / take;
                out.extend_from_slice(&desc[row * DESC_DIM..(row + 1) * DESC_DIM]);
            }
            if out.len() / DESC_DIM >= max {
                break;
            }
        }
        out.truncate(max * DESC_DIM);
        out
    }
}

// The process-wide store; every indexer, backfill and search goes through this one lock.
static GLOBAL: Lazy<Mutex<Option<FeatureStore>>> = Lazy::new(|| Mutex::new(None));

// Locks the global store, opening it on first use. Hold the guard only for the call at hand.
pub fn with_store<T>(f: impl FnOnce(&mut FeatureStore) -> T) -> Result<T> {
    let mut guard: MutexGuard<Option<FeatureStore>> = GLOBAL.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(FeatureStore::open(StorePaths::default_paths())?);
    }
    Ok(f(guard.as_mut().expect("opened above")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("fs_test_{}_{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    // Deterministic features for `n` keypoints, distinct per seed.
    fn feats(seed: u8, n: usize) -> (Vec<f32>, Vec<u8>) {
        let pts = (0..n * 2).map(|i| seed as f32 * 1000.0 + i as f32 * 0.5).collect();
        let desc = (0..n * DESC_DIM).map(|i| (i as u8).wrapping_add(seed)).collect();
        (pts, desc)
    }

    fn put(s: &mut FeatureStore, id: i64, seed: u8, n: usize) {
        let (p, d) = feats(seed, n);
        s.upsert(id, 640 + seed as u16, 480, &p, &d).unwrap();
    }

    // What goes in comes out, and survives reopening the file.
    #[test]
    fn round_trips_through_disk() {
        let dir = tmp("rt");
        let paths = StorePaths::in_dir(&dir);
        let mut s = FeatureStore::open(paths.clone()).unwrap();
        put(&mut s, 7, 1, 40);
        put(&mut s, 8, 2, 0);
        put(&mut s, -3, 3, 5);
        let (p, d) = feats(1, 40);
        assert_eq!(s.read(7), Some(Features { w: 641, h: 480, pts: p, desc: d }));
        assert_eq!(s.read(8).unwrap().n(), 0);
        assert!(s.read(99).is_none());
        drop(s);
        let s = FeatureStore::open(paths).unwrap();
        assert_eq!(s.len(), 3);
        assert_eq!(s.read(-3).unwrap().n(), 5);
        assert_eq!(s.read(7).unwrap().desc, feats(1, 40).1);
    }

    // A second upsert replaces the first: one live record, the new payload, the old one tombstoned across reopen.
    #[test]
    fn upsert_replaces() {
        let dir = tmp("up");
        let paths = StorePaths::in_dir(&dir);
        let mut s = FeatureStore::open(paths.clone()).unwrap();
        put(&mut s, 1, 1, 10);
        put(&mut s, 1, 9, 20);
        assert_eq!(s.len(), 1);
        assert_eq!(s.read(1).unwrap().n(), 20);
        assert_eq!(s.read(1).unwrap().w, 649);
        drop(s);
        let s = FeatureStore::open(paths).unwrap();
        assert_eq!((s.len(), s.read(1).unwrap().n()), (1, 20));
    }

    // Removal tombstones; compaction past 30% dead shrinks the file and every surviving read still lands on its own bytes.
    #[test]
    fn tombstone_compaction_and_offset_reads() {
        let dir = tmp("cp");
        let paths = StorePaths::in_dir(&dir);
        let mut s = FeatureStore::open(paths.clone()).unwrap();
        for id in 0..10i64 {
            put(&mut s, id, id as u8, 8 + id as usize);
        }
        let before = fs::metadata(&paths.features).unwrap().len();
        assert_eq!(s.remove(&[0, 1, 2]), 3);
        assert_eq!(s.remove(&[1000]), 0);
        // 3 of 10 similar-size records dead is ~25%: not yet compacted.
        assert_eq!(fs::metadata(&paths.features).unwrap().len(), before);
        s.remove(&[3, 4]);
        let after = fs::metadata(&paths.features).unwrap().len();
        assert!(after < before, "compaction should shrink {before} -> {after}");
        assert_eq!(s.len(), 5);
        for id in 5..10i64 {
            let f = s.read(id).unwrap();
            let (p, d) = feats(id as u8, 8 + id as usize);
            assert_eq!((f.pts, f.desc), (p, d), "record {id} after compaction");
        }
        assert!(s.read(2).is_none());
        // New appends after compaction land at the right offset and survive a reopen.
        put(&mut s, 50, 50, 6);
        drop(s);
        let s = FeatureStore::open(paths).unwrap();
        assert_eq!(s.len(), 6);
        assert_eq!(s.read(50).unwrap().desc, feats(50, 6).1);
        assert_eq!(s.read(9).unwrap().desc, feats(9, 17).1);
    }

    // A file written under another version is dropped along with words and vocab, and the store starts empty.
    #[test]
    fn version_mismatch_drops_the_files() {
        let dir = tmp("ver");
        let paths = StorePaths::in_dir(&dir);
        let mut s = FeatureStore::open(paths.clone()).unwrap();
        put(&mut s, 1, 1, 4);
        s.set_words(1, &[1, 2, 3, 4]);
        s.save().unwrap();
        s.save_vocab(&vec![0.5; DESC_DIM * 2]).unwrap();
        drop(s);
        let mut bytes = fs::read(&paths.features).unwrap();
        bytes[8..12].copy_from_slice(&(MATCH_FEATURES_VERSION + 1).to_le_bytes());
        fs::write(&paths.features, bytes).unwrap();
        let s = FeatureStore::open(paths.clone()).unwrap();
        assert!(s.is_empty());
        assert!(!paths.words.exists() && !paths.vocab.exists());
        assert_eq!(fs::metadata(&paths.features).unwrap().len(), HEADER_LEN);
    }

    // A half-written trailing record is cut off instead of poisoning the scan.
    #[test]
    fn torn_tail_is_truncated() {
        let dir = tmp("torn");
        let paths = StorePaths::in_dir(&dir);
        let mut s = FeatureStore::open(paths.clone()).unwrap();
        put(&mut s, 1, 1, 4);
        put(&mut s, 2, 2, 4);
        drop(s);
        let len = fs::metadata(&paths.features).unwrap().len();
        OpenOptions::new().write(true).open(&paths.features).unwrap().set_len(len - 100).unwrap();
        let s = FeatureStore::open(paths).unwrap();
        assert_eq!(s.len(), 1);
        assert!(s.read(1).is_some() && s.read(2).is_none());
    }

    // Words persist, follow their file's removal, reset on upsert, and unassigned slots count as missing.
    #[test]
    fn words_lifecycle() {
        let dir = tmp("words");
        let paths = StorePaths::in_dir(&dir);
        let mut s = FeatureStore::open(paths.clone()).unwrap();
        put(&mut s, 1, 1, 3);
        put(&mut s, 2, 2, 3);
        put(&mut s, 3, 3, 2);
        assert_eq!(s.ids_missing_words(), vec![1, 2, 3]);
        s.set_words(1, &[5, 6, 7]);
        s.set_words(2, &[WORD_UNASSIGNED, 1, 2]);
        s.set_words(999, &[1]);
        assert_eq!(s.ids_missing_words(), vec![2, 3]);
        assert!(s.save().unwrap());
        assert!(!s.save().unwrap(), "clean store writes nothing");
        drop(s);
        let mut s = FeatureStore::open(paths).unwrap();
        assert_eq!(s.words_snapshot().get(&1), Some(&vec![5, 6, 7]));
        assert!(!s.words_snapshot().contains_key(&2));
        put(&mut s, 1, 9, 3);
        assert_eq!(s.ids_missing_words(), vec![1, 2, 3]);
        s.set_words(1, &[1, 1, 1]);
        s.remove(&[1]);
        assert!(s.words_snapshot().is_empty());
    }

    // The vocabulary round-trips and a wrong-length file reads as absent.
    #[test]
    fn vocab_round_trip() {
        let dir = tmp("vocab");
        let paths = StorePaths::in_dir(&dir);
        let s = FeatureStore::open(paths.clone()).unwrap();
        assert!(s.load_vocab().is_none());
        let c: Vec<f32> = (0..DESC_DIM * 3).map(|i| i as f32).collect();
        s.save_vocab(&c).unwrap();
        assert_eq!(s.load_vocab(), Some(c));
        assert!(s.save_vocab(&[1.0; 5]).is_err());
        let mut bytes = fs::read(&paths.vocab).unwrap();
        bytes.truncate(bytes.len() - 4);
        fs::write(&paths.vocab, bytes).unwrap();
        assert!(s.load_vocab().is_none());
    }

    // Sampling takes rows from every file, never more than asked, and each row is a real descriptor row.
    #[test]
    fn sample_is_uniform_over_files() {
        let dir = tmp("sample");
        let mut s = FeatureStore::open(StorePaths::in_dir(&dir)).unwrap();
        put(&mut s, 1, 10, 100);
        put(&mut s, 2, 20, 3);
        put(&mut s, 3, 30, 50);
        let sample = s.sample_descriptors(12);
        // 12 / 3 files = 4 each, but the 3-row file only has 3.
        assert_eq!(sample.len(), 11 * DESC_DIM);
        let rows: Vec<&[u8]> = sample.chunks(DESC_DIM).collect();
        for seed in [10u8, 20, 30] {
            assert!(rows.iter().any(|r| (r[0] == seed || r[0] == seed.wrapping_add(128)) && (0..DESC_DIM).all(|j| r[j] == r[0].wrapping_add(j as u8))));
        }
        assert!(s.sample_descriptors(0).is_empty());
        assert!(FeatureStore::open(StorePaths::in_dir(&tmp("sample_empty"))).unwrap().sample_descriptors(10).is_empty());
    }
}
