//! Search execution — embeds the query image, syncs the folder if stale,
//! queries the vector store, and maps IDs back to file paths.

use crate::{
    config::VECTOR_STORE_PATH,
    core::{database, embedder, progress, vector_store::VectorStore},
    error::{Result, VisaraError},
    services::sync,
    utils::{file_utils, image_loader},
};
use serde::Serialize;
use std::path::Path;

// ── Result types ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct SearchResult {
    pub rank:       usize,
    pub path:       String,
    pub name:       String,
    pub similarity: f32,
}

#[derive(Debug, Serialize, Clone)]
pub struct FailedFile {
    pub file:   String,
    pub reason: String,
}

// ── Public API ────────────────────────────────────────────────────────────

/// Execute a visual similarity search.
///
/// 1. Validate preconditions (model ready, paths exist).
/// 2. Load the vector store from disk.
/// 3. Sync the target folder if the DB is out of date.
/// 4. Embed the query image.
/// 5. Query the vector store for the top-`k` matches.
/// 6. Map vector IDs back to file paths via the DB.
///
/// Returns `(results, failed_files)`.  Raises `VisaraError` on unrecoverable
/// failures; per-file errors are returned in `failed_files`.
pub fn execute(
    image_path:  &Path,
    folder_path: &Path,
    top_k:       usize,
) -> Result<(Vec<SearchResult>, Vec<FailedFile>)> {
    // ── Fatal pre-checks ──────────────────────────────────────────────
    if !embedder::is_ready() {
        return Err(VisaraError::ModelNotReady);
    }
    if !image_path.exists() {
        return Err(VisaraError::Fatal(
            "The selected reference image no longer exists.".into(),
        ));
    }
    if !folder_path.is_dir() {
        return Err(VisaraError::Fatal(
            "The selected folder no longer exists.".into(),
        ));
    }

    let folder_str = folder_path.to_string_lossy().to_string();
    let mut failed: Vec<FailedFile> = Vec::new();

    // ── Load vector store ─────────────────────────────────────────────
    let mut store = VectorStore::load(VECTOR_STORE_PATH.as_path())?;

    // ── Sync folder if file count has changed ─────────────────────────
    let con       = database::open()?;
    let db_count  = database::folder_file_count(&con, &folder_str)?;
    drop(con);

    let disk_count = file_utils::scan_images(folder_path).len();

    if db_count != disk_count {
        log::info!("Folder out of sync (db={db_count}, disk={disk_count}) — syncing");
        let sync_errors = sync::sync_folder(&mut store, folder_path)?;
        for e in sync_errors {
            failed.push(FailedFile { file: e.file, reason: e.reason });
        }
        store.save(VECTOR_STORE_PATH.as_path())?;
    }

    // ── Embed query image ─────────────────────────────────────────────
    let image_name = image_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("query");

    progress::reset();
    progress::set_progress(
        Some("Searching"),
        Some(0), Some(1),
        Some(image_name),
        Some(0),
    );

    let query_img = image_loader::load_image(image_path)
        .map_err(|e| VisaraError::Fatal(format!("Could not load reference image: {e}")))?;

    let pixels    = embedder::preprocess(query_img);
    let embeddings = embedder::embed_batch(&pixels, 1)
        .map_err(|e| VisaraError::Fatal(format!("Failed to embed reference image: {e}")))?;

    let query_emb = embeddings
        .into_iter()
        .next()
        .ok_or_else(|| VisaraError::Fatal("Embedding returned empty result".into()))?;

    // ── Vector search ─────────────────────────────────────────────────
    let con    = database::open()?;
    let id_map = database::folder_id_map(&con, &folder_str)?;
    drop(con);

    let scores = store.search(&query_emb, top_k);

    let results: Vec<SearchResult> = scores
        .into_iter()
        .enumerate()
        .filter_map(|(rank, (id, score))| {
            let path = id_map.get(&id)?;
            let name = Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(path)
                .to_string();
            // Score is a cosine similarity in [-1, 1]; clamp and scale to [0, 100].
            let similarity = (score.clamp(-1.0, 1.0) * 100.0 * 10.0).round() / 10.0;
            Some(SearchResult {
                rank:       rank + 1,
                path:       path.clone(),
                name,
                similarity,
            })
        })
        .collect();

    progress::reset();
    Ok((results, failed))
}
