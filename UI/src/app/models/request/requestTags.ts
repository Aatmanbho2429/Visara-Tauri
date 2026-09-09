// Tagging request payloads — mirrors UI/src-tauri/src/models/request/request_tags.rs.

export interface requestTagsMutate {
  paths: string[];
  category: string;
  value: string;
}

export interface tagFilterDto {
  category: string;
  value: string;
}

export interface requestTagsQuery {
  filters: tagFilterDto[];
}
