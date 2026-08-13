//! Tagging service — auto colour, manual multi-select tags, suggestions, and
//! tag-based querying.  Each public fn returns a `serde_json::Value` in the
//! `BaseResponse` envelope, matching the other services.

use crate::core::{database, search_gate, sidecar};
use serde_json::{json, Value};
use std::collections::BTreeSet;

/// Categories that hold a single value per file (setting one replaces the old).
const SINGLE_VALUED: &[&str] = &["color", "size", "material", "finish", "design"];

fn is_single(category: &str) -> bool {
    SINGLE_VALUED.contains(&category)
}

// ── Mutations ───────────────────────────────────────────────────────────────

/// Apply one tag to every selected image.
pub fn set_tags(paths: Vec<String>, category: String, value: String) -> Value {
    let value = value.trim().to_string();
    if value.is_empty() {
        return err("Tag value cannot be empty.".into());
    }

    let con = match database::open() {
        Ok(c) => c,
        Err(e) => return err(format!("Database error: {e}")),
    };

    let mut ok = 0usize;
    for p in &paths {
        let res = if is_single(&category) {
            database::set_single_tag(&con, p, &category, &value, "manual")
        } else {
            database::add_tag(&con, p, &category, &value, "manual")
        };
        match res {
            Ok(_) => ok += 1,
            Err(e) => log::warn!("[tags] set failed for {p}: {e}"),
        }
    }

    json!({
        "success": true,
        "message": format!("Tagged {ok} image{}", if ok == 1 { "" } else { "s" }),
        "data": null,
    })
}

/// Remove a specific tag from every selected image.
pub fn remove_tag(paths: Vec<String>, category: String, value: String) -> Value {
    let con = match database::open() {
        Ok(c) => c,
        Err(e) => return err(format!("Database error: {e}")),
    };
    for p in &paths {
        if let Err(e) = database::remove_tag(&con, p, &category, &value) {
            log::warn!("[tags] remove failed for {p}: {e}");
        }
    }
    json!({ "success": true, "message": "Tag removed.", "data": null })
}

// ── Reads ────────────────────────────────────────────────────────────────────

/// All tags for a set of paths (used by the Categorize panel to show current tags).
pub fn get_tags(paths: Vec<String>) -> Value {
    match database::open().and_then(|c| database::tags_for_paths(&c, &paths)) {
        Ok(tags) => json!({ "success": true, "message": "ok", "data": { "tags": tags } }),
        Err(e) => err(format!("Could not read tags: {e}")),
    }
}

/// Distinct (category, value) tags with counts — drives the filter chips.
pub fn facets() -> Value {
    match database::open().and_then(|c| database::tag_facets(&c)) {
        Ok(facets) => json!({ "success": true, "message": "ok", "data": { "facets": facets } }),
        Err(e) => err(format!("Could not read tags: {e}")),
    }
}

/// Paths matching ALL of the given (category, value) filters.
pub fn query(filters: Vec<(String, String)>) -> Value {
    match database::open().and_then(|c| database::query_paths_by_tags(&c, &filters)) {
        Ok(paths) => json!({ "success": true, "message": "ok", "data": { "paths": paths } }),
        Err(e) => err(format!("Could not query tags: {e}")),
    }
}

/// Suggested tags for a selection, derived from file/folder names (not pixels).
pub fn suggest(paths: Vec<String>) -> Value {
    let mut set: BTreeSet<(String, String)> = BTreeSet::new();
    for p in &paths {
        let lower = p.to_lowercase();
        if let Some(sz) = extract_size(&lower) { set.insert(("size".into(), sz)); }
        if let Some(m) = detect_material(&lower) { set.insert(("material".into(), m.into())); }
        if let Some(f) = detect_finish(&lower) { set.insert(("finish".into(), f.into())); }
        if let Some(d) = detect_design(&lower) { set.insert(("design".into(), d.into())); }
    }
    let suggestions: Vec<Value> = set
        .into_iter()
        .map(|(category, value)| json!({ "category": category, "value": value }))
        .collect();
    json!({ "success": true, "message": "ok", "data": { "suggestions": suggestions } })
}

// ── Colour backfill ──────────────────────────────────────────────────────────

/// Files per sidecar `/describe` call, and the granularity at which the backfill
/// checks whether it should pause for a search.
const BACKFILL_CHUNK: usize = 24;

/// Write the auto colour tag(s) for one path. Primary bucket replaces the
/// category, any runner-up is added alongside.
pub fn apply_color_tags(con: &rusqlite::Connection, path: &str, colors: &[String]) {
    for (i, name) in colors.iter().enumerate() {
        let _ = if i == 0 {
            database::set_single_tag(con, path, "color", name, "auto")
        } else {
            database::add_tag(con, path, "color", name, "auto")
        };
    }
}

/// Compute auto colour tags for every image under `folder` that has none yet.
/// Blocking; call from a spawned thread. Returns how many were tagged.
///
/// Freshly indexed files are already tagged inline by `services::sync` from the
/// descriptor it fetched anyway, so in practice this only covers files indexed
/// before that existed. It asks the sidecar rather than decoding here: the
/// sidecar derives the dominant colours from the decode it already performs,
/// where the old Rust path spent a full-resolution decode per file (~1.5 s each
/// on a 5 MB JPEG) purely to name one colour — enough CPU to stall an
/// interactive search by orders of magnitude.
///
/// Yields to searches between chunks via [`search_gate`], and the sidecar's own
/// "index" lane lets a search cut in file-by-file within a chunk, so a search
/// waits for at most one file rather than the whole batch.
pub fn backfill_colors(folder: &str) -> usize {
    let paths = match database::open().and_then(|c| database::paths_missing_category_in_folder(&c, folder, "color")) {
        Ok(p) => p,
        Err(e) => { log::warn!("[tags] colour backfill: cannot list paths: {e}"); return 0; }
    };
    if paths.is_empty() {
        return 0;
    }
    if !sidecar::is_ready() {
        log::info!("[tags] colour backfill: sidecar not ready, skipping {} images under {folder}", paths.len());
        return 0;
    }
    log::info!("[tags] colour backfill: {} images under {folder}", paths.len());
    let t_total = std::time::Instant::now();

    let con = match database::open() {
        Ok(c) => c,
        Err(e) => { log::warn!("[tags] colour backfill: cannot open db: {e}"); return 0; }
    };

    let mut done = 0usize;
    let mut paused_for_search = 0usize;
    for chunk in paths.chunks(BACKFILL_CHUNK) {
        // Stand aside for an interactive search, then pick up where we left off.
        if search_gate::is_active() {
            paused_for_search += 1;
            search_gate::wait_while_active();
        }

        let described = match sidecar::describe(chunk, sidecar::Priority::Index) {
            Ok(d) => d,
            Err(e) => { log::warn!("[tags] colour backfill: describe failed: {e}"); break; }
        };
        for (path, desc) in described {
            let colors = match desc {
                Some(d) if !d.dominant.is_empty() => d.dominant,
                _ => continue,
            };
            apply_color_tags(&con, &path, &colors);
            done += 1;
        }
    }

    log::info!(
        "[timing] colour backfill done: {done} images coloured under {folder} \
         paused_for_search={paused_for_search} total_ms={:.2}",
        t_total.elapsed().as_secs_f64() * 1000.0,
    );
    done
}

// ── Filename / folder suggestion rules ───────────────────────────────────────

fn detect_material(lower: &str) -> Option<&'static str> {
    if lower.contains("pgvt") { return Some("PGVT"); }
    if lower.contains("gvt") { return Some("GVT"); }
    if lower.contains("double charge") || lower.contains("doublecharge") || lower.contains("double-charge") {
        return Some("Double Charge");
    }
    if lower.contains("full body") || lower.contains("fullbody") || lower.contains("full-body") {
        return Some("Full Body");
    }
    if lower.contains("soluble") { return Some("Soluble Salt"); }
    if lower.contains("nano") { return Some("Nano"); }
    if lower.contains("porcelain") { return Some("Porcelain"); }
    if lower.contains("ceramic") { return Some("Ceramic"); }
    if lower.contains("vitrified") { return Some("Vitrified"); }
    None
}

fn detect_finish(lower: &str) -> Option<&'static str> {
    if lower.contains("glossy") || lower.contains("hi gloss") || lower.contains("high gloss") {
        return Some("Glossy");
    }
    if lower.contains("matt") { return Some("Matt"); }
    if lower.contains("polish") { return Some("Polished"); }
    if lower.contains("carving") { return Some("Carving"); }
    if lower.contains("sugar") { return Some("Sugar"); }
    if lower.contains("lappato") { return Some("Lappato"); }
    if lower.contains("satin") { return Some("Satin"); }
    if lower.contains("rustic") { return Some("Rustic"); }
    if lower.contains("rocker") { return Some("Rocker"); }
    None
}

fn detect_design(lower: &str) -> Option<&'static str> {
    const MARBLE: &[&str] = &["marble", "statuario", "carrara", "calacatta", "arabescato", "albastro", "satuario", "onyx"];
    if MARBLE.iter().any(|k| lower.contains(k)) { return Some("Marble"); }
    if lower.contains("wood") || lower.contains("plank") { return Some("Wood"); }
    if lower.contains("travertine") || lower.contains("stone") || lower.contains("slate") { return Some("Stone"); }
    if lower.contains("concrete") || lower.contains("cement") { return Some("Concrete"); }
    if lower.contains("terrazzo") { return Some("Terrazzo"); }
    None
}

/// Find a `NNNxNNN` size token (2–4 digits each side, `x`/`X`/`×` separator).
fn extract_size(s: &str) -> Option<String> {
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    let mut i = 0;
    while i < n {
        if chars[i].is_ascii_digit() {
            let s1 = i;
            while i < n && chars[i].is_ascii_digit() { i += 1; }
            let d1: String = chars[s1..i].iter().collect();

            let mut j = i;
            while j < n && chars[j] == ' ' { j += 1; }
            if j < n && (chars[j] == 'x' || chars[j] == 'X' || chars[j] == '×') {
                let mut k = j + 1;
                while k < n && chars[k] == ' ' { k += 1; }
                let s2 = k;
                while k < n && chars[k].is_ascii_digit() { k += 1; }
                let d2: String = chars[s2..k].iter().collect();
                if (2..=4).contains(&d1.len()) && (2..=4).contains(&d2.len()) {
                    return Some(format!("{d1}x{d2}"));
                }
                i = k;
            }
        } else {
            i += 1;
        }
    }
    None
}

fn err(msg: String) -> Value {
    json!({ "success": false, "message": msg, "data": null })
}
