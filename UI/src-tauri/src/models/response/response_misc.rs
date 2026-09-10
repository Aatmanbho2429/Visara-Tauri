// One-off event payloads that don't belong to a bigger entity.
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseHotkeyPressed {
    pub has_image: bool,
    pub image_path: String,
}

// Emitted by `core::sidecar`'s watchdog when the sidecar process exits on its
// own (not via `shutdown()`) — a crash, not a normal shutdown. `recovering:
// true` means it's being relaunched; `false` means the watchdog gave up
// after repeated failures and a human needs to restart the app.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseSidecarCrashed {
    pub recovering: bool,
    pub message: String,
}
