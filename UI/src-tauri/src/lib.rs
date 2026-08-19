use reqwest::Client;
use serde_json::Value;
use tauri::Emitter;

const API_BASE: &str = "http://127.0.0.1:8765/api/v1";

fn error_response(message: &str) -> Value {
    serde_json::json!({ "success": false, "message": message, "data": null })
}

#[tauri::command]
async fn auth_login(app: tauri::AppHandle, email: String, password: String) {
    let client = Client::new();
    let result = match client
        .post(format!("{}/auth/login", API_BASE))
        .json(&serde_json::json!({ "email": email, "password": password }))
        .send()
        .await
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
        .send()
        .await
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
        .send()
        .await
    {
        Ok(res) => res.json::<Value>().await.unwrap_or_else(|_| error_response("Invalid response from server")),
        Err(_)  => error_response("Cannot connect to Visara service. Please restart the application."),
    };
    let _ = app.emit("auth_request_access_response", result);
}

#[tauri::command]
async fn search_images(
    app:         tauri::AppHandle,
    query_image: String,
    folder_path: String,
    top_k:       Option<u32>,
) {
    let client = Client::new();
    let result = match client
        .post(format!("{}/search", API_BASE))
        .json(&serde_json::json!({
            "query_image": query_image,
            "folder_path": folder_path,
            "top_k":       top_k.unwrap_or(50),
        }))
        .send()
        .await
    {
        Ok(res) => res.json::<Value>().await.unwrap_or_else(|_| error_response("Invalid response from server")),
        Err(_)  => error_response("Cannot connect to Visara service. Please restart the application."),
    };
    let _ = app.emit("search_images_response", result);
}

#[tauri::command]
async fn search_progress(app: tauri::AppHandle) {
    let client = Client::new();
    let result = match client
        .get(format!("{}/search/progress", API_BASE))
        .send()
        .await
    {
        Ok(res) => res.json::<Value>().await.unwrap_or_else(|_| error_response("Invalid response from server")),
        Err(_)  => error_response("Cannot connect to Visara service. Please restart the application."),
    };
    let _ = app.emit("search_progress_response", result);
}

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
        .invoke_handler(tauri::generate_handler![
            auth_login,
            auth_validate_token,
            auth_request_access,
            search_images,
            search_progress
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
