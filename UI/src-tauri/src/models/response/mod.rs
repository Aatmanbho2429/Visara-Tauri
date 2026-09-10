// Response models — one module per entity, re-exported flat so callers write
// `models::response::ResponseLogin` instead of chasing submodule paths.
pub mod api_response;
pub mod response_auth;
pub mod response_browse;
pub mod response_library;
pub mod response_misc;
pub mod response_search;
pub mod response_subscription;
pub mod response_sync;
pub mod response_tags;
pub mod response_update;

pub use api_response::ApiResponse;
pub use response_auth::*;
pub use response_browse::*;
pub use response_library::*;
pub use response_misc::*;
pub use response_search::*;
pub use response_subscription::*;
pub use response_sync::*;
pub use response_tags::*;
pub use response_update::*;
