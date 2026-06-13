//! Search execution — embeds the query image, queries the vector store across
//! one or more watched folders, and maps IDs back to file paths.
//!
//! Multi-folder model: callers pass a list of folder paths (the scope).  If
//! the list is empty, the scope is automatically every watched folder in the
//! Library.  The watcher keeps each folder's index current, so this function
//! no longer triggers an on-the-fly sync — it just searches the store.

use crate::{
    config::VECTOR_STORE_PATH,
    core::{database, embedder, vector_store::VectorStore},
    error::{Result, PictoriaError},
    utils::image_loader,
};
use serde::Serialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

// ── Result types ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct SearchResult {
    pub rank:       usize,
    pub path:       String,
    pub name:       String,
    pub similarity: f32,
    /// Watched-folder root this result belongs to — handy for the UI to
    /// show "from: 2024" alongside each card.
    pub folder:     String,
}

#[derive(Debug, Serialize, Clone)]
pub struct FailedFile {
    pub file:   String,
    pub reason: String,
}

// ── Public API ────────────────────────────────────────────────────────────

/// Execute a visual similarity search across the given folder scope.
///
/// If `scope` is empty, every watched folder in the Library is searched.
/// All folders must already be indexed (the watcher handles this).
pub fn execute(
    image_path: &Path,
    scope:      &[PathBuf],
    top_k:      usize,
) -> Result<(Vec<SearchResult>, Vec<FailedFile>)> {
    // ── Fatal pre-checks ──────────────────────────────────────────────
    if !embedder::is_ready() {
        return Err(PictoriaError::ModelNotReady);
    }
    if !image_path.exists() {
        return Err(PictoriaError::Fatal(
            "The selected reference image no longer exists.".into(),
        ));
    }

    // ── Resolve scope ─────────────────────────────────────────────────
    let con = database::open()?;
    let scope_paths: Vec<String> = if scope.is_empty() {
        database::watched_folder_paths(&con)?
    } else {
        scope.iter().map(|p| p.to_string_lossy().to_string()).collect()
    };
    drop(con);

    if scope_paths.is_empty() {
        return Err(PictoriaError::Fatal(
            "No folders to search.  Add a folder to your Library first.".into(),
        ));
    }

    // ── Load vector store (acquire IO lock to serialize with watcher) ─
    let _store_guard = crate::core::vector_store::store_io_guard();
    let store = VectorStore::load(VECTOR_STORE_PATH.as_path())?;

    // ── Embed query image ─────────────────────────────────────────────
    // NOTE: search deliberately does not touch the global progress state.
    // That state is owned by the sync/index pipeline and drives the Library
    // page's progress bar; writing "Searching" here (or calling reset()) would
    // corrupt a concurrent folder index — and reading it back in the search
    // command produced the phantom "Indexing…" bar during search.  The search
    // command emits its own lightweight "Searching" snapshot instead.
    let query_img  = image_loader::load_image(image_path)
        .map_err(|e| PictoriaError::Fatal(format!("Could not load reference image: {e}")))?;
    let pixels     = embedder::preprocess(query_img);
    let embeddings = embedder::embed_batch(&pixels, 1)
        .map_err(|e| PictoriaError::Fatal(format!("Failed to embed reference image: {e}")))?;
    let query_emb  = embeddings.into_iter().next().ok_or_else(||
        PictoriaError::Fatal("Embedding returned empty result".into())
    )?;

    // ── Build combined id_map across every folder in scope ────────────
    let con = database::open()?;
    let mut id_map: HashMap<i64, (String, String)> = HashMap::new();
    for folder_str in &scope_paths {
        let folder_id_map = database::folder_id_map(&con, folder_str)?;
        for (id, path) in folder_id_map {
            id_map.insert(id, (path, folder_str.clone()));
        }
    }
    drop(con);

    // ── Vector search.  Over-fetch so the post-scope filter still has
    //    enough rows to fill top_k when the store contains unwatched legacy
    //    vectors.  4× buffer is plenty in practice.
    let raw_k     = top_k.saturating_mul(4).max(top_k);
    let scores    = store.search(&query_emb, raw_k);

    let results: Vec<SearchResult> = scores
        .into_iter()
        .filter_map(|(id, score)| {
            let (path, folder) = id_map.get(&id)?.clone();
            // Skip stale hits whose file was renamed/deleted since indexing —
            // the watcher reconciles these eventually, but never surface a
            // dead path in results in the meantime.
            if !Path::new(&path).exists() {
                return None;
            }
            let name = Path::new(&path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&path)
                .to_string();
            // Score is a cosine similarity in [-1, 1]; clamp and scale to [0, 100].
            let similarity = (score.clamp(-1.0, 1.0) * 100.0 * 10.0).round() / 10.0;
            Some(SearchResult {
                rank: 0, // filled in after truncation
                path,
                name,
                similarity,
                folder,
            })
        })
        .take(top_k)
        .enumerate()
        .map(|(i, mut r)| { r.rank = i + 1; r })
        .collect();

    Ok((results, Vec::new()))
}
