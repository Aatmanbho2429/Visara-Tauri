// The exact-match engine's background work: fingerprint and feature backfills, vocabulary training, word assignment.

use crate::{
    config::{
        BACKFILL_STARTUP_WAIT_SECS, MATCH_BACKFILL_CHUNK, MATCH_SHORTLIST, MATCH_VOCAB_ITERS, MATCH_VOCAB_K, MATCH_VOCAB_MIN_IMAGES,
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

// What stage 0b found for one query.
pub struct Shortlist {
    // (vector id, score), best first, at most `MATCH_SHORTLIST`.
    pub hits: Vec<(i64, f32)>,
    // Time spent extracting the query's features (straight and mirrored).
    pub query_features_ms: f64,
    // Time quantising the query and scanning the inverted index.
    pub retrieve_ms: f64,
    // False while no vocabulary exists yet, so the stage was skipped.
    pub vocab_ready: bool,
}

// Stage 0b: extracts the query's features straight and mirrored, quantises both and returns the best `MATCH_SHORTLIST` files by visual-word score.
pub fn retrieve_shortlist(query_path: &str, priority: sidecar::Priority) -> Shortlist {
    let mut out = Shortlist { hits: Vec::new(), query_features_ms: 0.0, retrieve_ms: 0.0, vocab_ready: false };
    let Some(vocab) = current_vocab() else { return out };
    out.vocab_ready = true;

    let t_feat = Instant::now();
    let paths = [query_path.to_string()];
    let mut variants: Vec<Features> = Vec::new();
    for mirror in [false, true] {
        match sidecar::extract_features(&paths, mirror, priority) {
            Ok(rows) => variants.extend(rows.into_iter().filter_map(|(_, r)| r.ok())),
            Err(e) => log::warn!("[match_engine] query feature extraction failed (mirror={mirror}): {e}"),
        }
    }
    out.query_features_ms = t_feat.elapsed().as_secs_f64() * 1000.0;
    if variants.is_empty() {
        return out;
    }

    let t_ret = Instant::now();
    let index = match_index::resident(|| {
        let words = feature_store::with_store(|s| s.words_snapshot()).unwrap_or_default();
        match_index::MatchIndex::build(&words)
    });
    let mut best: HashMap<i64, f32> = HashMap::new();
    for f in &variants {
        let words = match_index::quantize(&f.desc, &vocab);
        for (id, score) in index.query(&words, MATCH_SHORTLIST) {
            let e = best.entry(id).or_insert(score);
            if score > *e {
                *e = score;
            }
        }
    }
    let mut hits: Vec<(i64, f32)> = best.into_iter().collect();
    hits.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0)));
    hits.truncate(MATCH_SHORTLIST);
    out.hits = hits;
    out.retrieve_ms = t_ret.elapsed().as_secs_f64() * 1000.0;
    out
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{core::search_gate, services::search::match_tier};

    // End to end against a live sidecar (`-- --ignored`): transformed copies of img_007 must retrieve it first and verify as copies.
    // Needs PICTORIA_E2E_LIB, PICTORIA_E2E_QUERIES and a sidecar on the configured port.
    #[test]
    #[ignore]
    fn e2e_retrieves_and_verifies_transformed_copies() {
        let home = std::env::temp_dir().join(format!("pictoria_e2e_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        std::env::set_var("HOME", &home);
        let lib = std::env::var("PICTORIA_E2E_LIB").expect("PICTORIA_E2E_LIB");
        let queries = std::env::var("PICTORIA_E2E_QUERIES").expect("PICTORIA_E2E_QUERIES");

        let mut files: Vec<String> =
            std::fs::read_dir(&lib).unwrap().map(|e| e.unwrap().path().to_string_lossy().to_string()).collect();
        files.sort();
        let target = files.iter().position(|p| p.ends_with("img_007.png")).expect("img_007");
        let mut ids = HashMap::new();
        for chunk in files.chunks(MATCH_BACKFILL_CHUNK).collect::<Vec<_>>() {
            let rows = sidecar::extract_features(chunk, false, sidecar::Priority::Index).expect("extract");
            for (path, res) in rows {
                let f = res.expect("features");
                let id = files.iter().position(|p| *p == path).unwrap() as i64;
                assert!(f.n() > 50, "{path} has only {} keypoints", f.n());
                index_file_features(id, Some(&f), None);
                ids.insert(path, id);
            }
        }
        assert_eq!(feature_store::with_store(|s| s.len()).unwrap(), files.len());

        // Vocabulary at the production k; a much smaller k would turn nearly every word into a stop word at this library size.
        let sample = feature_store::with_store(|s| s.sample_descriptors(MATCH_VOCAB_SAMPLE)).unwrap();
        let vocab = sidecar::train_vocab(&sample, MATCH_VOCAB_K, MATCH_VOCAB_ITERS).expect("train_vocab");
        feature_store::with_store(|s| s.save_vocab(&vocab)).unwrap().unwrap();
        assert_eq!(current_vocab().unwrap().len(), vocab.len());
        assign_words();
        assert!(feature_store::with_store(|s| s.ids_missing_words()).unwrap().is_empty());

        let stored_target = files[target].clone();
        for name in ["exact.png", "rot90.png", "mirror.png", "half_jpg.jpg", "crop50.png"] {
            let q = format!("{queries}/{name}");
            let sl = retrieve_shortlist(&q, sidecar::Priority::Search);
            assert!(sl.vocab_ready && !sl.hits.is_empty(), "{name}: empty shortlist");
            let rank = sl.hits.iter().position(|(id, _)| *id == target as i64);
            assert_eq!(rank, Some(0), "{name}: target should be first, got {rank:?} in {:?}", &sl.hits[..3.min(sl.hits.len())]);

            // Verify the top few from stored features, straight then mirrored; only the target may count as a copy.
            let top: Vec<(String, crate::core::feature_store::Features)> = sl.hits.iter().take(5)
                .map(|(id, _)| (files[*id as usize].clone(), feature_store::with_store(|s| s.read(*id)).unwrap().unwrap()))
                .collect();
            let mut copies = Vec::new();
            for mirror in [false, true] {
                for (p, r) in sidecar::verify_features(&q, &top, mirror, sidecar::Priority::Search).unwrap() {
                    if match_tier::is_copy(r.matched, r.inliers, r.inlier_ratio) {
                        copies.push((p, r.inliers, r.mirrored));
                    }
                }
            }
            assert!(copies.iter().any(|(p, _, _)| *p == stored_target), "{name}: target never verified as a copy: {copies:?}");
            assert!(copies.iter().all(|(p, _, _)| *p == stored_target), "{name}: false copy: {copies:?}");
            println!("e2e {name}: rank0 score={:.3} copies={copies:?}", sl.hits[0].1);
        }
        let _ = search_gate::is_active();
    }
}
