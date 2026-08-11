use crate::{
    config::BATCH_SIZE,
    core::{database, embedder, progress, regions::{self, Region}, vector_store::VectorStore},
    error::{Result, PictoriaError},
    utils::{file_utils, image_loader},
};
use rayon::prelude::*;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    time::{Instant, SystemTime},
};

// ── Temporary benchmarking instrumentation ──────────────────────────────────
//
// Logs a `[timing]`-tagged line at every pipeline stage boundary so a folder
// load can be broken down cost-by-code-block after the fact (grep the Tauri
// log for "[timing]"). Cheap (Instant::now() + a log line per file/chunk) —
// safe to leave compiled in, but flagged here in case it should be stripped
// or gated behind a debug-only cfg later.

#[derive(Debug, Clone)]
pub struct FileError {
    pub file:   String,
    pub reason: String,
}

/// Files pre-processed together before an inference batch is dispatched.
///
/// Deliberately smaller than [`BATCH_SIZE`]: each file now expands into up to
/// `regions::MAX_REGIONS` tensors of 224x224x3 f32 (~600 KB each), so chunking
/// by BATCH_SIZE files would hold hundreds of megabytes of pixel data at once.
/// Inference is still dispatched in BATCH_SIZE-sized batches — this only bounds
/// how much is staged in memory ahead of it.
const INDEX_FILE_CHUNK: usize = 8;

/// One region of an image, ready to embed.
type PreparedRegion = (Region, Vec<f32>, Vec<f32>);

/// Load an image at slicing resolution and pre-process every planned region:
/// the whole frame plus overlapping windows (see [`crate::core::regions`]).
///
/// Returns `(region, model input tensor, colour histogram)` per region.  The
/// colour histogram is computed per region so a partial match reports the
/// palette of the part that actually matched, not of the whole sheet.
fn prepare_regions(path: &Path) -> Result<Vec<PreparedRegion>> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let file_kb = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0) as f64 / 1024.0;

    // Detailed load: slices are cut from this, so a 512px reduction would leave
    // each window with less source than the 224px the model wants.
    let t_decode = Instant::now();
    let img = image_loader::load_image_detailed(path)?;
    let decode_ms = t_decode.elapsed().as_secs_f64() * 1000.0;
    let (w, h) = (img.width(), img.height());

    let t_prep = Instant::now();
    let plan = regions::plan(w, h);
    let n_regions = plan.len();
    let mut out: Vec<Option<PreparedRegion>> = vec![None; plan.len()];

    for (i, r) in plan.iter().enumerate() {
        if r.is_whole() {
            continue; // handled below, so it can consume `img` without a copy
        }
        let (x, y, cw, ch) = r.to_pixels(w, h);
        let crop   = img.crop_imm(x, y, cw, ch);
        let color  = crate::core::color::histogram(&crop);
        let pixels = embedder::preprocess_grayscale(crop);
        out[i] = Some((*r, pixels, color));
    }

    if let Some(i) = plan.iter().position(|r| r.is_whole()) {
        let color  = crate::core::color::histogram(&img);
        let pixels = embedder::preprocess_grayscale(img);
        out[i] = Some((plan[i], pixels, color));
    }
    let prep_ms = t_prep.elapsed().as_secs_f64() * 1000.0;

    log::debug!(
        "[timing] decode file={:?} ext={ext} dim={w}x{h} size_kb={file_kb:.1} \
         regions={n_regions} decode_ms={decode_ms:.2} region_prep_ms={prep_ms:.2} \
         total_ms={:.2}",
        path.file_name().unwrap_or_default(),
        decode_ms + prep_ms,
    );

    Ok(out.into_iter().flatten().collect())
}

pub fn sync_folder(
    store:       &mut VectorStore,
    folder_path: &Path,
) -> Result<Vec<FileError>> {
    if !embedder::is_ready() {
        return Err(PictoriaError::ModelNotReady);
    }

    let t_total = Instant::now();
    let folder_str = folder_path.to_string_lossy().to_string();
    let mut errors: Vec<FileError> = Vec::new();

    let con = database::open()?;

    // Remove DB rows for files deleted from disk.
    let removed_ids = database::cleanup_missing_in_folder(&con, &folder_str)?;
    if !removed_ids.is_empty() {
        store.remove(&removed_ids);
    }

    // Phase 1: collect files and compute hashes in parallel.
    let t_scan = Instant::now();
    let current_files = file_utils::scan_images(folder_path);
    let total         = current_files.len();
    log::info!(
        "[timing] scan_images folder={folder_str} files={total} scan_ms={:.2}",
        t_scan.elapsed().as_secs_f64() * 1000.0
    );

    progress::set_progress(Some("Scanning files"), Some(0), Some(total), Some(""), Some(0));

    // Tuple: (path, hash, error, mtime, already_indexed).
    // `already_indexed` = the file already has its own DB row at this exact path
    // with an unchanged mtime, so it needs no work at all.
    let t_hash = Instant::now();
    let hash_results: Vec<(PathBuf, Option<String>, Option<String>, f64, bool)> = current_files
        .par_iter()
        .map(|path| {
            let path_str = path.to_string_lossy().to_string();
            let mtime    = file_mtime(path);

            let thread_con = match database::open() {
                Ok(c)  => c,
                Err(e) => return (path.clone(), None, Some(e.to_string()), mtime, false),
            };

            if let Ok(Some((_, stored_hash, stored_mtime))) =
                database::find_by_path(&thread_con, &path_str)
            {
                if (stored_mtime - mtime).abs() < 0.001 {
                    // Already indexed at this path, unchanged → flag to skip.
                    return (path.clone(), Some(stored_hash), None, mtime, true);
                }
            }

            match file_utils::fast_hash(path) {
                Ok(h)  => (path.clone(), Some(h), None, mtime, false),
                Err(e) => (path.clone(), None, Some(e.to_string()), mtime, false),
            }
        })
        .collect();
    log::info!(
        "[timing] hash_phase folder={folder_str} files={total} hash_ms={:.2}",
        t_hash.elapsed().as_secs_f64() * 1000.0
    );

    // Classify: already indexed / needs embedding / error.
    let mut needs_embed: Vec<(PathBuf, String, f64)> = Vec::new();
    let mut seen_hashes: HashSet<String>              = HashSet::new();

    for (i, (path, hash_opt, err_opt, mtime, already_indexed)) in hash_results.into_iter().enumerate() {
        let path_str = path.to_string_lossy().to_string();
        progress::set_progress(Some("Scanning files"), Some(i + 1), None, Some(&path_str), None);

        if let Some(reason) = err_opt {
            errors.push(FileError { file: path_str, reason: format!("Hash failed: {reason}") });
            progress::increment_errors();
            continue;
        }

        let hash = hash_opt.unwrap();
        seen_hashes.insert(hash.clone());

        // Already indexed at this exact path with an unchanged mtime → it is
        // done; leave it untouched.  This is what stops every duplicate copy
        // from being re-embedded on every sync: find_by_hash below only points
        // at one copy of a shared image, so without this guard all the *other*
        // copies look like they still need embedding.  Skipping here also means
        // an unchanged folder reads no files, so it can't bump access-times and
        // re-trigger the watcher.
        if already_indexed {
            continue;
        }

        // Per-file DB lookup must never abort the whole folder — record and skip.
        let (existing_path, existing_id) = match database::find_by_hash(&con, &hash) {
            Ok(v)  => v,
            Err(e) => {
                errors.push(FileError { file: path_str, reason: format!("DB lookup failed: {e}") });
                progress::increment_errors();
                continue;
            }
        };

        if let Some(_id) = existing_id {
            if existing_path.as_deref() != Some(&path_str) {
                let existing  = existing_path.as_deref().unwrap_or("");
                let in_folder = Path::new(existing).starts_with(folder_path);

                // Same image already indexed elsewhere on disk:
                //  • in this folder        → embed a fresh copy for this path
                //  • old copy gone         → rename (move) the existing row
                //  • old copy still exists  → it's a genuine duplicate across
                //    folders, so embed a separate row for this path too.
                if in_folder {
                    needs_embed.push((path, hash, mtime));
                } else if !Path::new(existing).exists() {
                    if let Err(e) = database::move_file(&con, existing, &path_str) {
                        log::warn!("[sync] move_file failed for {path_str}: {e}; will re-embed");
                        needs_embed.push((path, hash, mtime));
                    }
                } else {
                    needs_embed.push((path, hash, mtime));
                }
            }
        } else {
            if let Ok(Some((old_id, _, _))) = database::find_by_path(&con, &path_str) {
                store.remove(&[old_id]);
                let _ = database::delete_file(&con, &path_str);
            }
            needs_embed.push((path, hash, mtime));
        }
    }

    // Phase 2: embed and persist new/changed files.
    let total_embed = needs_embed.len();

    if total_embed > 0 {
        let ext_counts: HashMap<String, usize> = {
            let mut m: HashMap<String, usize> = HashMap::new();
            for (p, _, _) in &needs_embed {
                let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                *m.entry(ext).or_insert(0) += 1;
            }
            m
        };
        progress::set_file_type_totals(ext_counts);
        progress::set_progress(Some("Indexing images"), Some(0), Some(total_embed), None, None);

        // Start the first batch transaction.
        // rusqlite is in autocommit mode by default (unlike Python's sqlite3),
        // so we must issue BEGIN explicitly before using COMMIT; BEGIN; per batch.
        con.execute_batch("BEGIN;")?;

        let mut done_count = 0usize;
        let mut chunk_no    = 0usize;

        for chunk in needs_embed.chunks(INDEX_FILE_CHUNK) {
            chunk_no += 1;
            let first_name = chunk[0].0.file_name().and_then(|n| n.to_str()).unwrap_or("");
            progress::set_progress(None, None, None, Some(first_name), None);

            // Pre-process images in parallel.  Each worker loads the image once
            // and derives, for every region of it, BOTH vectors: the model pixel
            // tensor (design) and the HSV colour histogram (palette).
            let t_preprocess = Instant::now();
            let preprocess_results: Vec<(usize, Result<Vec<PreparedRegion>>)> = chunk
                .par_iter()
                .enumerate()
                .map(|(i, (path, _, _))| (i, prepare_regions(path)))
                .collect();
            let preprocess_wall_ms = t_preprocess.elapsed().as_secs_f64() * 1000.0;

            let mut flat_pixels: Vec<f32> = Vec::new();
            // (chunk index, region, colour vector) for every successfully
            // pre-processed region, in the same order their pixels were appended
            // to flat_pixels.  One file contributes several entries here.
            let mut valid: Vec<(usize, Region, Vec<f32>)> = Vec::new();

            for (i, result) in preprocess_results {
                match result {
                    Ok(prepared) => {
                        for (region, pixels, color) in prepared {
                            flat_pixels.extend(pixels);
                            valid.push((i, region, color));
                        }
                    }
                    Err(e) => {
                        let path_str = chunk[i].0.to_string_lossy().to_string();
                        errors.push(FileError { file: path_str, reason: e.to_string() });
                        progress::increment_errors();
                    }
                }
            }

            let n_vectors = flat_pixels.len() / (3 * crate::config::CLIP_INPUT_SIZE as usize * crate::config::CLIP_INPUT_SIZE as usize);

            if !flat_pixels.is_empty() {
                let t_embed = Instant::now();
                let embeddings = embedder::embed_batch(&flat_pixels, BATCH_SIZE)
                    .map_err(|e| PictoriaError::Fatal(format!("Embedding failed: {e}")))?;
                let embed_ms = t_embed.elapsed().as_secs_f64() * 1000.0;

                // All regions of a file share its vector id, so the row is
                // created once — on the file's first region — and reused.
                // `Err` marks a file whose persistence failed, so its remaining
                // regions are skipped rather than added against no DB row.
                let mut file_ids: HashMap<usize, std::result::Result<i64, ()>> = HashMap::new();

                let t_persist = Instant::now();
                for (emb_idx, (chunk_idx, region, color)) in valid.into_iter().enumerate() {
                    let (path, hash, mtime) = &chunk[chunk_idx];
                    let path_str = path.to_string_lossy().to_string();

                    // Each persistence step is per-file-fatal only: a single
                    // failure is recorded and skipped so the rest of the folder
                    // still indexes (e.g. one locked/duplicate file won't kill
                    // the whole batch).
                    let id_entry = file_ids.entry(chunk_idx).or_insert_with(|| {
                        let vector_id = match database::next_vector_id(&con) {
                            Ok(id) => id,
                            Err(e) => {
                                errors.push(FileError { file: path_str.clone(), reason: format!("ID allocation failed: {e}") });
                                progress::increment_errors();
                                return Err(());
                            }
                        };
                        if let Err(e) = database::insert_file(&con, &path_str, hash, vector_id, *mtime) {
                            errors.push(FileError { file: path_str.clone(), reason: format!("DB insert failed: {e}") });
                            progress::increment_errors();
                            return Err(());
                        }
                        progress::increment_file_type(
                            path.extension().and_then(|e| e.to_str()).unwrap_or(""),
                        );
                        Ok(vector_id)
                    });

                    let vector_id = match id_entry {
                        Ok(id) => *id,
                        Err(()) => continue,
                    };

                    if let Err(e) = store.add(vector_id, region, &embeddings[emb_idx], &color) {
                        errors.push(FileError { file: path_str, reason: format!("Index add failed: {e}") });
                        progress::increment_errors();
                        continue;
                    }
                }
                let persist_ms = t_persist.elapsed().as_secs_f64() * 1000.0;

                log::info!(
                    "[timing] chunk #{chunk_no} files={} vectors={n_vectors} \
                     preprocess_wall_ms={preprocess_wall_ms:.2} embed_ms={embed_ms:.2} \
                     persist_ms={persist_ms:.2} avg_embed_ms_per_vector={:.3}",
                    chunk.len(),
                    embed_ms / n_vectors.max(1) as f64,
                );
            }

            // Commit this batch and immediately open the next transaction.
            con.execute_batch("COMMIT; BEGIN;")?;

            done_count += chunk.len();
            progress::set_progress(None, Some(done_count), None, None, None);
        }

        // Commit the final (possibly empty) open transaction.
        con.execute_batch("COMMIT;")?;
    }

    // Phase 3: remove hashes that are no longer on disk.
    let all_hashes = database::folder_hashes(&con, &folder_str)?;
    let deleted    = all_hashes.difference(&seen_hashes).cloned().collect::<HashSet<_>>();

    if !deleted.is_empty() {
        con.execute_batch("BEGIN;")?;
        let to_remove = database::files_by_hashes(&con, &deleted)?;
        let ids: Vec<i64>       = to_remove.iter().map(|(_, id)| *id).collect();
        let paths: Vec<&String> = to_remove.iter().map(|(p, _)| p).collect();
        store.remove(&ids);
        for path in paths {
            database::delete_file(&con, path)?;
        }
        con.execute_batch("COMMIT;")?;
    }

    log::info!(
        "[timing] sync_folder TOTAL folder={folder_str} files_scanned={total} \
         files_embedded={total_embed} errors={} total_ms={:.2}",
        errors.len(),
        t_total.elapsed().as_secs_f64() * 1000.0,
    );

    Ok(errors)
}

fn file_mtime(path: &Path) -> f64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map(|t| t.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs_f64())
        .unwrap_or(0.0)
}

/// Rebuild vectors **in place** for a folder's already-indexed files, keyed by
/// their existing `faiss_id`.  Used by the startup re-index migration: the
/// preprocessing / colour vector (or model) changed, so every stored vector must
/// be recomputed — but WITHOUT touching the `files` rows, whose ids anchor the
/// Browse tags via the `ON DELETE CASCADE` foreign key.
///
/// Idempotent: it first tombstones the folder's existing vectors, so a re-run
/// after an interrupted migration doesn't duplicate entries.
pub fn reembed_folder(
    store:       &mut VectorStore,
    folder_path: &Path,
) -> Result<Vec<FileError>> {
    if !embedder::is_ready() {
        return Err(PictoriaError::ModelNotReady);
    }

    let folder_str = folder_path.to_string_lossy().to_string();
    let con        = database::open()?;

    // faiss_id → path for every indexed file under this folder.
    let id_map = database::folder_id_map(&con, &folder_str)?;
    let mut errors: Vec<FileError> = Vec::new();
    if id_map.is_empty() {
        return Ok(errors);
    }

    // Drop any stale vectors for this folder so a re-run is idempotent.
    let existing_ids: Vec<i64> = id_map.keys().copied().collect();
    store.remove(&existing_ids);

    let items: Vec<(i64, PathBuf)> = id_map
        .into_iter()
        .map(|(id, p)| (id, PathBuf::from(p)))
        .collect();
    let total = items.len();

    progress::set_progress(Some("Rebuilding index"), Some(0), Some(total), Some(""), Some(0));
    let mut done = 0usize;

    for chunk in items.chunks(INDEX_FILE_CHUNK) {
        let first_name = chunk[0].1.file_name().and_then(|n| n.to_str()).unwrap_or("");
        progress::set_progress(None, None, None, Some(first_name), None);

        // Load once, derive every region's vectors, in parallel.
        let pre: Vec<(usize, Result<Vec<PreparedRegion>>)> = chunk
            .par_iter()
            .enumerate()
            .map(|(i, (_, path))| (i, prepare_regions(path)))
            .collect();

        let mut flat_pixels: Vec<f32> = Vec::new();
        let mut valid: Vec<(usize, Region, Vec<f32>)> = Vec::new();

        for (i, res) in pre {
            match res {
                Ok(prepared) => {
                    for (region, pixels, color) in prepared {
                        flat_pixels.extend(pixels);
                        valid.push((i, region, color));
                    }
                }
                Err(e) => {
                    errors.push(FileError {
                        file:   chunk[i].1.to_string_lossy().to_string(),
                        reason: e.to_string(),
                    });
                    progress::increment_errors();
                }
            }
        }

        if !flat_pixels.is_empty() {
            let embeddings = embedder::embed_batch(&flat_pixels, BATCH_SIZE)
                .map_err(|e| PictoriaError::Fatal(format!("Embedding failed: {e}")))?;

            for (emb_idx, (chunk_idx, region, color)) in valid.into_iter().enumerate() {
                let (faiss_id, path) = &chunk[chunk_idx];
                if let Err(e) = store.add(*faiss_id, region, &embeddings[emb_idx], &color) {
                    errors.push(FileError {
                        file:   path.to_string_lossy().to_string(),
                        reason: format!("Index add failed: {e}"),
                    });
                    progress::increment_errors();
                }
            }
        }

        done += chunk.len();
        progress::set_progress(None, Some(done), None, None, None);
    }

    Ok(errors)
}
