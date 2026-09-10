// Colour vocabulary for tiles.
//
// Tiles are mostly low-saturation neutrals (whites, creams, greys, browns), so
// a plain RGB nearest-colour match confuses beige/grey/cream badly.  Both
// colour signals therefore classify pixels in HSV — saturation first to split
// neutrals from chromatics — and vote.
//
// Only the vocabulary lives here now; both votes run sidecar-side, off the
// decode `describe()` already performs (`pipeline.color_histogram` for the
// similarity vector, `pipeline.dominant_colors` for the Browse tag).  They
// used to run in Rust, each paying its own full-resolution decode of a file
// the sidecar had just decoded anyway.
//
// `sidecar/pipeline.py` mirrors the bucket list below verbatim; changing one
// without the other silently corrupts stored vectors.

// Fixed bucket order for the colour histogram vector.  MUST NOT be reordered
// once shipped — the index of each bucket is the dimension it occupies in the
// stored colour vector, so a reorder would silently invalidate every persisted
// vector without any error surfacing.
pub const COLOR_BUCKETS: [&str; 16] = [
    "white", "light grey", "grey", "charcoal", "black",
    "cream", "beige",
    "red", "brown", "terracotta", "gold",
    "green", "teal", "blue", "purple", "pink",
];

// Dimensionality of the colour vector stored per file.
pub const COLOR_DIM: usize = COLOR_BUCKETS.len();
