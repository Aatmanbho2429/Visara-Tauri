//! Folder browsing for the Browse grid — a file-explorer view restricted to the
//! user's watched library roots (and their subfolders).

use crate::{config::IMAGE_EXTENSIONS, core::database, error::Result};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, MAIN_SEPARATOR},
};

pub fn browse(path: String) -> Value {
    match browse_inner(&path) {
        Ok(v) => v,
        Err(e) => err(format!("Could not open folder: {e}")),
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

fn browse_inner(path: &str) -> Result<Value> {
    let roots = watched_roots();

    // Top level: list the watched roots themselves.
    if path.is_empty() {
        let folders: Vec<Value> = roots
            .iter()
            .map(|r| json!({ "name": r, "path": r }))
            .collect();
        return Ok(json!({
            "success": true, "message": "ok",
            "data": { "current": "", "breadcrumb": [], "folders": folders, "images": [] }
        }));
    }

    let root = match within_root(path, &roots) {
        Some(r) => r.clone(),
        None => return Ok(err("That folder is outside your watched library.".into())),
    };

    let dir = Path::new(path);
    let mut folders: Vec<Value> = Vec::new();
    let mut images: Vec<Value> = Vec::new();

    for entry in fs::read_dir(dir)? {
        let entry = match entry { Ok(e) => e, Err(_) => continue };
        let p = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue; // hidden files (.DS_Store etc.)
        }
        if p.is_dir() {
            folders.push(json!({ "name": name, "path": p.to_string_lossy() }));
        } else if is_image(&p) {
            images.push(json!({ "name": name, "path": p.to_string_lossy() }));
        }
    }

    folders.sort_by(|a, b| name_of(a).to_lowercase().cmp(&name_of(b).to_lowercase()));
    images.sort_by(|a, b| name_of(a).to_lowercase().cmp(&name_of(b).to_lowercase()));

    Ok(json!({
        "success": true, "message": "ok",
        "data": {
            "current":    path,
            "breadcrumb": breadcrumb(&root, path),
            "folders":    folders,
            "images":     images,
        }
    }))
}

/// Breadcrumb from the watched root down to the current folder.
fn breadcrumb(root: &str, current: &str) -> Vec<Value> {
    let mut crumbs = vec![json!({ "name": root, "path": root })];
    let root_n = norm(root);
    let cur_n = norm(current);
    if let Some(rel) = cur_n.strip_prefix(&format!("{root_n}{MAIN_SEPARATOR}")) {
        let mut acc = root_n.clone();
        for seg in rel.split(MAIN_SEPARATOR).filter(|s| !s.is_empty()) {
            acc = format!("{acc}{MAIN_SEPARATOR}{seg}");
            crumbs.push(json!({ "name": seg, "path": acc }));
        }
    }
    crumbs
}

fn name_of(v: &Value) -> String {
    v.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string()
}

fn is_image(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

fn err(msg: String) -> Value {
    json!({ "success": false, "message": msg, "data": null })
}
