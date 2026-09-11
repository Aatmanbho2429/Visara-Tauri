# Comments

Deliberately not path-scoped: this applies to new files too, and a path-scoped rule only loads
once Claude has read a matching file.

One comment line above each function and each non-obvious variable or block, saying what it does —
and why, when the name doesn't already say it. Skip lines that explain themselves (`let i = 0;`).

- Line comments only: `//` in TypeScript and Rust, `#` in Python.
- No JSDoc (`/** ... */`), no `/* ... */`, no `"""docstrings"""`.
- Rust: `//`, not `///` or `//!`, unless the item is genuinely public API meant for `cargo doc`.

## One line, not a paragraph

A comment is one line. Two only when a single line genuinely can't carry the "why". Three or more
means the explanation belongs somewhere durable — a plan in `docs/plans/`, a note in
`docs/backend/`, or a rule here — and the inline comment should point at it instead.

Write this:

```python
# Checked once per batch, not per file — Rust gates on `embed_ready`, so this shouldn't happen.
```

Not this:

```python
# Checked once for the whole batch rather than per file: without the
# DINO model every single describe would fail identically, and Rust
# gates on `embed_ready` precisely so this can't happen — so report it
# as one job-level error instead of N indistinguishable per-file ones.
```

The long version isn't wrong, it's misplaced. Reasoning about a design decision goes in the
document that owns the decision; the code gets the one line that tells a reader what they're
looking at.

Two exceptions, both already in the tree — leave them long, and don't treat them as precedent for
ordinary code:

- The changelog block above `core::migrate::EMBED_SCHEMA_VERSION`, which records why each version
  bumped.
- The gate constants at the top of `update.service.ts`, which document a safety property (no flag
  can switch forced updates *off* in a shipped build).
