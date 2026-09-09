// Subscription/payment response payloads — thin typed wrappers over the
// Supabase Edge Function responses (see services::subscription).
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub id: String,
    pub name: String,
    pub duration: i64,
    pub amount: String,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanRef {
    pub name: String,
    pub duration: i64,
}

// See the comment on `AuthUser` in response_auth.rs: `rename_all` governs
// serialize AND the primary deserialize name, so the multi-word fields below
// need an `alias` for Supabase's own snake_case JSON to still parse.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subscription {
    pub id: String,
    pub amount: String,
    pub currency: String,
    pub status: String,
    #[serde(alias = "start_date")]
    pub start_date: String,
    #[serde(alias = "end_date")]
    pub end_date: String,
    #[serde(alias = "created_at")]
    pub created_at: String,
    #[serde(default, alias = "razorpay_payment_id")]
    pub razorpay_payment_id: Option<String>,
    #[serde(default, alias = "payment_method")]
    pub payment_method: Option<String>,
    #[serde(default)]
    pub plans: Option<PlanRef>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponsePlans {
    pub plans: Vec<Plan>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseSubscriptions {
    pub subscriptions: Vec<Subscription>,
}

// Razorpay order fields plus the plan/user Supabase echoed back — kept as raw
// JSON since they're display-only passthrough, not consumed field-by-field.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseCreateOrder {
    pub order_id: String,
    pub amount: serde_json::Value,
    pub currency: String,
    pub key_id: String,
    pub plan: serde_json::Value,
    pub user: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseVerifyPayment {
    pub subscription_status: serde_json::Value,
    pub subscription_end: serde_json::Value,
    pub days_remaining: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Same regression guard as AuthUser's — get_user_subscriptions() calls
    // `serde_json::from_value::<Vec<Subscription>>` straight against
    // Supabase's snake_case JSON, silently swallowed into an empty Vec via
    // `unwrap_or_default()` if it fails, which reads as "no purchase history"
    // rather than an error.
    #[test]
    fn deserializes_supabase_snake_case_subscription_json() {
        let supabase_json = serde_json::json!({
            "id": "s1",
            "amount": "999",
            "currency": "INR",
            "status": "active",
            "start_date": "2026-01-01",
            "end_date": "2026-02-01",
            "created_at": "2026-01-01T00:00:00Z",
            "razorpay_payment_id": "pay_123",
            "payment_method": "card",
            "plans": { "name": "Monthly", "duration": 30 },
        });
        let sub: Subscription = serde_json::from_value(supabase_json).expect("must parse Supabase's snake_case JSON");
        assert_eq!(sub.start_date, "2026-01-01");
        assert_eq!(sub.end_date, "2026-02-01");
        assert_eq!(sub.created_at, "2026-01-01T00:00:00Z");
        assert_eq!(sub.razorpay_payment_id.as_deref(), Some("pay_123"));

        let out = serde_json::to_value(&sub).unwrap();
        assert_eq!(out["startDate"], "2026-01-01");
        assert_eq!(out["razorpayPaymentId"], "pay_123");
    }
}
