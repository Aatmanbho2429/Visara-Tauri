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
        Err(e) => {
            log::warn!("[subscription] get_plans: request failed: {e}");
            return network_error(e);
        }
        Ok(r) => r,
    };

    let status = resp.status();
    let body = match resp.text().await {
        Ok(b) => b,
        Err(e) => {
            log::warn!("[subscription] get_plans: could not read response body: {e}");
            return ApiResponse::err(500, "Could not fetch plans");
        }
    };

    let data: Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("[subscription] get_plans: bad response status={status} parse_error={e} body_len={}", body.len());
            return ApiResponse::err(500, "Could not fetch plans");
        }
    };

    if !data["success"].as_bool().unwrap_or(false) {
        log::warn!("[subscription] get_plans: server returned success=false status={status} message={:?}", data["message"].as_str());
        return ApiResponse::err(500, message_of(&data, "Could not fetch plans"));
    }

    // Logged loudly rather than silently defaulted to an empty list: this is
    // exactly the failure mode that hid the 2026-09-10 bug (`Plan.amount`
    // typed as `String` against Supabase's bare-JSON-number `numeric`
    // column) — a shape mismatch here must never again present as "zero
    // plans" with no trace of why.
    let plans: Vec<Plan> = match serde_json::from_value(data["plans"].clone()) {
        Ok(p) => p,
        Err(e) => {
            log::warn!("[subscription] get_plans: plans array failed to deserialize: {e} raw={}", data["plans"]);
            return ApiResponse::err(500, "Could not fetch plans");
        }
    };
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
        Err(e) => {
            log::warn!("[subscription] get_user_subscriptions: request failed: {e}");
            return network_error(e);
        }
        Ok(r) => r,
    };
    let data: Value = resp.json().await.unwrap_or(Value::Null);
    if !data["success"].as_bool().unwrap_or(false) {
        log::warn!("[subscription] get_user_subscriptions: server returned success=false message={:?}", data["message"].as_str());
        return ApiResponse::err(500, message_of(&data, "Could not fetch subscriptions"));
    }
    // Same shape-mismatch class of bug as get_plans() above — log rather
    // than silently default to "no purchase history".
    let subscriptions: Vec<Subscription> = match serde_json::from_value(data["subscriptions"].clone()) {
        Ok(s) => s,
        Err(e) => {
            log::warn!("[subscription] get_user_subscriptions: subscriptions array failed to deserialize: {e} raw={}", data["subscriptions"]);
            return ApiResponse::err(500, "Could not fetch subscriptions");
        }
    };
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
