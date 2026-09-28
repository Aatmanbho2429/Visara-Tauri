// Platform-specific device fingerprinting.
//
// The SHA-256 of several hardware identifiers is used as the device ID that
// Supabase ties each licence to.  The approach mirrors the Python
// `license_service.py` implementation exactly so existing bound devices
// continue to work after migration.
//
// The shell commands used to read those identifiers are wrapped in
// `obfstr!()` so they aren't visible verbatim to a `strings <binary>` scan —
// this is the most direct "recipe" for spoofing a device ID (e.g. via a
// fake `ioreg`/`system_profiler`/`wmic` earlier on `$PATH`), so it's worth
// not handing it out for free. This raises the bar slightly; it does not
// make spoofing impossible.

use obfstr::obfstr;
use sha2::{Digest, Sha256};
use std::process::Command;

use crate::error::{PictoriaError, Result};

// Sentinel for a lookup that failed — must never reach the hash as the only component.
const UNKNOWN: &str = "UNKNOWN";

// Derive a stable, opaque device identifier for the current machine.
pub fn device_id() -> Result<String> {
    let parts = platform_ids()?;

    // A fingerprint that read nothing has to fail, not hash a constant every machine shares.
    if parts.iter().all(|p| p.is_empty() || p == UNKNOWN) {
        return Err(PictoriaError::Fatal(
            "Could not read this device's hardware identifiers.".into(),
        ));
    }

    let raw  = parts.join("|");
    let hash = Sha256::digest(raw.as_bytes());
    Ok(hex::encode(hash))
}

// ── Platform implementations ──────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn platform_ids() -> Result<Vec<String>> {
    let wmic = vec![
        wmic_value(obfstr!("csproduct get uuid")),
        wmic_value(obfstr!("cpu get processorid")),
        wmic_value(obfstr!("diskdrive get serialnumber")),
    ];

    // A partial wmic failure keeps its mixed hash — re-keying it would unbind
    // every device that registered through wmic and still works.
    if wmic.iter().any(|v| v != UNKNOWN && !v.is_empty()) {
        return Ok(wmic);
    }

    // wmic.exe is absent on Windows 11 24H2+, where all three lookups failed and
    // every such machine hashed the same "UNKNOWN|UNKNOWN|UNKNOWN" constant.
    cim_ids()
}

// Same three identifiers read through CIM, falling back to MachineGuid.
#[cfg(target_os = "windows")]
fn cim_ids() -> Result<Vec<String>> {
    // Each value is interpolated into a string so a null still emits its own line.
    let lines = powershell_lines(obfstr!(
        "$ErrorActionPreference='SilentlyContinue'; \
         \"$((Get-CimInstance Win32_ComputerSystemProduct).UUID)\"; \
         \"$((Get-CimInstance Win32_Processor|Select-Object -First 1).ProcessorId)\"; \
         \"$((Get-CimInstance Win32_DiskDrive|Select-Object -First 1).SerialNumber)\"; \
         \"$((Get-ItemProperty 'HKLM:\\SOFTWARE\\Microsoft\\Cryptography').MachineGuid)\""
    ));

    let at  = |i: usize| lines.get(i).cloned().unwrap_or_default();
    let cim = vec![at(0), at(1), at(2)];

    if cim.iter().any(|v| !v.is_empty()) {
        return Ok(cim);
    }

    // Present on every Windows install since XP and readable without elevation.
    let machine_guid = at(3);
    if !machine_guid.is_empty() {
        return Ok(vec![machine_guid]);
    }

    Err(PictoriaError::Fatal(
        "Could not read this device's hardware identifiers.".into(),
    ))
}

#[cfg(target_os = "macos")]
fn platform_ids() -> Result<Vec<String>> {
    Ok(vec![
        shell_value(
            obfstr!("ioreg -rd1 -c IOPlatformExpertDevice | awk '/IOPlatformUUID/ { print $3 }'")
        ).trim_matches('"').to_string(),
        shell_value(
            obfstr!("system_profiler SPHardwareDataType | awk '/Serial Number/ { print $4 }'")
        ),
    ])
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn platform_ids() -> Result<Vec<String>> {
    Ok(vec!["UNSUPPORTED_OS".to_string()])
}

// ── Helpers ───────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn wmic_value(query: &str) -> String {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let output = Command::new(obfstr!("wmic"))
        .args(query.split_whitespace())
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    match output {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout);
            // `wmic` output has a header line followed by the value line.
            text.lines()
                .nth(1)
                .unwrap_or(UNKNOWN)
                .trim()
                .to_string()
        }
        Err(_) => UNKNOWN.to_string(),
    }
}

// One hidden PowerShell spawn, returning its stdout lines trimmed.
#[cfg(target_os = "windows")]
fn powershell_lines(script: &str) -> Vec<String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let output = Command::new(obfstr!("powershell"))
        .args([
            obfstr!("-NoProfile"),
            obfstr!("-NonInteractive"),
            obfstr!("-Command"),
            script,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .collect(),
        Err(_) => Vec::new(),
    }
}

#[cfg(target_os = "macos")]
fn shell_value(cmd: &str) -> String {
    let output = Command::new(obfstr!("sh")).args([obfstr!("-c"), cmd]).output();
    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Err(_) => UNKNOWN.to_string(),
    }
}
