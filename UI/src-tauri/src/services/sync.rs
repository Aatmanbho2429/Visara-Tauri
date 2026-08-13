//! Folder indexing — scans a watched folder, hashes/dedupes against the DB,
//! and asks the sidecar to describe every new or changed file. One entry per
//! file now (no more region slicing — see `core::vector_store`).

use crate::{
    core::{database, progress, sidecar, vector_store::VectorStore},
    error::{PictoriaError, Result},
    utils::file_utils,
};
use rayon::prelude::*;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    time::{Instant, SystemTime},
};

#[derive(Debug, Clone)]
pub struct FileError {
    pub file: String,
    pub reason: String,
}

/// Files processed together per sidecar `/describe` call and DB transaction.
const INDEX_FILE_CHUNK: usize = 24;

pub fn sync_folder(store: &mut VectorStore, folder_path: &Path) -> Result<Vec<FileError>> {
    if !sidecar::is_ready() {
        return Err(PictoriaError::ModelNotReady);
    }

    let t_total = Instant::now();
    let folder_str = folder_path.to_string_lossy().to_string();
    let mut errors: Vec<FileError> = Vec::new();

    let con = database::open()?;

    let removed_ids = database::cleanup_missing_in_folder(&con, &folder_str)?;
    if !removed_ids.is_empty() {
        store.remove(&removed_ids);
    }

    let t_scan = Instant::now();
    let current_files = file_utils::scan_images(folder_path);
    let total = current_files.len();
    log::info!(
        "[timing] scan_images folder={folder_str} files={total} scan_ms={:.2}",
        t_scan.elapsed().as_secs_f64() * 1000.0
    );

    progress::set_progress(Some("Scanning files"), Some(0), Some(total), Some(""), Some(0));

    // (path, hash, error, mtime, already_indexed) — already_indexed means an
    // unchanged DB row already exists at this exact path, nothing to do.
    let hash_results: Vec<(PathBuf, Option<String>, Option<String>, f64, bool)> = current_files
        .par_iter()
        .map(|path| {
            let path_str = path.to_string_lossy().to_string();
            let mtime = file_mtime(path);
            let thread_con = match database::open() {
                Ok(c) => c,
                Err(e) => return (path.clone(), None, Some(e.to_string()), mtime, false),
            };
            if let Ok(Some((_, stored_hash, stored_mtime))) = database::find_by_path(&thread_con, &path_str) {
                if (stored_mtime - mtime).abs() < 0.001 {
                    return (path.clone(), Some(stored_hash), None, mtime, true);
                }
            }
            match file_utils::fast_hash(path) {
                Ok(h) => (path.clone(), Some(h), None, mtime, false),
                Err(e) => (path.clone(), None, Some(e.to_string()), mtime, false),
            }
        })
        .collect();

    let mut needs_describe: Vec<(PathBuf, String, f64)> = Vec::new();
    let mut seen_hashes: HashSet<String> = HashSet::new();

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
        if already_indexed {
            continue;
        }

        let (existing_path, existing_id) = match database::find_by_hash(&con, &hash) {
            Ok(v) => v,
            Err(e) => {
                errors.push(FileError { file: path_str, reason: format!("DB lookup failed: {e}") });
                progress::increment_errors();
                continue;
            }
        };

        if let Some(_id) = existing_id {
            if existing_path.as_deref() != Some(&path_str) {
                let existing = existing_path.as_deref().unwrap_or("");
                let in_folder = Path::new(existing).starts_with(folder_path);
                // Same image already indexed elsewhere: same folder → embed a
                // second row; old copy gone → this is a move, rename the row;
                // old copy still exists elsewhere → a genuine cross-folder
                // duplicate, embed its own row too.
                if in_folder {
                    needs_describe.push((path, hash, mtime));
                } else if !Path::new(existing).exists() {
                    if let Err(e) = database::move_file(&con, existing, &path_str) {
                        log::warn!("[sync] move_file failed for {path_str}: {e}; will re-describe");
                        needs_describe.push((path, hash, mtime));
                    }
                } else {
                    needs_describe.push((path, hash, mtime));
                }
            }
        } else {
            if let Ok(Some((old_id, _, _))) = database::find_by_path(&con, &path_str) {
                store.remove(&[old_id]);
                let _ = database::delete_file(&con, &path_str);
            }
            needs_describe.push((path, hash, mtime));
        }
    }

    let total_describe = needs_describe.len();
    if total_describe > 0 {
        index_chunks(store, &con, &needs_describe, &mut errors)?;
    }

    // Files whose hash is no longer present under this folder at all.
    let all_hashes = database::folder_hashes(&con, &folder_str)?;
    let deleted = all_hashes.difference(&seen_hashes).cloned().collect::<HashSet<_>>();
    if !deleted.is_empty() {
        con.execute_batch("BEGIN;")?;
        let to_remove = database::files_by_hashes(&con, &deleted)?;
        let ids: Vec<i64> = to_remove.iter().map(|(_, id)| *id).collect();
        let paths: Vec<&String> = to_remove.iter().map(|(p, _)| p).collect();
        store.remove(&ids);
        for path in paths {
            database::delete_file(&con, path)?;
        }
        con.execute_batch("COMMIT;")?;
    }

    log::info!(
        "[timing] sync_folder TOTAL folder={folder_str} files_scanned={total} \
         files_described={total_describe} errors={} total_ms={:.2}",
        errors.len(),
        t_total.elapsed().as_secs_f64() * 1000.0,
    );
    Ok(errors)
}

/// Describe and persist one batch of new/changed files: colour histograms
/// computed locally in parallel, design descriptors from one sidecar call per
/// chunk (the sidecar itself lets a search jump in between files — see
/// `sidecar/server.py::_run` — so this doesn't need its own yielding).
fn index_chunks(
    store: &mut VectorStore,
    con: &rusqlite::Connection,
    needs_describe: &[(PathBuf, String, f64)],
    errors: &mut Vec<FileError>,
) -> Result<()> {
    let ext_counts: HashMap<String, usize> = {
        let mut m = HashMap::new();
        for (p, _, _) in needs_describe {
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            *m.entry(ext).or_insert(0) += 1;
        }
        m
    };
    progress::set_file_type_totals(ext_counts);
    progress::set_progress(Some("Indexing images"), Some(0), Some(needs_describe.len()), None, None);

    con.execute_batch("BEGIN;")?;
    let mut done_count = 0usize;

    for chunk in needs_describe.chunks(INDEX_FILE_CHUNK) {
        let first_name = chunk[0].0.file_name().and_then(|n| n.to_str()).unwrap_or("");
        progress::set_progress(None, None, None, Some(first_name), None);

        let paths: Vec<String> = chunk.iter().map(|(p, _, _)| p.to_string_lossy().to_string()).collect();
        let t_describe = Instant::now();
        let described = sidecar::describe(&paths, sidecar::Priority::Index)?;
        let describe_ms = t_describe.elapsed().as_secs_f64() * 1000.0;
        let by_path: HashMap<String, Option<sidecar::Descriptor>> = described.into_iter().collect();

        let t_persist = Instant::now();
        for (path, hash, mtime) in chunk.iter() {
            let path_str = path.to_string_lossy().to_string();

            let desc = match by_path.get(&path_str) {
                Some(Some(d)) => d.clone(),
                _ => {
                    errors.push(FileError { file: path_str.clone(), reason: "Sidecar could not describe this image".into() });
                    progress::increment_errors();
                    continue;
                }
            };
            let gram_flat: Vec<f32> = desc.gram.into_iter().flatten().collect();

            let vector_id = match database::next_vector_id(con) {
                Ok(id) => id,
                Err(e) => {
                    errors.push(FileError { file: path_str.clone(), reason: format!("ID allocation failed: {e}") });
                    progress::increment_errors();
                    continue;
                }
            };
            if let Err(e) = database::insert_file(con, &path_str, hash, vector_id, *mtime) {
                errors.push(FileError { file: path_str.clone(), reason: format!("DB insert failed: {e}") });
                progress::increment_errors();
                continue;
            }
            progress::increment_file_type(path.extension().and_then(|e| e.to_str()).unwrap_or(""));

            // Colour tag now, from the descriptor we already have — the sidecar
            // derived it from the same decode. Unconditional is safe here and
            // only here: `insert_file` above is INSERT OR REPLACE, so the row is
            // new and its tags (which cascade off it) were just cleared. Files
            // re-embedded in place keep their rows, and so may carry a manual
            // colour the user set — `reembed_folder` deliberately leaves those
            // alone and lets `tags::backfill_colors` fill only the gaps.
            if !desc.dominant.is_empty() {
                crate::services::tags::apply_color_tags(con, &path_str, &desc.dominant);
            }

            if let Err(e) = store.upsert(vector_id, &desc.rose, &gram_flat, &desc.color) {
                errors.push(FileError { file: path_str, reason: format!("Index upsert failed: {e}") });
                progress::increment_errors();
            }
        }
        let persist_ms = t_persist.elapsed().as_secs_f64() * 1000.0;

        log::info!(
            "[timing] chunk files={} describe_ms={describe_ms:.2} persist_ms={persist_ms:.2} \
             avg_describe_ms_per_file={:.3}",
            chunk.len(),
            describe_ms / chunk.len().max(1) as f64,
        );

        con.execute_batch("COMMIT; BEGIN;")?;
        done_count += chunk.len();
        progress::set_progress(None, Some(done_count), None, None, None);
    }

    con.execute_batch("COMMIT;")?;
    Ok(())
}

fn file_mtime(path: &Path) -> f64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map(|t| t.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs_f64())
        .unwrap_or(0.0)
}

/// Rebuild descriptors **in place** for a folder's already-indexed files,
/// keyed by their existing `faiss_id` — used by the startup re-index
/// migration. Tombstones the folder's existing vectors first so a re-run
/// after an interrupted migration doesn't duplicate entries; leaves `files`
/// rows untouched since Browse tags cascade off them.
pub fn reembed_folder(store: &mut VectorStore, folder_path: &Path) -> Result<Vec<FileError>> {
    if !sidecar::is_ready() {
        return Err(PictoriaError::ModelNotReady);
    }

    let t_total = Instant::now();
    let folder_str = folder_path.to_string_lossy().to_string();
    let con = database::open()?;

    let id_map = database::folder_id_map(&con, &folder_str)?;
    let mut errors: Vec<FileError> = Vec::new();
    if id_map.is_empty() {
        return Ok(errors);
    }

    let existing_ids: Vec<i64> = id_map.keys().copied().collect();
    store.remove(&existing_ids);

    let items: Vec<(i64, PathBuf)> = id_map.into_iter().map(|(id, p)| (id, PathBuf::from(p))).collect();
    let total = items.len();
    progress::set_progress(Some("Rebuilding index"), Some(0), Some(total), Some(""), Some(0));
    let mut done = 0usize;

    for chunk in items.chunks(INDEX_FILE_CHUNK) {
        let first_name = chunk[0].1.file_name().and_then(|n| n.to_str()).unwrap_or("");
        progress::set_progress(None, None, None, Some(first_name), None);

        let paths: Vec<String> = chunk.iter().map(|(_, p)| p.to_string_lossy().to_string()).collect();
        let described = sidecar::describe(&paths, sidecar::Priority::Index)?;
        let by_path: HashMap<String, Option<sidecar::Descriptor>> = described.into_iter().collect();

        for (id, path) in chunk.iter() {
            let path_str = path.to_string_lossy().to_string();
            let desc = match by_path.get(&path_str) {
                Some(Some(d)) => d.clone(),
                _ => {
                    errors.push(FileError { file: path_str, reason: "Sidecar could not describe this image".into() });
                    progress::increment_errors();
                    continue;
                }
            };
            let gram_flat: Vec<f32> = desc.gram.into_iter().flatten().collect();
            if let Err(e) = store.upsert(*id, &desc.rose, &gram_flat, &desc.color) {
                errors.push(FileError { file: path_str, reason: format!("Index upsert failed: {e}") });
                progress::increment_errors();
            }
        }

        done += chunk.len();
        progress::set_progress(None, Some(done), None, None, None);
    }

    log::info!(
        "[timing] reembed_folder TOTAL folder={folder_str} files={total} errors={} total_ms={:.2}",
        errors.len(),
        t_total.elapsed().as_secs_f64() * 1000.0,
    );
    Ok(errors)
}
