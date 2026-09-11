---
paths:
  - "UI/src-tauri/src/services/auth/**/*.rs"
  - "UI/src-tauri/src/services/subscription/**/*.rs"
  - "UI/src-tauri/src/services/license/**/*.rs"
  - "UI/src/app/services/auth/**/*.ts"
  - "UI/src/app/services/subscription/**/*.ts"
---

# Auth, licensing, subscription

Auth and subscription state come from Supabase (`config::SUPABASE_EDGE`). Token-validate returns
an `onnx_key` that `services::auth` hands to `core::sidecar::set_model_key` — that is what unlocks
`/describe`, so a broken auth path presents as indexing and search never starting.

- The bearer token is the **only** thing persisted to disk (`~/.pictoria_token`, via the OS
  keychain where available — see the `keyring` dependency — falling back to a plaintext file).
  Deleted on logout.
- `config::OFFLINE_GRACE_SECS` (3 days) is how long a cached "valid" subscription state survives
  with no network before a re-validate is forced.
- `services::license` is device fingerprinting: a SHA-256 over hardware ids, with the shell
  commands that read them wrapped in `obfstr!()` so they don't surface in a `strings` scan.

## Read `docs/backend/README.md` first

**Only Rust talks to Supabase.** Angular goes through `invoke()`/events into `services::auth` and
`services::subscription`, which own every HTTP call. Grepping `UI/src/app/` for `supabase` returns
nothing, and that is intentional — keep it that way.

`docs/backend/` documents the live project (schema, RLS, all 20 deployed edge functions against
the 11 the app actually calls). Two footguns that have cost time before:

- **Production runs on the `-test`-suffixed functions.** `login-user-test` and
  `validate-token-test` serve real traffic while the non-suffixed twins are still deployed and
  ACTIVE — `login-user` is at a *higher* version than the one in use. Patching `validate-token`
  and seeing no change is the failure mode.
- **`verify_jwt` is `false` on all 20 functions**, and several (`create-order`, `verify-payment`,
  `get-user-subscriptions`) accept a `user_id` in the body with no proof of identity. Worth
  closing before any entitlement or quota state is written through the same surface.

Also: RLS is on for all five tables but only `users` has policies, so a client-side query against
the others silently returns nothing. `subscription_status` is free-form `text` with no CHECK
constraint — `trial`, `active`, `expired`, `exhausted` are the live values, and the last two hard-
block the session in `validate_saved_token`.
