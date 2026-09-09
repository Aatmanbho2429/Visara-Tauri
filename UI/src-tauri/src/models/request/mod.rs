// Request models — one module per entity, re-exported flat.
//
// These are the documented, TS-mirrored IPC contract (see models.md) that the
// Angular side builds its `zoneWrapper.invoke()` payload against. Tauri v2
// binds each `#[tauri::command]`'s arguments by individual named parameter
// rather than one deserialized struct, so nothing on the Rust side actually
// constructs these — hence the blanket `dead_code` allow here rather than on
// each struct.
#![allow(dead_code, unused_imports)]
pub mod request_auth;
pub mod request_common;
pub mod request_library;
pub mod request_search;
pub mod request_subscription;
pub mod request_tags;

pub use request_auth::*;
pub use request_common::*;
pub use request_library::*;
pub use request_search::*;
pub use request_subscription::*;
pub use request_tags::*;
