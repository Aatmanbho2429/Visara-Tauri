# Database schema

Schema `public`, five tables. Row counts are from 2026-09-10 and are indicative
of an early-stage project, not load.

RLS is enabled on every table. Only `users` defines policies — see
[README](README.md#things-worth-knowing-before-you-change-anything) note 3.

---

## `users` — 12 rows

The account record. Also carries licence state, device binding, and the search
counter.

| Column | Type | Null | Default | Notes |
|---|---|---|---|---|
| `id` | `uuid` | no | `gen_random_uuid()` | PK |
| `first_name` | `text` | no | | |
| `last_name` | `text` | no | | |
| `email` | `text` | no | | **unique** |
| `phone_number` | `text` | yes | | |
| `company_name` | `text` | yes | | |
| `is_active` | `boolean` | yes | `true` | |
| `device_id` | `text` | yes | | Machine binding; set at login/register from `license::device_id()` |
| `device_reset_requested` | `boolean` | yes | `false` | Manual approval gate for rebinding to a new machine |
| `created_at` | `timestamptz` | yes | `now()` | |
| `updated_at` | `timestamptz` | yes | `now()` | |
| `subscription_status` | `text` | yes | `'trial'` | `trial` / `active` / `expired` / `exhausted` — no CHECK constraint |
| `subscription_end` | `timestamptz` | yes | | |
| `search_count` | `integer` | no | `0` | Search metering. The functions that move it are not working — see below |

**Keys and indexes**
- PK `users_pkey` on `id`
- UNIQUE `users_email_key` on `email`

**RLS policies**
| Policy | Command | Expression |
|---|---|---|
| `users_select_own` | SELECT | `auth.uid() = id` |
| `users_update_own` | UPDATE | `auth.uid() = id` |
| `block_direct_insert` | INSERT | `WITH CHECK (false)` |
| `block_direct_delete` | DELETE | `USING (false)` |

Inserts and deletes are impossible except through the service role, i.e. only
via edge functions. That is the correct shape for this app.

**On `search_count`:** the column exists and defaults to 0, and `record-search`
/ `decrement-search` are deployed against it. Both are reported non-functional,
so treat this column as **inert** — nothing currently maintains it, and
`subscription_status = 'exhausted'` (which the client does enforce) is therefore
not reachable through normal use today.

**Not a column:** `days_remaining` appears in `AuthUser`
([response_auth.rs](../../UI/src-tauri/src/models/response/response_auth.rs))
and in login/validate responses, but it is computed by the edge function from
`subscription_end`, not stored.

---

## `plans` — 2 rows

Purchasable plans, read by the app to render the pricing screen.

| Column | Type | Null | Default |
|---|---|---|---|
| `id` | `uuid` | no | `gen_random_uuid()` |
| `name` | `text` | no | |
| `duration` | `integer` | no | days |
| `amount` | `numeric` | no | |
| `currency` | `text` | yes | `'INR'` |
| `is_active` | `boolean` | yes | `true` |
| `sort_order` | `integer` | yes | `0` |
| `created_at` | `timestamptz` | yes | `now()` |

**Current rows**

| name | duration | amount | currency | is_active | sort_order |
|---|---|---|---|---|---|
| Monthly | 30 | 9999.00 | INR | true | 1 |
| Quarterly | 90 | 24999.00 | INR | true | 2 |

**Keys and indexes:** PK `plans_pkey` on `id`. Nothing else.

No entitlement columns — every plan grants identical capability.

---

## `subscriptions` — 2 rows

Purchase history and the Razorpay payment record.

| Column | Type | Null | Default |
|---|---|---|---|
| `id` | `uuid` | no | `gen_random_uuid()` |
| `user_id` | `uuid` | yes | → `users(id)` **ON DELETE CASCADE** |
| `plan_id` | `uuid` | yes | → `plans(id)` |
| `amount` | `numeric` | no | |
| `currency` | `text` | yes | `'INR'` |
| `status` | `text` | yes | `'active'` |
| `start_date` | `timestamptz` | no | |
| `end_date` | `timestamptz` | no | |
| `razorpay_order_id` | `text` | yes | |
| `razorpay_payment_id` | `text` | yes | |
| `razorpay_signature` | `text` | yes | |
| `payment_method` | `text` | yes | `'razorpay'` |
| `notes` | `text` | yes | |
| `created_at` | `timestamptz` | yes | `now()` |
| `created_by` | `text` | yes | |

**Keys and indexes:** PK `subscriptions_pkey` on `id` only.

Both foreign keys are **unindexed**. Irrelevant at 2 rows; add
`idx_subscriptions_user_id` before the table grows, since `get-user-subscriptions`
filters on exactly that column.

---

## `email_otps` — 5 rows

Registration OTP state, keyed by email. Carries its own rate-limit counters.

| Column | Type | Null | Default |
|---|---|---|---|
| `email` | `text` | no | PK |
| `code_hash` | `text` | no | code is hashed, never stored plain |
| `expires_at` | `timestamptz` | no | |
| `attempts` | `integer` | no | `0` — verification attempts |
| `last_sent_at` | `timestamptz` | no | `now()` |
| `send_count` | `integer` | no | `0` — sends in the current window |
| `window_start` | `timestamptz` | no | `now()` — rate-limit window anchor |

---

## `password_reset_otps` — 1 row

Password-reset OTP state. Same idea as `email_otps` but without the send-rate
columns.

| Column | Type | Null | Default |
|---|---|---|---|
| `email` | `text` | no | PK |
| `code_hash` | `text` | no | |
| `expires_at` | `timestamptz` | no | |
| `attempts` | `integer` | no | `0` |
| `created_at` | `timestamptz` | no | `now()` |

---

## Relationships

```
users ──1:N──> subscriptions <──N:1── plans
  │                                (no cascade)
  └── ON DELETE CASCADE

email_otps           (standalone, keyed by email)
password_reset_otps  (standalone, keyed by email)
```

Deleting a user cascades their subscriptions away — including the Razorpay
payment record, which is usually something you want to retain for accounting.
Consider `ON DELETE SET NULL` or a soft delete via the existing `is_active`.
