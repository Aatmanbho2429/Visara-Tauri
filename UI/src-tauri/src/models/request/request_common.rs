// Shared request shapes reused by several commands that take nothing but a
// path or a path list — kept here once rather than duplicated per command,
// since the shape (and therefore the TS ↔ Rust mirror) is identical.
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestPath {
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestPaths {
    pub paths: Vec<String>,
}
