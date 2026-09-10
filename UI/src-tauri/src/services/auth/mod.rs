// Re-exported flat so existing callers keep writing `services::auth::login(...)`
// rather than `services::auth::auth_service::login(...)`.
pub mod auth_service;
pub use auth_service::*;
