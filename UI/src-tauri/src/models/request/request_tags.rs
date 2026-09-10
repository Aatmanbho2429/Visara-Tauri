// Tagging request payloads.
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestTagsMutate {
    pub paths: Vec<String>,
    pub category: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagFilterDto {
    pub category: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestTagsQuery {
    pub filters: Vec<TagFilterDto>,
}
