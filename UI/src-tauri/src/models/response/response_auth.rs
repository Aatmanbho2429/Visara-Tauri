// Auth response payloads. `AuthUser` mirrors the subset of the Supabase user
// object the UI actually reads (see UI/src/app/models/response/responseAuth.ts) —
// Supabase owns the full shape, we only type what we consume.
use serde::{Deserialize, Serialize};

// `rename_all = "camelCase"` governs BOTH directions: it's what makes this
// struct serialize as camelCase for Angular, but it also means deserializing
// straight from Supabase's own JSON (which is snake_case — `first_name`, not
// `firstName`) would silently fail on every multi-word field, since serde
// looks for the camelCase name and doesn't find it. The `alias` on each such
// field is what makes that path work: it adds the snake_case name as an
// accepted *input* name without touching what gets serialized back out.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthUser {
    pub id: String,
    pub email: String,
    #[serde(alias = "first_name")]
    pub first_name: String,
    #[serde(alias = "last_name")]
    pub last_name: String,
    #[serde(default, alias = "phone_number")]
    pub phone_number: Option<String>,
    #[serde(default, alias = "company_name")]
    pub company_name: Option<String>,
    #[serde(alias = "subscription_status")]
    pub subscription_status: String,
    #[serde(default, alias = "subscription_end")]
    pub subscription_end: Option<String>,
    #[serde(default, alias = "days_remaining")]
    pub days_remaining: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseLogin {
    pub token: String,
    pub user: AuthUser,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseValidateToken {
    pub user: AuthUser,
}

// `action` mirrors what the periodic-revalidate tick decided to do — see
// services::auth::periodic_revalidate for the meaning of each value.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponsePeriodicRevalidate {
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<AuthUser>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Guards the exact bug this file's `alias` attributes exist to prevent:
    // deserializing Supabase's actual snake_case user JSON (as returned by
    // login-user-test / validate-token-test) into AuthUser must succeed, or
    // every login/session-validate call fails and authGuard bounces the user
    // between `/` and `/master` forever.
    #[test]
    fn deserializes_supabase_snake_case_user_json() {
        let supabase_json = serde_json::json!({
            "id": "u1",
            "email": "a@example.com",
            "first_name": "Ada",
            "last_name": "Lovelace",
            "phone_number": "+1234567890",
            "company_name": "Analytical Engines Inc",
            "subscription_status": "active",
            "subscription_end": "2026-01-01T00:00:00Z",
            "days_remaining": 42,
        });
        let user: AuthUser = serde_json::from_value(supabase_json).expect("must parse Supabase's snake_case JSON");
        assert_eq!(user.first_name, "Ada");
        assert_eq!(user.last_name, "Lovelace");
        assert_eq!(user.subscription_status, "active");
        assert_eq!(user.days_remaining, Some(42));

        // The outgoing wire format to Angular must still be camelCase.
        let out = serde_json::to_value(&user).unwrap();
        assert_eq!(out["firstName"], "Ada");
        assert_eq!(out["subscriptionStatus"], "active");
        assert!(out.get("first_name").is_none());
    }

    // Optional fields must not be required — Supabase can omit them entirely.
    #[test]
    fn deserializes_supabase_json_with_optional_fields_missing() {
        let supabase_json = serde_json::json!({
            "id": "u1",
            "email": "a@example.com",
            "first_name": "Ada",
            "last_name": "Lovelace",
            "subscription_status": "trial",
        });
        let user: AuthUser = serde_json::from_value(supabase_json).expect("optional fields must not be required");
        assert_eq!(user.phone_number, None);
        assert_eq!(user.subscription_end, None);
        assert_eq!(user.days_remaining, None);
    }
}
