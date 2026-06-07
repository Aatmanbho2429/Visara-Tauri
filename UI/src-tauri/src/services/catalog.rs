//! Catalog theme storage — CRUD over the `catalog_themes` table.  Themes are
//! opaque JSON document models authored in the frontend builder.

use crate::core::database;
use serde_json::{json, Value};

pub fn save_theme(id: String, name: String, json_doc: String) -> Value {
    let con = match database::open() {
        Ok(c) => c,
        Err(e) => return err(format!("Database error: {e}")),
    };
    match database::save_theme(&con, &id, &name, &json_doc) {
        Ok(_) => json!({ "success": true, "message": "Theme saved.", "data": { "id": id } }),
        Err(e) => err(format!("Could not save theme: {e}")),
    }
}

pub fn list_themes() -> Value {
    match database::open().and_then(|c| database::list_themes(&c)) {
        Ok(themes) => json!({ "success": true, "message": "ok", "data": { "themes": themes } }),
        Err(e) => err(format!("Could not list themes: {e}")),
    }
}

pub fn get_theme(id: String) -> Value {
    match database::open().and_then(|c| database::get_theme(&c, &id)) {
        Ok(Some(theme)) => json!({ "success": true, "message": "ok", "data": { "theme": theme } }),
        Ok(None) => err("Theme not found.".into()),
        Err(e) => err(format!("Could not load theme: {e}")),
    }
}

pub fn delete_theme(id: String) -> Value {
    match database::open().and_then(|c| database::delete_theme(&c, &id)) {
        Ok(_) => json!({ "success": true, "message": "Theme deleted.", "data": null }),
        Err(e) => err(format!("Could not delete theme: {e}")),
    }
}

/// Decode a base64 PDF and write it to `path` (the user-chosen save location).
pub fn save_pdf(path: String, base64: String) -> Value {
    use base64::{engine::general_purpose::STANDARD, Engine};
    match STANDARD.decode(base64.trim()) {
        Ok(bytes) => match std::fs::write(&path, bytes) {
            Ok(_) => json!({ "success": true, "message": "Catalog saved.", "data": { "path": path } }),
            Err(e) => err(format!("Could not write file: {e}")),
        },
        Err(_) => err("Invalid PDF data.".into()),
    }
}

fn err(msg: String) -> Value {
    json!({ "success": false, "message": msg, "data": null })
}
