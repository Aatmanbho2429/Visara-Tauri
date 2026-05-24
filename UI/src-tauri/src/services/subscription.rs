//! Subscription management — wraps the Supabase Edge Functions for plans,
//! order creation, payment verification, and history.

use crate::{config::SUPABASE_EDGE, services::auth};
use serde_json::Value;

// ── Public API ────────────────────────────────────────────────────────────

pub async fn get_plans() -> Value {
    let result = reqwest::Client::new()
        .get(format!("{SUPABASE_EDGE}/get-plans"))
        .send()
        .await;

    match result {
        Err(e) => network_error(e),
        Ok(resp) => {
            let data: Value = resp.json().await.unwrap_or_else(|_| server_error());
            if !data["success"].as_bool().unwrap_or(false) {
                return data;
            }
            serde_json::json!({
                "success": true,
                "message": "Plans fetched",
                "data":    { "plans": data["plans"] }
            })
        }
    }
}

pub async fn create_order(user_id: &str, plan_id: &str) -> Value {
    let result = reqwest::Client::new()
        .post(format!("{SUPABASE_EDGE}/create-order"))
        .json(&serde_json::json!({ "user_id": user_id, "plan_id": plan_id }))
        .send()
        .await;

    match result {
        Err(e) => network_error(e),
        Ok(resp) => {
            let data: Value = resp.json().await.unwrap_or_else(|_| server_error());
            if !data["success"].as_bool().unwrap_or(false) {
                return data;
            }
            serde_json::json!({
                "success": true,
                "message": "Order created",
                "data": {
                    "order_id": data["order_id"],
                    "amount":   data["amount"],
                    "currency": data["currency"],
                    "key_id":   data["key_id"],
                    "plan":     data["plan"],
                    "user":     data["user"],
                }
            })
        }
    }
}

pub async fn get_user_subscriptions() -> Value {
    let user_id = match auth::session_user_id() {
        Some(id) => id,
        None     => return serde_json::json!({
            "success": false,
            "message": "No saved session",
            "data":    null
        }),
    };

    let result = reqwest::Client::new()
        .post(format!("{SUPABASE_EDGE}/get-user-subscriptions"))
        .json(&serde_json::json!({ "user_id": user_id }))
        .send()
        .await;

    match result {
        Err(e) => network_error(e),
        Ok(resp) => {
            let data: Value = resp.json().await.unwrap_or_else(|_| server_error());
            if !data["success"].as_bool().unwrap_or(false) {
                return data;
            }
            serde_json::json!({
                "success": true,
                "message": "Fetched",
                "data":    { "subscriptions": data["subscriptions"] }
            })
        }
    }
}

pub async fn verify_payment(
    razorpay_order_id:   &str,
    razorpay_payment_id: &str,
    razorpay_signature:  &str,
    user_id:             &str,
    plan_id:             &str,
) -> Value {
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

    match result {
        Err(e) => network_error(e),
        Ok(resp) => {
            let data: Value = resp.json().await.unwrap_or_else(|_| server_error());
            if !data["success"].as_bool().unwrap_or(false) {
                return data;
            }
            serde_json::json!({
                "success": true,
                "message": data["message"].as_str().unwrap_or("Payment verified"),
                "data": {
                    "subscription_status": data["subscription_status"],
                    "subscription_end":    data["subscription_end"],
                    "days_remaining":      data["days_remaining"],
                }
            })
        }
    }
}

// ── Error constructors ────────────────────────────────────────────────────

fn server_error() -> Value {
    serde_json::json!({
        "success": false,
        "message": "Invalid response from server",
        "data":    null
    })
}

fn network_error(e: reqwest::Error) -> Value {
    let msg = if e.is_connect() || e.is_timeout() {
        "No internet connection.".to_string()
    } else {
        format!("Network error: {e}")
    };
    serde_json::json!({ "success": false, "message": msg, "data": null })
}
