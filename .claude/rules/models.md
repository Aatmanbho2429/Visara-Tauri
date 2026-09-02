---
paths:
  - "src/app/models/**/*.ts"
  - "src-tauri/src/models/**/*.rs"
---

# Models

## TypeScript (`src/app/models/`)
- Requests live in `models/request/`, responses in `models/response/`.
- File and exported type names are camelCase, prefixed with `request` or `response` — e.g. `requestLogin.ts`, `responseLogin.ts`.
- Model fields are camelCase.

## Rust (`src-tauri/src/models/`)
- Same split: `models/request/`, `models/response/`.
- File and struct names follow idiomatic Rust convention — `snake_case` file, `PascalCase` struct — e.g. `request_login.rs` → `struct RequestLogin`.
- Annotate every struct with `#[serde(rename_all = "camelCase")]` so JSON crossing the IPC boundary matches the TS field names exactly.

## Keep the two trees mirrored
Every TS model has a same-named Rust counterpart: `requestLogin.ts` ↔ `request_login.rs`. When adding, renaming, or changing fields on one, update the other in the same change — nothing enforces this automatically.