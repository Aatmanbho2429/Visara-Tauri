# Comments

- One `//` line above each function and non-obvious variable, explaining what it does.
- No JSDoc blocks (`/** ... */`), no multi-line comments.
- Same rule on the Rust side: `//`, not `///` doc-comments, unless the item is genuinely part of a public API meant for `cargo doc`.