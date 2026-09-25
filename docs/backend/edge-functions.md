# Edge functions in use

The 11 functions the desktop app actually calls. Base URL is
`config::SUPABASE_EDGE`. Every function has `verify_jwt = false` and performs
its own authentication (or none — see the auth column).

All request/response shapes below are transcribed from the Rust call sites, so
they reflect the fields the app *sends and reads*, which may be a subset of what
the function accepts or returns.

## Summary

| Function | Method | Auth | Caller |
|---|---|---|---|
| `login-user-test` | POST | none (credentials in body) | `auth_service.rs:177` |
| `validate-token-test` | GET | **Bearer** + `x-device-id` | `auth_service.rs:226` |
| `send-otp` | POST | none | `auth_service.rs:398` |
| `register-request` | POST | none (OTP in body) | `auth_service.rs:416` |
| `forgot-password-send-otp` | POST | none | `auth_service.rs:446` |
| `forgot-password-verify-otp` | POST | none (OTP in body) | `auth_service.rs:458` |
| `change-password` | POST | **Bearer** | `auth_service.rs:484` |
| `get-plans` | GET | none | `subscription_service.rs:15` |
| `create-order` | POST | **none — `user_id` in body** | `subscription_service.rs:33` |
| `get-user-subscriptions` | POST | **none — `user_id` in body** | `subscription_service.rs:63` |
| `verify-payment` | POST | **none — `user_id` in body** | `subscription_service.rs:88` |

The three bolded rows trust a client-supplied `user_id` with no proof of
identity. See [README](README.md) note 2.

---

# Auth — `services/auth/auth_service.rs`

## `POST /login-user-test`

Request

```json
{ "email": "...", "password": "...", "device_id": "..." }
```

Response

```json
{ "success": true, "message": "...", "token": "...", "user": { } }
```

`success: false` is surfaced as 401 with `message`. On success the token is
persisted (OS keychain, falling back to `~/.pictoria_token`) and the user is put
in the in-memory session.

`device_id` comes from `license::device_id()` and is what binds the account to a
machine.

## `GET /validate-token-test`

Headers: `Authorization: Bearer <token>`, `x-device-id: <device_id>`

Response

```json
{ "valid": true, "message": "...", "user": { }, "onnx_key": "..." }
```

The single most important call in the app. Runs on startup and on every periodic
re-validate, and does four things:

1. `valid: false` causes the token to be deleted, session cleared, model key
   dropped, and 401 returned.
2. Populates the in-memory session.
3. Sets `SUBSCRIPTION_OK` from `user.subscription_status` — `expired` and
   `exhausted` block the session (`has_active_session()` feeds
   `sidecar::is_ready()`, so indexing and search both stop).
4. Hands `onnx_key` to `sidecar::set_model_key`, which is what decrypts the DINO
   model. **Withholding `onnx_key` is the licence enforcement mechanism** — an
   empty key means the model never loads and `/describe` fails, so search and
   indexing cannot run at all. This is deliberate, not an error path.

The key is never written to disk on either side and never logged.

## The `user` object

Typed as `AuthUser` in
[response_auth.rs](../../UI/src-tauri/src/models/response/response_auth.rs).
Returned by both login and validate. Supabase emits snake_case; the struct
carries `alias` attributes so it deserializes, and re-serializes to camelCase for
Angular.

```json
{
  "id": "uuid",
  "email": "...",
  "first_name": "...",
  "last_name": "...",
  "phone_number": null,
  "company_name": null,
  "subscription_status": "trial|active|expired|exhausted",
  "subscription_end": "2026-01-01T00:00:00Z",
  "days_remaining": 42
}
```

`days_remaining` is computed by the function, not a stored column. There are
regression tests in `response_auth.rs` guarding this exact deserialization — if
it breaks, `authGuard` bounces the user between `/` and `/master` forever.

## `POST /send-otp`

Request `{ "email": "..." }` — emails a 6-digit registration code. Rate limiting
lives in `email_otps` (`send_count`, `window_start`).

## `POST /register-request`

```json
{
  "first_name": "...", "last_name": "...", "email": "...",
  "password": "...", "phone_number": null, "company_name": null,
  "otp_code": "123456", "device_id": "..."
}
```

Verifies the OTP from `email_otps` and creates the account. Note the account is
created here, not by `create-user` (which is admin-only).

## `POST /forgot-password-send-otp`

Request `{ "email": "..." }` — emails a code to a *registered* address only.
State goes in `password_reset_otps`.

## `POST /forgot-password-verify-otp`

Request `{ "email": "...", "otp_code": "123456" }`

On success Supabase generates a new random password, sets it on the account, and
emails it. Nothing is written locally; the user then logs in normally.

## `POST /change-password`

Header: `Authorization: Bearer <token>`

Request `{ "old_password": "...", "new_password": "..." }`

The token identifies the account — no client-supplied user id is trusted here,
which is the pattern the subscription endpoints should follow. The UI logs the
user out on success.

## Shared response shape for the five above

`send-otp`, `register-request`, both forgot-password calls, and `change-password`
all return `{ "success": bool, "message": "..." }` and nothing else. Rust relays
them through `passthrough()` at
[auth_service.rs:513](../../UI/src-tauri/src/services/auth/auth_service.rs):
`success: true` becomes 200 with the message, `false` becomes **400** with the
message.

---

# Subscription — `services/subscription/subscription_service.rs`

Every function here checks `data["success"]` and returns **500** with the
function's own `message` when it is false.

## `GET /get-plans`

Response

```json
{ "success": true, "plans": [ { "id": "", "name": "", "duration": 30, "amount": "", "currency": "INR" } ] }
```

The app reads only those five fields (`Plan` in `response_subscription.rs`), so
`is_active` filtering and `sort_order` ordering must happen server-side.

## `POST /create-order`

Request `{ "user_id": "...", "plan_id": "..." }`

Response

```json
{
  "success": true, "order_id": "...", "amount": 0,
  "currency": "INR", "key_id": "rzp_...", "plan": { }, "user": { }
}
```

Creates the Razorpay order. `key_id` is the Razorpay publishable key handed to
the checkout widget. `plan` and `user` are passed through to the UI as raw JSON.

## `POST /get-user-subscriptions`

Request `{ "user_id": "..." }` — `user_id` from the in-memory session.

Response `{ "success": true, "subscriptions": [ ] }`, typed as `Subscription`,
including a nested `plans: { name, duration }`.

## `POST /verify-payment`

```json
{
  "razorpay_order_id": "...", "razorpay_payment_id": "...",
  "razorpay_signature": "...", "user_id": "...", "plan_id": "..."
}
```

Response

```json
{
  "success": true, "message": "...",
  "subscription_status": "active",
  "subscription_end": "...", "days_remaining": 30
}
```

Verifies the Razorpay signature, writes the `subscriptions` row, and updates
`users.subscription_status` / `subscription_end`. This is the function that turns
a payment into licence state, so it is the one to read first when a paid user
still cannot search.

---

# Error handling on the Rust side

Every call funnels into `ApiResponse<T>` with these mappings:

| Condition | Status | Message |
|---|---|---|
| Connect/timeout failure | 503 | "No internet connection. Please connect and try again." |
| Other network error | 503 | `Network error: {e}` |
| Response body not JSON | 500 | "Invalid response from server" |
| `success: false` (auth passthrough) | 400 | the function's own `message` |
| `success: false` (subscription) | 500 | the function's own `message` |
| `valid: false` (validate-token) | 401 | the function's own `message` |

Note the inconsistency: a `success: false` from an auth function becomes 400,
while the same thing from a subscription function becomes 500.

Offline behaviour is governed by `config::OFFLINE_GRACE_SECS` (3 days): a
network-level failure with the token still intact keeps the cached session alive
that long before forcing a logout. See `auth_service::periodic_revalidate`.
