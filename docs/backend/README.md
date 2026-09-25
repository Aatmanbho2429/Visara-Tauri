# Pictoria backend (Supabase)

Reference for the Supabase project that backs auth, licensing, and billing.
Generated 2026-09-10 by inspecting the live project and the Rust call sites.

| | |
|---|---|
| Project ref | `qpxvwdxuhgbthzbcppye` |
| Region | `ap-southeast-1` |
| Postgres | 17.6.1.084 |
| Edge base URL | `https://qpxvwdxuhgbthzbcppye.supabase.co/functions/v1` |
| Constant in code | `config::SUPABASE_EDGE` ([config.rs:10](../../UI/src-tauri/src/config.rs)) |

## Contents

- [schema.md](schema.md) — the five tables, columns, keys, indexes, RLS
- [edge-functions.md](edge-functions.md) — the 11 functions the app calls, with request/response shapes

## Who talks to Supabase

Only Rust. Angular never calls Supabase directly — it goes through
`invoke()`/events into `services::auth` and `services::subscription`, which own
every HTTP call. Grepping `UI/src/app/` for `supabase` returns nothing, and that
is intentional; keep it that way.

Two Rust files hold every call site:

- `services/auth/auth_service.rs` — 7 functions (login, session, OTP, password)
- `services/subscription/subscription_service.rs` — 4 functions (plans, orders, payment, history)

## Function inventory

20 edge functions are deployed. 11 are called by the app; 9 are not.

**Used (11):** `login-user-test`, `validate-token-test`, `send-otp`,
`register-request`, `forgot-password-send-otp`, `forgot-password-verify-otp`,
`change-password`, `get-plans`, `create-order`, `get-user-subscriptions`,
`verify-payment`

**Not called by the desktop app (9):**

| Function | Notes |
|---|---|
| `login-user` | Superseded twin of `login-user-test`. Deployed v16 — *ahead* of the one in use |
| `validate-token` | Superseded twin of `validate-token-test` |
| `record-search` | Search metering. Reported not working — treat as inactive |
| `decrement-search` | Search metering. Reported not working — treat as inactive |
| `create-user` | Admin panel |
| `edit-user` | Admin panel |
| `delete-user` | Admin panel |
| `get-user-list` | Admin panel |
| `get-user-by-id` | Admin panel |

The five admin functions are presumably driven by a separate admin surface, not
this desktop app.

## Things worth knowing before you change anything

**1. Production runs on the `-test` functions.** `config::SUPABASE_EDGE` plus
`auth_service` resolve to `/login-user-test` and `/validate-token-test`. The
non-suffixed `login-user` and `validate-token` are still deployed and ACTIVE, and
`login-user` is at a *higher* version than the one actually serving traffic. The
failure mode is someone patching `validate-token`, seeing no change, and hunting
the bug in the wrong file. Rename or delete the unused pair.

**2. `verify_jwt` is `false` on all 20 functions.** Each function does its own
auth, and the app is inconsistent about supplying any:

| Sends `Authorization: Bearer` | Sends only a body |
|---|---|
| `validate-token-test`, `change-password` | `create-order`, `verify-payment`, `get-user-subscriptions` |

The right-hand column accepts a `user_id` in the request body with no proof of
identity. Anyone holding a user's UUID can read their subscription history or
open an order in their name. Worth closing before any quota or entitlement state
is written through this same surface — a metering scheme is only as strong as the
weakest endpoint that can write to it.

**3. RLS is enabled on all five tables, but only `users` has policies.** The
other four have RLS on with zero policies, which in Postgres denies everything
for anon/authenticated roles. Edge functions use the service role and bypass RLS
entirely, so this is a locked-down posture rather than a bug — just be aware that
adding a client-side query to any of those tables will silently return nothing
until a policy exists.

**4. `subscription_status` is free-form `text` with no CHECK constraint.** Four
values are known to be live: `trial` (the column default), `active`, `expired`,
`exhausted`. `expired` and `exhausted` both hard-block the session in
`auth_service::validate_saved_token`. A typo in an edge function writing this
column would silently unblock a lapsed account.

**5. `plans` carries price but no entitlements.** There is no `max_images`,
`max_seats`, or `billing_period`. Every plan grants identical capability and only
differs in duration and price.
