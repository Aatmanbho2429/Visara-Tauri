---
paths:
  - "UI/src/app/models/response/**/*.ts"
  - "UI/src-tauri/src/models/response/**/*.rs"
  - "UI/src-tauri/src/commands/**/*.rs"
---

# API response envelope

Every call — success or error — returns the same shape:

```ts
export interface apiResponse<T> {
  statusCode: number;
  message: string;
  data: T;
}
```

```rust
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiResponse<T> {
    pub status_code: u16,
    pub message: String,
    pub data: T,
}
```

- Commands never `panic!` or return a raw error string. Catch errors in the service and map them into this envelope with an error-range `statusCode` and a `message`.
- `ZoneWrapperService.invoke()` unwraps `.data` once, so services and components only ever see the plain payload type — not the envelope.