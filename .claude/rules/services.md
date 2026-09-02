---
paths:
  - "src/app/services/**/*.ts"
  - "src-tauri/src/services/**/*.rs"
---

# Services

- One subfolder per entity: `services/auth/`, `services/user/`, `services/product/`, etc.
- A service owns every API call for its entity. Components never call `invoke` or HTTP directly — only through the matching service.
- Angular services call Tauri exclusively through `ZoneWrapperService` (see `zone-wrapper.md`) — never import `@tauri-apps/api` directly in a service.
- Rust services (`<entity>_service.rs`) hold the actual logic and return `ApiResponse<T>`; command handlers in `commands/` stay thin and just delegate to the service.