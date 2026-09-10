// Hot-key clipboard query — writes the clipboard image to a single
// reusable temp file (overwritten on every hot-key press, never duplicated).
//
// The path lives inside the OS temp dir, so cleanup is handled by the OS:
//   Windows : %LOCALAPPDATA%\Temp\pictoria_clipboard.png
//   macOS   : /var/folders/.../T/pictoria_clipboard.png
//   Linux   : /tmp/pictoria_clipboard.png

use std::path::PathBuf;

// Returns the path used for the captured clipboard image.
// Always the same path — it is overwritten on each hot-key press, so no
// stray files accumulate over time.
pub fn temp_path() -> PathBuf {
    std::env::temp_dir().join("pictoria_clipboard.png")
}
