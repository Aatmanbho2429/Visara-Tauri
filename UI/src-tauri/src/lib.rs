use reqwest::Client;
use serde_json::Value;
use tauri::Emitter;

const API_BASE: &str = "http://127.0.0.1:8765/api/v1";

fn error_response(message: &str) -> Value {
    serde_json::json!({ "success": false, "message": message, "data": null })
}

// ── Auth commands ──────────────────────────────────────────────────────────

#[tauri::command]
async fn auth_login(app: tauri::AppHandle, email: String, password: String) {
    let client = Client::new();
    let result = match client
        .post(format!("{}/auth/login", API_BASE))
        .json(&serde_json::json!({ "email": email, "password": password }))
        .send().await
    {
        Ok(res) => res.json::<Value>().await.unwrap_or_else(|_| error_response("Invalid response from server")),
        Err(_)  => error_response("Cannot connect to Visara service. Please restart the application."),
    };
    let _ = app.emit("auth_login_response", result);
}

#[tauri::command]
async fn auth_validate_token(app: tauri::AppHandle) {
    let client = Client::new();
    let result = match client
        .get(format!("{}/auth/validate-token", API_BASE))
        .send().await
    {
        Ok(res) => res.json::<Value>().await.unwrap_or_else(|_| error_response("Invalid response from server")),
        Err(_)  => error_response("Cannot connect to Visara service. Please restart the application."),
    };
    let _ = app.emit("auth_validate_token_response", result);
}

#[tauri::command]
async fn auth_request_access(
    app:          tauri::AppHandle,
    first_name:   String,
    last_name:    String,
    email:        String,
    password:     String,
    phone_number: Option<String>,
    company_name: Option<String>,
) {
    let client = Client::new();
    let result = match client
        .post(format!("{}/auth/request-access", API_BASE))
        .json(&serde_json::json!({
            "first_name":   first_name,
            "last_name":    last_name,
            "email":        email,
            "password":     password,
            "phone_number": phone_number,
            "company_name": company_name,
        }))
        .send().await
    {
        Ok(res) => res.json::<Value>().await.unwrap_or_else(|_| error_response("Invalid response from server")),
        Err(_)  => error_response("Cannot connect to Visara service. Please restart the application."),
    };
    let _ = app.emit("auth_request_access_response", result);
}

// ── Search command ─────────────────────────────────────────────────────────

#[tauri::command]
async fn start_search(
    app:         tauri::AppHandle,
    image_path:  String,
    folder_path: String,
    top_k:       i32,
) {
    let app_clone = app.clone();
    tokio::spawn(async move {
        let client = Client::new();

        // Start the search (non-blocking on Python side)
        match client
            .post(format!("{}/search/start", API_BASE))
            .json(&serde_json::json!({
                "image_path":  image_path,
                "folder_path": folder_path,
                "top_k":       top_k,
            }))
            .send().await
        {
            Err(_) => {
                let _ = app_clone.emit("search_error", error_response(
                    "Cannot connect to Visara service. Make sure Python is running."
                ));
                return;
            }
            Ok(res) => {
                let data = res.json::<Value>().await.unwrap_or_default();
                if !data["success"].as_bool().unwrap_or(false) {
                    let _ = app_clone.emit("search_error", data);
                    return;
                }
            }
        }

        // Poll progress until done
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            match client
                .get(format!("{}/search/progress", API_BASE))
                .send().await
            {
                Ok(res) => {
                    let data: Value = res.json().await.unwrap_or_default();
                    let success     = data["success"].as_bool().unwrap_or(false);
                    let payload     = data["data"].clone();

                    // Fatal error — success: false from Python
                    if !success {
                        let _ = app_clone.emit("search_error", &data);
                        break;
                    }

                    // Done successfully
                    if payload["done"].as_bool().unwrap_or(false) {
                        let _ = app_clone.emit("search_complete", &payload);
                        break;
                    }

                    // Still in progress
                    let _ = app_clone.emit("search_progress", &payload);
                }
                Err(_) => {
                    let _ = app_clone.emit("search_error", error_response(
                        "Lost connection during search."
                    ));
                    break;
                }
            }
        }
    });
}

// ── File opener ────────────────────────────────────────────────────────────

#[tauri::command]
fn open_file_path(path: String) {
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
        let _ = std::process::Command::new("xdg-open").arg(folder).spawn();
    }
}

// ── App entry ──────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            auth_login,
            auth_validate_token,
            auth_request_access,
            start_search,
            open_file_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
