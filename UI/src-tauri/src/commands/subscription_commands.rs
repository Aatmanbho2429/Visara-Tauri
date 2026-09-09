// Tauri command handlers for subscription management.

use crate::services::subscription;
use tauri::Emitter;

#[tauri::command]
pub async fn subscription_get_plans(app: tauri::AppHandle, request_id: Option<String>) {
    let result = subscription::get_plans().await.with_request_id(request_id);
    let _ = app.emit("subscription_get_plans_response", result);
}

#[tauri::command]
pub async fn subscription_get_user_subscriptions(app: tauri::AppHandle, request_id: Option<String>) {
    let result = subscription::get_user_subscriptions().await.with_request_id(request_id);
    let _ = app.emit("subscription_get_user_subscriptions_response", result);
}

#[tauri::command]
pub async fn subscription_create_order(app: tauri::AppHandle, user_id: String, plan_id: String, request_id: Option<String>) {
    let result = subscription::create_order(&user_id, &plan_id).await.with_request_id(request_id);
    let _ = app.emit("subscription_create_order_response", result);
}

#[tauri::command]
pub async fn subscription_verify_payment(
    app:                 tauri::AppHandle,
    razorpay_order_id:   String,
    razorpay_payment_id: String,
    razorpay_signature:  String,
    user_id:             String,
    plan_id:             String,
    request_id:          Option<String>,
) {
    let result = subscription::verify_payment(
        &razorpay_order_id,
        &razorpay_payment_id,
        &razorpay_signature,
        &user_id,
        &plan_id,
    )
    .await
    .with_request_id(request_id);
    let _ = app.emit("subscription_verify_payment_response", result);
}
