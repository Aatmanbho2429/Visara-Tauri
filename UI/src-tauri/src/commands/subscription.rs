//! Tauri command handlers for subscription management.

use crate::services::subscription;
use tauri::Emitter;

#[tauri::command]
pub async fn get_plans(app: tauri::AppHandle) {
    let result = subscription::get_plans().await;
    let _ = app.emit("get_plans_response", result);
}

#[tauri::command]
pub async fn get_user_subscriptions(app: tauri::AppHandle) {
    let result = subscription::get_user_subscriptions().await;
    let _ = app.emit("get_user_subscriptions_response", result);
}

#[tauri::command]
pub async fn create_order(app: tauri::AppHandle, user_id: String, plan_id: String) {
    let result = subscription::create_order(&user_id, &plan_id).await;
    let _ = app.emit("create_order_response", result);
}

#[tauri::command]
pub async fn verify_payment(
    app:                 tauri::AppHandle,
    razorpay_order_id:   String,
    razorpay_payment_id: String,
    razorpay_signature:  String,
    user_id:             String,
    plan_id:             String,
) {
    let result = subscription::verify_payment(
        &razorpay_order_id,
        &razorpay_payment_id,
        &razorpay_signature,
        &user_id,
        &plan_id,
    )
    .await;
    let _ = app.emit("verify_payment_response", result);
}
