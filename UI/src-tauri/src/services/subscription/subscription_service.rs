// Subscription management — wraps the Supabase Edge Functions for plans,
// order creation, payment verification, and history.

use crate::{
    config::SUPABASE_EDGE,
    models::response::{ApiResponse, Plan, ResponseCreateOrder, ResponsePlans, ResponseSubscriptions, ResponseVerifyPayment, Subscription},
    services::auth,
};
use serde_json::Value;

// ── Public API ────────────────────────────────────────────────────────────

pub async fn get_plans() -> ApiResponse<ResponsePlans> {
    let result = reqwest::Client::new()
        .get(format!("{SUPABASE_EDGE}/get-plans"))
        .send()
        .await;

    let resp = match result {
        Err(e) => return network_error(e),
        Ok(r) => r,
    };
    let data: Value = resp.json().await.unwrap_or(Value::Null);
    if !data["success"].as_bool().unwrap_or(false) {
        return ApiResponse::err(500, message_of(&data, "Could not fetch plans"));
    }
    let plans: Vec<Plan> = serde_json::from_value(data["plans"].clone()).unwrap_or_default();
    ApiResponse::ok_with_message("Plans fetched", ResponsePlans { plans })
}

pub async fn create_order(user_id: &str, plan_id: &str) -> ApiResponse<ResponseCreateOrder> {
    let result = reqwest::Client::new()
        .post(format!("{SUPABASE_EDGE}/create-order"))
        .json(&serde_json::json!({ "user_id": user_id, "plan_id": plan_id }))
        .send()
        .await;

    let resp = match result {
        Err(e) => return network_error(e),
        Ok(r) => r,
    };
    let data: Value = resp.json().await.unwrap_or(Value::Null);
    if !data["success"].as_bool().unwrap_or(false) {
        return ApiResponse::err(500, message_of(&data, "Could not create order"));
    }
    ApiResponse::ok_with_message("Order created", ResponseCreateOrder {
        order_id: data["order_id"].as_str().unwrap_or_default().to_string(),
        amount:   data["amount"].clone(),
        currency: data["currency"].as_str().unwrap_or_default().to_string(),
        key_id:   data["key_id"].as_str().unwrap_or_default().to_string(),
        plan:     data["plan"].clone(),
        user:     data["user"].clone(),
    })
}

pub async fn get_user_subscriptions() -> ApiResponse<ResponseSubscriptions> {
    let user_id = match auth::session_user_id() {
        Some(id) => id,
        None     => return ApiResponse::err(401, "No saved session"),
    };

    let result = reqwest::Client::new()
        .post(format!("{SUPABASE_EDGE}/get-user-subscriptions"))
        .json(&serde_json::json!({ "user_id": user_id }))
        .send()
        .await;

    let resp = match result {
        Err(e) => return network_error(e),
        Ok(r) => r,
    };
    let data: Value = resp.json().await.unwrap_or(Value::Null);
    if !data["success"].as_bool().unwrap_or(false) {
        return ApiResponse::err(500, message_of(&data, "Could not fetch subscriptions"));
    }
    let subscriptions: Vec<Subscription> = serde_json::from_value(data["subscriptions"].clone()).unwrap_or_default();
    ApiResponse::ok_with_message("Fetched", ResponseSubscriptions { subscriptions })
}

pub async fn verify_payment(
    razorpay_order_id:   &str,
    razorpay_payment_id: &str,
    razorpay_signature:  &str,
    user_id:             &str,
    plan_id:             &str,
) -> ApiResponse<ResponseVerifyPayment> {
    let result = reqwest::Client::new()
        .post(format!("{SUPABASE_EDGE}/verify-payment"))
        .json(&serde_json::json!({
            "razorpay_order_id":   razorpay_order_id,
            "razorpay_payment_id": razorpay_payment_id,
            "razorpay_signature":  razorpay_signature,
            "user_id":             user_id,
            "plan_id":             plan_id,
        }))
        .send()
        .await;

    let resp = match result {
        Err(e) => return network_error(e),
        Ok(r) => r,
    };
    let data: Value = resp.json().await.unwrap_or(Value::Null);
    if !data["success"].as_bool().unwrap_or(false) {
        return ApiResponse::err(500, message_of(&data, "Payment verification failed"));
    }
    ApiResponse::ok_with_message(message_of(&data, "Payment verified"), ResponseVerifyPayment {
        subscription_status: data["subscription_status"].clone(),
        subscription_end:    data["subscription_end"].clone(),
        days_remaining:      data["days_remaining"].clone(),
    })
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn message_of(data: &Value, fallback: &str) -> String {
    data["message"].as_str().unwrap_or(fallback).to_string()
}

fn network_error<T>(e: reqwest::Error) -> ApiResponse<T> {
    let msg = if e.is_connect() || e.is_timeout() {
        "No internet connection.".to_string()
    } else {
        format!("Network error: {e}")
    };
    ApiResponse::err(503, msg)
}
