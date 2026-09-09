// Tagging response payloads. Wraps `core::database`'s FileTag/TagFacet
// directly — see response_library.rs for why.
use crate::core::database::{FileTag, TagFacet};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagSuggestionDto {
    pub category: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseTagsGet {
    pub tags: Vec<FileTag>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseTagFacets {
    pub facets: Vec<TagFacet>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseTagQuery {
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseTagSuggest {
    pub suggestions: Vec<TagSuggestionDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseTagsUpdated {
    pub colored: usize,
}
