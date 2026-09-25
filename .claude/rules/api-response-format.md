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

The Rust envelope also carries `request_id: Option<String>`, echoed back from the caller so
concurrent invocations of the same command can be told apart. See `tauri-ipc.md`.

- Commands never `panic!` or return a raw error string. Catch errors in the service and map them into this envelope with an error-range `statusCode` and a `message`.
- `ZoneWrapperService.invoke()` unwraps `.data` once, so services and components only ever see the plain payload type — not the envelope.
- Build the error branch with `PictoriaError::to_response::<T>()` rather than assembling an envelope by hand — it maps straight off `status_code()`: 401 session, 409 `SearchBusy`, 422 image decode, 503 network/model-not-ready, 500 everything else.
- Components never check `.success`. The error branch is reached through `BaseComponent.handle(obs, onSuccess, onError?)`, or a raw `.subscribe({next, error})`, with `onError` receiving the thrown `ApiError`.