// Folder browsing for the Browse grid — a file-explorer view restricted to the
// user's watched library roots (and their subfolders).

use crate::{
    config::IMAGE_EXTENSIONS,
    core::database,
    error::Result,
    models::response::{ApiResponse, BrowseEntry, ResponseBrowseDirectory},
};
use std::{
    fs,
    path::{Path, MAIN_SEPARATOR},
};

pub fn browse(path: String) -> ApiResponse<ResponseBrowseDirectory> {
    match browse_inner(&path) {
        Ok(v) => v,
        Err(e) => ApiResponse::err(500, format!("Could not open folder: {e}")),
    }
}

fn watched_roots() -> Vec<String> {
    database::open()
        .and_then(|c| database::list_watched_folders(&c))
        .map(|v| v.into_iter().map(|f| f.path).collect())
        .unwrap_or_default()
}

fn norm(p: &str) -> String {
    p.trim_end_matches(MAIN_SEPARATOR).to_string()
}

fn within_root<'a>(path: &str, roots: &'a [String]) -> Option<&'a String> {
    let p = norm(path);
    roots.iter().find(|r| {
        let rn = norm(r);
        p == rn || p.starts_with(&format!("{rn}{MAIN_SEPARATOR}"))
    })
}

fn browse_inner(path: &str) -> Result<ApiResponse<ResponseBrowseDirectory>> {
    let roots = watched_roots();

    // Top level: list the watched roots themselves.
    if path.is_empty() {
        let folders: Vec<BrowseEntry> = roots
            .iter()
            .map(|r| BrowseEntry { name: r.clone(), path: r.clone() })
            .collect();
        return Ok(ApiResponse::ok(ResponseBrowseDirectory {
            current: String::new(),
            breadcrumb: Vec::new(),
            folders,
            images: Vec::new(),
        }));
    }

    let root = match within_root(path, &roots) {
        Some(r) => r.clone(),
        None => return Ok(ApiResponse::err(403, "That folder is outside your watched library.")),
    };

    let dir = Path::new(path);
    let mut folders: Vec<BrowseEntry> = Vec::new();
    let mut images: Vec<BrowseEntry> = Vec::new();

    for entry in fs::read_dir(dir)? {
        let entry = match entry { Ok(e) => e, Err(_) => continue };
        let p = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue; // hidden files (.DS_Store etc.)
        }
        if p.is_dir() {
            folders.push(BrowseEntry { name, path: p.to_string_lossy().to_string() });
        } else if is_image(&p) {
            images.push(BrowseEntry { name, path: p.to_string_lossy().to_string() });
        }
    }

    folders.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    images.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    Ok(ApiResponse::ok(ResponseBrowseDirectory {
        current: path.to_string(),
        breadcrumb: breadcrumb(&root, path),
        folders,
        images,
    }))
}

// Breadcrumb from the watched root down to the current folder.
fn breadcrumb(root: &str, current: &str) -> Vec<BrowseEntry> {
    let mut crumbs = vec![BrowseEntry { name: root.to_string(), path: root.to_string() }];
    let root_n = norm(root);
    let cur_n = norm(current);
    if let Some(rel) = cur_n.strip_prefix(&format!("{root_n}{MAIN_SEPARATOR}")) {
        let mut acc = root_n.clone();
        for seg in rel.split(MAIN_SEPARATOR).filter(|s| !s.is_empty()) {
            acc = format!("{acc}{MAIN_SEPARATOR}{seg}");
            crumbs.push(BrowseEntry { name: seg.to_string(), path: acc.clone() });
        }
    }
    crumbs
}

fn is_image(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}
