// Platform file opener — reveals a file in the OS's file manager.

use crate::models::response::ApiResponse;
use tauri::Emitter;

#[tauri::command]
pub fn file_open_path(app: tauri::AppHandle, path: String, request_id: Option<String>) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer")
        .args(["/select,", &path])
        .spawn();

    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open")
        .args(["-R", &path])
        .spawn();

    #[cfg(target_os = "linux")]
    {
        let folder = std::path::Path::new(&path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or(path);
        let _ = std::process::Command::new("xdg-open").arg(&folder).spawn();
    }

    let result: ApiResponse<()> = ApiResponse::ok_empty("Opened").with_request_id(request_id);
    let _ = app.emit("file_open_path_response", result);
}
