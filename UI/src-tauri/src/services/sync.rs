use crate::{
    config::BATCH_SIZE,
    core::{database, embedder, progress, vector_store::VectorStore},
    error::{Result, VisaraError},
    utils::{file_utils, image_loader},
};
use rayon::prelude::*;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Debug, Clone)]
pub struct FileError {
    pub file:   String,
    pub reason: String,
}

pub fn sync_folder(
    store:       &mut VectorStore,
    folder_path: &Path,
) -> Result<Vec<FileError>> {
    if !embedder::is_ready() {
        return Err(VisaraError::ModelNotReady);
    }

    let folder_str = folder_path.to_string_lossy().to_string();
    let mut errors: Vec<FileError> = Vec::new();

    let con = database::open()?;

    // Remove DB rows for files deleted from disk.
    let removed_ids = database::cleanup_missing_in_folder(&con, &folder_str)?;
    if !removed_ids.is_empty() {
        store.remove(&removed_ids);
    }

    // Phase 1: collect files and compute hashes in parallel.
    let current_files = file_utils::scan_images(folder_path);
    let total         = current_files.len();

    progress::set_progress(Some("Scanning files"), Some(0), Some(total), Some(""), Some(0));

    // Tuple: (path, hash, error, mtime, already_indexed).
    // `already_indexed` = the file already has its own DB row at this exact path
    // with an unchanged mtime, so it needs no work at all.
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

        for chunk in needs_embed.chunks(BATCH_SIZE) {
            let first_name = chunk[0].0.file_name().and_then(|n| n.to_str()).unwrap_or("");
            progress::set_progress(None, None, None, Some(first_name), None);

            // Pre-process images in parallel.
            let preprocess_results: Vec<(usize, Result<Vec<f32>>)> = chunk
                .par_iter()
                .enumerate()
                .map(|(i, (path, _, _))| {
                    let res = image_loader::load_image(path).map(embedder::preprocess);
                    (i, res)
                })
                .collect();

            let mut flat_pixels: Vec<f32>   = Vec::new();
            let mut valid_indices: Vec<usize> = Vec::new();

            for (i, result) in preprocess_results {
                match result {
                    Ok(pixels) => {
                        flat_pixels.extend(pixels);
                        valid_indices.push(i);
                    }
                    Err(e) => {
                        let path_str = chunk[i].0.to_string_lossy().to_string();
                        errors.push(FileError { file: path_str, reason: e.to_string() });
                        progress::increment_errors();
                    }
                }
            }

            if !flat_pixels.is_empty() {
                let embeddings = embedder::embed_batch(&flat_pixels, BATCH_SIZE)
                    .map_err(|e| VisaraError::Fatal(format!("Embedding failed: {e}")))?;

                for (emb_idx, chunk_idx) in valid_indices.into_iter().enumerate() {
                    let (path, hash, mtime) = &chunk[chunk_idx];
                    let path_str = path.to_string_lossy().to_string();

                    // Each persistence step is per-file-fatal only: a single
                    // failure is recorded and skipped so the rest of the folder
                    // still indexes (e.g. one locked/duplicate file won't kill
                    // the whole batch).
                    let vector_id = match database::next_vector_id(&con) {
                        Ok(id) => id,
                        Err(e) => {
                            errors.push(FileError { file: path_str, reason: format!("ID allocation failed: {e}") });
                            progress::increment_errors();
                            continue;
                        }
                    };
                    if let Err(e) = store.add(vector_id, &embeddings[emb_idx]) {
                        errors.push(FileError { file: path_str, reason: format!("Index add failed: {e}") });
                        progress::increment_errors();
                        continue;
                    }
                    if let Err(e) = database::insert_file(&con, &path_str, hash, vector_id, *mtime) {
                        errors.push(FileError { file: path_str, reason: format!("DB insert failed: {e}") });
                        progress::increment_errors();
                        continue;
                    }
                    progress::increment_file_type(
                        path.extension().and_then(|e| e.to_str()).unwrap_or(""),
                    );
                }
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

    Ok(errors)
}

fn file_mtime(path: &Path) -> f64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map(|t| t.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs_f64())
        .unwrap_or(0.0)
}
