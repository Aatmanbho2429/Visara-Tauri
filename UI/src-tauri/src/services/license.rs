//! Platform-specific device fingerprinting.
//!
//! The SHA-256 of several hardware identifiers is used as the device ID that
//! Supabase ties each licence to.  The approach mirrors the Python
//! `license_service.py` implementation exactly so existing bound devices
//! continue to work after migration.
//!
//! The shell commands used to read those identifiers are wrapped in
//! `obfstr!()` so they aren't visible verbatim to a `strings <binary>` scan —
//! this is the most direct "recipe" for spoofing a device ID (e.g. via a
//! fake `ioreg`/`system_profiler`/`wmic` earlier on `$PATH`), so it's worth
//! not handing it out for free. This raises the bar slightly; it does not
//! make spoofing impossible.

use obfstr::obfstr;
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
        wmic_value(obfstr!("csproduct get uuid")),
        wmic_value(obfstr!("cpu get processorid")),
        wmic_value(obfstr!("diskdrive get serialnumber")),
    ]
}

#[cfg(target_os = "macos")]
fn platform_ids() -> Vec<String> {
    vec![
        shell_value(
            obfstr!("ioreg -rd1 -c IOPlatformExpertDevice | awk '/IOPlatformUUID/ { print $3 }'")
        ).trim_matches('"').to_string(),
        shell_value(
            obfstr!("system_profiler SPHardwareDataType | awk '/Serial Number/ { print $4 }'")
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
    let output = Command::new(obfstr!("wmic"))
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
    let output = Command::new(obfstr!("sh")).args([obfstr!("-c"), cmd]).output();
    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Err(_) => "UNKNOWN".to_string(),
    }
}
