// The exact-match engine's background work: fingerprint and feature backfills, vocabulary training, word assignment.

use crate::{
    config::{
        BACKFILL_STARTUP_WAIT_SECS, MATCH_BACKFILL_CHUNK, MATCH_VOCAB_ITERS, MATCH_VOCAB_K, MATCH_VOCAB_MIN_IMAGES,
        MATCH_VOCAB_SAMPLE, MATCH_WORD_ASSIGN_CHUNK, SIDECAR_HEALTH_POLL_MS,
    },
    core::{
        database,
        feature_store::{self, Features},
        match_index, progress, search_gate, sidecar,
    },
    services::sync,
};
use once_cell::sync::Lazy;
use rayon::prelude::*;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant},
};

// Serialises backfills so the post-sync trigger and the startup trigger never do the same work twice.
static BACKFILL_LOCK: Mutex<()> = Mutex::new(());

// The trained vocabulary (flat k x 128 f32), loaded from vocab.bin on first use.
static VOCAB: Lazy<RwLock<Option<Arc<Vec<f32>>>>> = Lazy::new(|| RwLock::new(None));

// The current vocabulary, or `None` while none has been trained yet.
pub fn current_vocab() -> Option<Arc<Vec<f32>>> {
    if let Some(v) = VOCAB.read().unwrap_or_else(|e| e.into_inner()).clone() {
        return Some(v);
    }
    let loaded = feature_store::with_store(|s| s.load_vocab()).ok().flatten().map(Arc::new)?;
    *VOCAB.write().unwrap_or_else(|e| e.into_inner()) = Some(loaded.clone());
    Some(loaded)
}

// Extracts features for a chunk of paths at index priority; a sidecar failure is logged and yields nothing, never an error.
pub fn extract_chunk(paths: &[String]) -> HashMap<String, Features> {
    match sidecar::extract_features(paths, false, sidecar::Priority::Index) {
        Ok(rows) => rows.into_iter().filter_map(|(p, r)| r.ok().map(|f| (p, f))).collect(),
        Err(e) => {
            log::warn!("[match_engine] feature extraction failed for a chunk of {}: {e}", paths.len());
            HashMap::new()
        }
    }
}

// Stores one file's features (and its words when a vocabulary exists); with no features it drops any stale record for the id.
pub fn index_file_features(id: i64, features: Option<&Features>, vocab: Option<&[f32]>) {
    let words = features.zip(vocab).map(|(f, v)| match_index::quantize(&f.desc, v));
    let res = feature_store::with_store(|s| match features {
        Some(f) => {
            s.upsert(id, f.w, f.h, &f.pts, &f.desc)?;
            if let Some(w) = &words {
                s.set_words(id, w);
            }
            Ok(())
        }
        None => {
            s.remove(&[id]);
            Ok(())
        }
    });
    if let Err(e) = res.and_then(|r: crate::error::Result<()>| r) {
        log::warn!("[match_engine] storing features for id {id} failed: {e}");
    }
}

// Removes engine data for vector ids whose files are gone, then persists and drops the cached index.
pub fn remove_ids(ids: &[i64]) {
    if ids.is_empty() {
        return;
    }
    let _ = feature_store::with_store(|s| {
        if s.remove(ids) > 0 {
            let _ = s.save();
        }
    });
    match_index::invalidate_resident();
}

// Persists words.bin and drops the cached inverted index when words changed.
pub fn flush_words() {
    if let Ok(Ok(true)) = feature_store::with_store(|s| s.save()) {
        match_index::invalidate_resident();
    }
}

// Drops engine data for ids with no `files` row (purged folders, tombstoned deletes).
fn prune_orphans() {
    let Ok(live) = database::open().and_then(|c| database::all_vector_ids(&c)) else { return };
    let orphans: Vec<i64> = match feature_store::with_store(|s| s.ids()) {
        Ok(ids) => ids.into_iter().filter(|id| !live.contains(id)).collect(),
        Err(_) => return,
    };
    if !orphans.is_empty() {
        log::info!("[match_engine] pruning {} orphaned feature records", orphans.len());
        remove_ids(&orphans);
    }
}

// Extracts and stores features for every file under `folder` that has none yet; blocking. Returns how many were stored.
pub fn backfill_features(folder: &str) -> usize {
    let todo: Vec<(i64, String)> = match database::open().and_then(|c| database::folder_id_map(&c, folder)) {
        Ok(map) => {
            let mut v: Vec<(i64, String)> = map.into_iter().collect();
            v.sort();
            let missing = feature_store::with_store(|s| v.iter().map(|(id, _)| !s.contains(*id)).collect::<Vec<bool>>()).unwrap_or_default();
            v.into_iter().zip(missing).filter(|(_, m)| *m).map(|(kv, _)| kv).collect()
        }
        Err(e) => {
            log::warn!("[match_engine] feature backfill: cannot list files: {e}");
            return 0;
        }
    };
    if todo.is_empty() {
        return 0;
    }
    if !sidecar::is_healthy() {
        log::info!("[match_engine] feature backfill: sidecar not ready, skipping {} files under {folder}", todo.len());
        return 0;
    }
    log::info!("[match_engine] feature backfill: {} files under {folder}", todo.len());
    let t_total = Instant::now();
    let vocab = current_vocab();
    let (mut done, mut paused) = (0usize, 0usize);
    for chunk in todo.chunks(MATCH_BACKFILL_CHUNK) {
        if search_gate::is_active() {
            paused += 1;
            search_gate::wait_while_active();
        }
        let paths: Vec<String> = chunk.iter().map(|(_, p)| p.clone()).collect();
        let by_path = match sidecar::extract_features(&paths, false, sidecar::Priority::Index) {
            Ok(rows) => rows,
            Err(e) => {
                log::warn!("[match_engine] feature backfill: sidecar call failed: {e}");
                break;
            }
        };
        let ids: HashMap<&str, i64> = chunk.iter().map(|(id, p)| (p.as_str(), *id)).collect();
        for (path, res) in by_path {
            // A decode failure stores nothing, so a later pass retries it.
            let (Some(&id), Ok(f)) = (ids.get(path.as_str()), res) else { continue };
            index_file_features(id, Some(&f), vocab.as_deref().map(|v| v.as_slice()));
            done += 1;
        }
    }
    flush_words();
    log::info!(
        "[timing] feature backfill done: {done} of {} files under {folder} paused_for_search={paused} total_ms={:.2}",
        todo.len(),
        t_total.elapsed().as_secs_f64() * 1000.0,
    );
    done
}

// Trains the vocabulary once enough files have features, then assigns words to every file missing them; blocking, in the background.
pub fn ensure_vocab() {
    let total = match database::open().and_then(|c| database::file_count(&c)) {
        Ok(n) => n,
        Err(_) => return,
    };
    if current_vocab().is_none() {
        let enough = feature_store::with_store(|s| total > 0 && s.len() >= MATCH_VOCAB_MIN_IMAGES.min(total)).unwrap_or(false);
        if !enough || !sidecar::is_healthy() {
            return;
        }
        search_gate::wait_while_active();
        let t0 = Instant::now();
        let sample = match feature_store::with_store(|s| s.sample_descriptors(MATCH_VOCAB_SAMPLE)) {
            Ok(s) if !s.is_empty() => s,
            _ => return,
        };
        let n = sample.len() / feature_store::DESC_DIM;
        let centroids = match sidecar::train_vocab(&sample, MATCH_VOCAB_K, MATCH_VOCAB_ITERS) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("[match_engine] vocabulary training failed: {e}");
                return;
            }
        };
        let saved = feature_store::with_store(|s| {
            s.save_vocab(&centroids)?;
            s.clear_words();
            Ok::<(), crate::error::PictoriaError>(())
        });
        if !matches!(saved, Ok(Ok(()))) {
            log::warn!("[match_engine] could not save the vocabulary");
            return;
        }
        let k = centroids.len() / feature_store::DESC_DIM;
        *VOCAB.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(centroids));
        match_index::invalidate_resident();
        log::info!("[timing] vocab train: sample_n={n} k={k} iters={MATCH_VOCAB_ITERS} total_ms={:.2}", t0.elapsed().as_secs_f64() * 1000.0);
    }
    assign_words();
}

// Quantises every stored file that has no words yet; blocking.
pub fn assign_words() {
    let Some(vocab) = current_vocab() else { return };
    let todo = feature_store::with_store(|s| s.ids_missing_words()).unwrap_or_default();
    if todo.is_empty() {
        return;
    }
    let t0 = Instant::now();
    let mut assigned = 0usize;
    for (step, chunk) in todo.chunks(MATCH_WORD_ASSIGN_CHUNK).enumerate() {
        search_gate::wait_while_active();
        let feats: Vec<(i64, Features)> = match feature_store::with_store(|s| chunk.iter().filter_map(|&id| s.read(id).map(|f| (id, f))).collect()) {
            Ok(f) => f,
            Err(_) => break,
        };
        let words: Vec<(i64, Vec<u32>)> = feats.par_iter().map(|(id, f)| (*id, match_index::quantize(&f.desc, &vocab))).collect();
        let _ = feature_store::with_store(|s| {
            for (id, w) in &words {
                s.set_words(*id, w);
            }
        });
        assigned += words.len();
        // Persist now and then so a quit mid-way keeps most of the work.
        if step % 64 == 63 {
            flush_words();
        }
    }
    flush_words();
    log::info!("[timing] word assign: files={assigned} total_ms={:.2}", t0.elapsed().as_secs_f64() * 1000.0);
}

// Runs every backfill for one folder (blocking); call from a spawned thread.
pub fn backfill_folder(folder: &str) {
    let _guard = BACKFILL_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    sync::backfill_fingerprints(folder);
    backfill_features(folder);
    prune_orphans();
    ensure_vocab();
}

// At launch, once the sidecar is up, backfills every watched folder regardless of whether a sync runs.
pub fn spawn_startup_backfill() {
    std::thread::spawn(|| {
        let deadline = Instant::now() + Duration::from_secs(BACKFILL_STARTUP_WAIT_SECS);
        while !sidecar::is_healthy() {
            if Instant::now() >= deadline {
                log::info!("[match_engine] startup backfill: sidecar never came up, skipping this launch");
                return;
            }
            std::thread::sleep(Duration::from_millis(SIDECAR_HEALTH_POLL_MS));
        }
        let folders = match database::open().and_then(|c| database::watched_folder_paths(&c)) {
            Ok(f) => f,
            Err(e) => {
                log::warn!("[match_engine] startup backfill: cannot list folders: {e}");
                return;
            }
        };
        for folder in folders {
            // A running sync holds the DB write lock for seconds at a time, so let it finish first.
            let sync_deadline = Instant::now() + Duration::from_secs(BACKFILL_STARTUP_WAIT_SECS);
            while progress::get_progress().active && Instant::now() < sync_deadline {
                std::thread::sleep(Duration::from_secs(2));
            }
            backfill_folder(&folder);
        }
    });
}
