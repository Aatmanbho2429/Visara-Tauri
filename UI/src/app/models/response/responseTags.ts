// Tagging response payloads — mirrors UI/src-tauri/src/models/response/response_tags.rs.

// A tag on a file.
export interface FileTag {
  path: string;
  category: string;
  value: string;
  source: string;   // 'auto' | 'manual' | 'filename'
}

// A distinct tag with how many files carry it (drives filter chips).
export interface TagFacet {
  category: string;
  value: string;
  count: number;
}

export interface TagSuggestion {
  category: string;
  value: string;
}

export interface responseTagsGet    { tags: FileTag[]; }
export interface responseTagFacets  { facets: TagFacet[]; }
export interface responseTagQuery   { paths: string[]; }
export interface responseTagSuggest { suggestions: TagSuggestion[]; }
export interface responseTagsUpdated { colored: number; }

// Tag categories the UI knows about (in display order).
export const TAG_CATEGORIES = ['color', 'material', 'finish', 'size', 'design', 'collection', 'custom'] as const;
export type TagCategory = typeof TAG_CATEGORIES[number];

// Categories that hold a single value per file (must match the backend).
export const SINGLE_VALUED: TagCategory[] = ['color', 'size', 'material', 'finish', 'design'];

// Preset options offered in the Categorize panel.
export const TAG_PRESETS: Record<string, string[]> = {
  material: ['PGVT', 'GVT', 'Double Charge', 'Full Body', 'Soluble Salt', 'Porcelain', 'Ceramic', 'Vitrified'],
  finish:   ['Glossy', 'Matt', 'Polished', 'Carving', 'Lappato', 'Satin', 'Rustic', 'Sugar', 'Rocker'],
  size:     ['300x300', '300x600', '600x600', '600x1200', '800x800', '800x1600', '1000x1000', '1200x1200'],
  design:   ['Marble', 'Wood', 'Stone', 'Concrete', 'Terrazzo', 'Onyx', 'Solid', 'Geometric'],
  color:    ['white', 'cream', 'beige', 'grey', 'light grey', 'charcoal', 'black', 'brown', 'gold', 'terracotta', 'blue', 'green'],
};

export const CATEGORY_LABEL: Record<string, string> = {
  color: 'Color', material: 'Material', finish: 'Finish',
  size: 'Size', design: 'Design', collection: 'Collection', custom: 'My Tags',
};
