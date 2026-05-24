//! Platform-specific device fingerprinting.
//!
//! The SHA-256 of several hardware identifiers is used as the device ID that
//! Supabase ties each licence to.  The approach mirrors the Python
//! `license_service.py` implementation exactly so existing bound devices
//! continue to work after migration.

use sha2::{Digest, Sha256};
use std::process::Command;

/// Derive a stable, opaque device identifier for the current machine.
pub fn device_id() -> String {
    let parts = platform_ids();
    let raw    = parts.join("|");
    let hash   = Sha256::digest(raw.as_bytes());
    hex::encode(hash)
}

// ── Platform implementations ──────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn platform_ids() -> Vec<String> {
    vec![
        wmic_value("csproduct get uuid"),
        wmic_value("cpu get processorid"),
        wmic_value("diskdrive get serialnumber"),
    ]
}

#[cfg(target_os = "macos")]
fn platform_ids() -> Vec<String> {
    vec![
        shell_value(
            "ioreg -rd1 -c IOPlatformExpertDevice | awk '/IOPlatformUUID/ { print $3 }'"
        ).trim_matches('"').to_string(),
        shell_value(
            "system_profiler SPHardwareDataType | awk '/Serial Number/ { print $4 }'"
        ),
    ]
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn platform_ids() -> Vec<String> {
    vec!["UNSUPPORTED_OS".to_string()]
}

// ── Helpers ───────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn wmic_value(query: &str) -> String {
    let output = Command::new("wmic")
        .args(query.split_whitespace())
        .output();

    match output {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout);
            // `wmic` output has a header line followed by the value line.
            text.lines()
                .nth(1)
                .unwrap_or("UNKNOWN")
                .trim()
                .to_string()
        }
        Err(_) => "UNKNOWN".to_string(),
    }
}

#[cfg(target_os = "macos")]
fn shell_value(cmd: &str) -> String {
    let output = Command::new("sh").args(["-c", cmd]).output();
    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Err(_) => "UNKNOWN".to_string(),
    }
}
