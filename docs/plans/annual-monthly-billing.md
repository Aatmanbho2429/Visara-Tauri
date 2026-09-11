# Annual & Quarterly Billed Monthly (Razorpay Autopay) — Implementation Plan

> **Status:** approved, not started. Phases in order — do not batch.
>
> - [ ] P0 — Razorpay + Supabase prerequisites (may block for days; start first)
> - [ ] P1 — DDL, additive only
> - [ ] P2 — Guard `get-plans` / `create-order` **before** any recurring plan row exists
> - [ ] P3 — New edge functions (`create-subscription`, `verify-subscription`, `razorpay-webhook`)
> - [ ] P4 — One additive field on `validate-token-test`
> - [ ] P5 — Test-mode end-to-end with **compressed** billing cycles
> - [ ] P6 — Ship the client, then wait 48h for propagation
> - [ ] P7 — Go-live: insert the live plan rows (this is also the kill switch)
>
> **Ordering is load-bearing.** The client must understand the new world before the backend emits
> it, and the live plan row goes in last because it is the only rollback that exists.

---

## Context

Pictoria sells Monthly (₹9,999) and Quarterly (₹24,999) as one-shot Razorpay payments. A yearly
plan at the current rate lands near **₹96,000 in a single transaction** — routinely declined by
Indian card issuers, and a lot to ask before a customer has used the product for a year.

The fix is **annual commitment, debited monthly**. In India that is not just a billing preference:
RBI's *Digital Payments — E-mandate Framework, 2026* permits unattended auto-debit **only up to
₹15,000 per transaction**. Above that the customer must authenticate *every single debit*, which
defeats automation entirely.

| Plan | Shape | Per debit | Unattended? |
|---|---|---|---|
| Annual (new) | ₹8,000 × 12 | ₹8,000 | Yes |
| Quarterly (converted) | ₹8,333 × 3 | ₹8,333 | Yes |
| Monthly | ₹9,999 one-shot | ₹9,999 | N/A — stays one-time |
| ~~Annual one-shot~~ | ~~₹96,000~~ | ₹96,000 | **No** |

Monthly stays one-time deliberately: it is already under ₹15,000, needs no mandate, and preserves
an option for customers who distrust autopay.

### Decisions taken

1. **Supabase stays on Free**, with a keep-alive ping. See §2 — the riskiest open item; the design
   is built so a lost webhook is survivable.
2. **Annual and Quarterly become recurring; Monthly stays one-time.**
3. **New capabilities get new function names. Variants of existing functions are patched in place,
   never forked** — the `-test`/non-`-test` twin split is already a documented footgun in
   `docs/backend/README.md`.

### The governing invariant

> **Fail closed by time, fail open by webhook.**
> Entitlement decays on its own as `subscription_end` passes. Webhooks only ever *extend* it.

Every webhook failure reduces to "the customer lapses on schedule" rather than "the customer is
wrongly locked out" or "the customer gets free service forever". Most hazards below are non-events
because of this. Hold the line.

---

## 1. Landmines — verified against the live edge functions and source

Any one of these, missed, charges customers and locks them out. Force-update means every user gets
the new build at once with no canary, so these fail **simultaneously for everyone**.

### 1.1 `subscription_end` must be ABSOLUTE and MAX-GUARDED

`verify-payment` extends incrementally from the current end date. **The webhook must not copy
that.** Redelivery, out-of-order cycles (cycle 3 before 2 is normal), and a user who buys recurring
while holding one-time days all corrupt entitlement.

```
new_end   = current_end (from payload) + BUFFER_DAYS
write_end = GREATEST(existing_subscription_end, new_end)
```

Never `+= 30 days`. Absolute + `GREATEST` makes replay and reordering harmless *by construction* —
stronger than a dedupe table, and it prevents "customer paid for 200 days, webhook shortened them
to 37".

### 1.2 Derive from `current_end`, never from the next charge date

`subscription.completed` fires after the final cycle and there is **no next charge**. A handler
written as "end = next charge + buffer" gets `null`. In JS `now > null` is `true`, so
`validate-token-test` auto-flips to `expired` **and writes it**. Every annual customer is locked out
the day they finish paying ₹96,000.

`current_end` is present on *every* `subscription.charged`, including the last.
`subscription.completed` writes **no** `subscription_end` — only a renewal-needed flag.

### 1.3 `validate-token-test` auto-expires, destructively

```ts
if (subscriptionStatus === 'active' && subscriptionEnd && now > new Date(subscriptionEnd)) {
  subscriptionStatus = 'expired'
  await svc.from("users").update({ subscription_status: 'expired' })   // persisted
}
```

Harmless today (end dates are 30–90 days out). Under monthly debiting it races the webhook: the
30-minute revalidate (`master.ts:43`) can fire between `charge_at` and delivery, write `expired`,
drop `SUBSCRIPTION_OK`, unload the decrypted DINO model, and bounce the user to profile — and the
client won't learn it was fixed for up to 30 more minutes, then pays a full model recompile.

**Two consequences:**
- **`BUFFER_DAYS = 7`.** Its primary job is *not* absorbing late debits — it is ensuring this
  auto-flip never fires on a healthy subscriber. Do not "optimize" it down.
- The `subscription.charged` handler must set `status = 'active'` **unconditionally**, reviving a
  wrongly-expired user. If it only extends already-active users, one race strands them permanently.

### 1.4 The recurring signature formula differs from the one-time one

One-time uses HMAC over `order_id|payment_id`. **Subscriptions use a different payload, and public
sources conflict on the operand order.** Copy-pasting `verify-payment`'s concatenation is the single
most likely bug in this work; it fails closed, rejecting every activation.

**Do not infer it.** Read the helper in the Razorpay SDK source (`razorpay-node →
utils/razorpay-utils.js`, `validatePaymentVerification`) and confirm against a real test-mode
payment in P5. Record the verified formula in the function's header comment.

**Two different secrets in adjacent functions:** webhook HMAC uses `RAZORPAY_WEBHOOK_SECRET`
(generated in the Razorpay dashboard) over the raw body; the checkout-handler HMAC uses the
existing `RAZORPAY_KEY_SECRET_PROD`.

### 1.5 Raw body, or the webhook never verifies

`await req.text()` → verify HMAC → *then* `JSON.parse`. Calling `req.json()` first destroys the
exact bytes that were signed, and it fails in ways that look like a Razorpay bug.

### 1.6 Epoch seconds, not milliseconds

Razorpay timestamps are Unix **seconds**. `new Date(x)` without `* 1000` yields 1970 →
`subscription_end` in the past → instant expiry for every subscriber at once.

### 1.7 New edge functions default to `verify_jwt = true`

All 20 existing functions are `false` because they were configured that way. `razorpay-webhook`
must be explicitly deployed with `verify_jwt = false` — Razorpay sends no Supabase JWT. Otherwise
every delivery 401s and you find out from a customer.

### 1.8 `cancel-subscription` must authenticate

`verify_jwt=false` plus a body-supplied `user_id` is a remote "cancel anyone's subscription"
endpoint. `docs/backend/README.md` note 2 already flags this pattern on `create-order` /
`verify-payment` / `get-user-subscriptions`; for a *destructive* operation it is not acceptable.
Derive the user from the bearer token the app already holds. Same for `create-subscription`.

---

## 2. Supabase on the Free plan

**Webhooks work on Free.** Edge Functions exist on every tier and Supabase documents them as the
way to receive third-party webhooks; all 20 existing functions already run the `verify_jwt: false`
config a webhook needs. **The capability is not the problem — the pause is.**

Free projects pause after ~7 days of low activity; paid projects never do. Two further Free-tier
costs specific to billing: **~1 day log retention** (you will debug webhook failures blind) and no
meaningful PITR while holding financial state. And **Razorpay's webhook retries expire in ~24h**, so
a pause longer than that *permanently destroys* charge events.

Three mitigations — the design needs all three:

1. **Keep-alive ping** — external scheduler (the GitHub Actions cron already used for releases)
   hits the project every ~48h. **It must alert on failure**, or it is theatre.
2. **The governing invariant** — a lost charge event means the customer lapses on schedule, not
   that they are wrongly locked out or given free service.
3. **Reconciliation** — `verify-subscription` (P3) fetches true state from the Razorpay API. Call
   it when the app opens and the user is recurring with `subscription_end` near. A permanently lost
   webhook self-heals next time the customer opens Pictoria — and the project cannot be paused
   while they are using it.

**Recommendation stands: upgrade to Pro before real money flows.** ~$25/mo is under a third of one
customer's monthly ₹8,000.

---

## 3. What was cut, and why

Do not reintroduce these without re-reading the reasoning.

| Cut | Why |
|---|---|
| **`past_due` as a status value** | Pollutes 8 code paths across 2 languages plus an edge function (Rust deny-list, Angular allow-list, `search.ts:325-329`, `profile.ts:198-203`, `profile.html:63`, and validate-token's onnx allow-list / auto-flip / daysRemaining branches) — all to render a banner. Replaced by a **non-gating advisory field**, `users.past_due_since`. Adding a *field* is additive and safe; adding a *status* is not. |
| **`validate-token-v2`** | Falls out of the above — only one additive field to pass through. Forking the sole source of `onnx_key` means two functions that must both stay correct forever; drift there is silent total product failure. |
| **`get-plans-v2`** | Recreates the twin problem. Use a query param: no param → `one_time` only (old clients safe forever), `?include=recurring` → everything. |
| **CHECK constraint on `subscription_status`** | Actively harmful: the column is written by an external system's webhook. An unanticipated value makes the webhook throw, Razorpay retries for 24h, and the customer stays broken. |
| **"3 of 12 payments" UI** | Keep the columns (`paid_count` is in the payload, cheap); don't build UI on them in v1. |

---

## 4. Phases

**Force-update means no canary and no client rollback** — rolling back requires publishing a
*higher* version containing the old code. **Your only real rollback is the plans row**, so the
launch is structured so the final irreversible-feeling step is a single INSERT that
`is_active=false` reverts in five seconds.

### P0 — Prerequisites (start first)

**Razorpay rail status — verified on the live account 2026-09-11. Not blocked:**

| Rail | Status | Ceiling | Cancellation |
|---|---|---|---|
| **Cards Recurring** | **ACTIVATED** | ₹15,000 | Merchant |
| **eNACH** (netbanking + debit card) | **ACTIVATED** | ₹1 crore | Merchant |
| Paper NACH | ACTIVATED | high | Merchant — physical mandate, slow |
| **UPI Autopay** | **REQUESTED** — est. enablement **25 Sept 2026** | ₹15,000 | **Customer, from their UPI app** |

Subscriptions itself is active. **Build and test on Cards Recurring now**; UPI Autopay should land
before P6/P7. Do **not** cancel the pending UPI Autopay request — there is a "Cancel" link directly
beside its status badge.

### Checkout will offer debit card + UPI only

Product decision. Note what that actually selects:

**Decided: "Debit card" means a card mandate (Cards Recurring), not eNACH.** A debit card could
drive either — eNACH uses the card only to authenticate a *bank-account* mandate — but the product
choice is the card rail. Both offered rails are therefore:

| Rail | Mandate sits on | Ceiling | Who cancels |
|---|---|---|---|
| Cards Recurring | The card (tokenised per RBI) | ₹15,000 | Merchant |
| UPI Autopay | The UPI handle | ₹15,000 | **Customer, from their UPI app** |

**Consequence: ₹15,000 is now a hard ceiling with no escape hatch.** Previously eNACH (₹1 crore)
sat behind the design as a fallback. With both offered rails capped, **any future price increase
that pushes a single debit above ₹15,000 breaks the entire model** — not just one rail. At ₹8,333
the headroom is ~₹6,600/month. Treat that as a pricing constraint, not an implementation detail.

**Keep eNACH activated even though it is not offered.** It is the escape hatch for a customer whose
debit card does not support mandates, and the only rail that could carry a binding commitment.
Activated-but-unoffered costs nothing.

### Card expiry is a churn source that UPI does not have

A card mandate dies when the card expires — mid-subscription, silently. Razorpay surfaces this
(the Subscriptions dashboard has a "Cards Expiring In" filter and a "Subscriptions with Cards
Expiring in 7 days" counter), which is Razorpay signalling that merchants must manage it.

**Requirement for P6:** handle card expiry before it halts the mandate. At minimum, consume the
expiry signal and prompt the customer to re-register in-app. Recovery is cancel + create new (there
is no card-update flow for mandates), so this uses the same "Restart subscription" surface. Without
it, annual subscribers on a card expiring in month 7 churn silently and you learn from a support
ticket.

Three consequences to act on:

1. **Keep Cards Recurring enabled on the account even though it is not in the UI.** It is the only
   instantly-testable mandate rail until UPI Autopay lands ~25 Sept; P5 would otherwise be gated on
   eNACH's slower registration.
2. **Request eSign now** (Account & Settings → Netbanking → E-Mandate; currently *not* activated).
   It is the Aadhaar-based eNACH path and materially improves success where a customer's bank has
   weak netbanking or debit-card mandate support. It likely carries a lead time, as UPI Autopay did.
3. **Query the Methods API for the real bank list** (`recurring.emandate` in the response) before
   committing to debit-card-only. eNACH debit-card support is bank-dependent, and a customer whose
   bank is unsupported has no way to pay at all once credit cards are removed from the UI.

**UX hazard specific to eNACH:** mandate registration is not always instant — some banks take hours
or longer, unlike UPI Autopay. Between now and ~25 Sept, an eNACH-only checkout means a customer
can complete signup and *not yet* have a working app. Decide how that is surfaced before P6; it
interacts directly with the P3 decision on whether checkout charges immediately.

Remaining P0 items:
- **Test-mode API keys.** Only `RAZORPAY_KEY_SECRET_PROD` exists in Supabase env today; testing
  without a test key means testing against production with a real card.
- Decide the authorisation behaviour in the Subscriptions → Settings tab: does checkout charge
  immediately, or ₹0-authorise and debit tomorrow? **This decides whether the user gets access at
  checkout or a day later.**
- Decide Supabase Pro (§2). If staying on Free, stand up the keep-alive **with alerting**.
- **Verify `device_id` behaviour (pre-existing hazard):** `validate-token-test` tears down the
  session on an `x-device-id` mismatch. A customer who reinstalls Windows gets logged out **while
  ₹8,000/month keeps debiting**. Confirm the reset path works before any mandate exists.

### P1 — DDL, additive only
- `plans`: `billing_type` default `'one_time'`, `razorpay_plan_id`, `total_cycles`
- `users`: `past_due_since` (advisory, non-gating)
- `subscriptions`: `razorpay_subscription_id`, `billing_type`, `cycles_paid`, `cancel_at_period_end`
- new `razorpay_webhook_events`: event id, **full payload jsonb**, `received_at`, `processed_at`,
  `error` — needed for disputes and replay

All nullable with defaults. No `NOT NULL`, no CHECK. **Razorpay Plan ids are mode-specific
(test vs live)** — add a second column or make the lookup mode-aware.

*Acceptance:* every existing row reads `billing_type='one_time'`; the running `verify-payment`
insert still succeeds.

### P2 — Guard the existing functions, before any recurring row exists
- `get-plans`: no param → `.eq('billing_type','one_time')`; `?include=recurring` → all
- `create-order`: **reject `billing_type != 'one_time'`** — defence in depth, since `verify_jwt=false`
  means a plan id need not have come from `get-plans`

**Hard stop, both directions:**
- Filter deployed *before the column exists* → `get-plans` errors → every user's plans dialog is
  empty (`subscription_service.rs:44` surfaces only a generic "Could not fetch plans")
- Recurring row inserted *before the filter ships* → an old client renders "Annual ₹8,000" and buys
  it through the one-time flow, granting `plans.duration` days for ₹8,000

*Acceptance:* call production `get-plans` with no params, confirm **exactly 2 plans** return.

### P3 — New edge functions (no recurring plan row yet)
- **`create-subscription`** — authenticated (§1.8). Refuse if the user already holds a live
  `razorpay_subscription_id`. Document `start_at`, `total_count`, `customer_notify`.
- **`verify-subscription`** — the checkout-callback counterpart. Must **fetch the subscription from
  the Razorpay API** to confirm, never trust the client payload. Doubles as the reconciliation
  entry point (§2).
- **`razorpay-webhook`** — `verify_jwt=false` (§1.7), raw body (§1.5), absolute + `GREATEST` (§1.1).
  - `subscription.charged` → extend, revive to `active`, clear `past_due_since`
  - `pending` / `halted` → set `past_due_since`, **do not expire**
  - `cancelled` → set `cancel_at_period_end`, let the period ride out (taking money and withholding
    service is a chargeback)
  - `completed` → renewal flag only, **no** `subscription_end` write (§1.2)
  - `payment.failed`, refunds → **store only**, no automated revocation in v1
  - **Return 200 for events you deliberately ignore**, or Razorpay retries for 24h
  - Dedupe on the `x-razorpay-event-id` header. Process first, then record.
- Register the URL in the Razorpay dashboard, fire a test ping, confirm 200.

### P4 — One additive line on `validate-token-test`
Pass through `past_due_since`. No status changes, no allow-list changes, no fork, no Rust change.
Safe for old clients: `response_auth.rs:108,127` already documents and tests that extra fields must
not break deserialization.

### P5 — Test-mode end-to-end with compressed cycles
You cannot wait 12 months to find the cycle-2 bug. **Create a daily- or weekly-interval test plan**
and prove, before a single real mandate exists:
- cycle 2 extends correctly
- the **final** cycle does not null `subscription_end` (§1.2)
- a redelivered event does not double-extend
- an `expired` user is revived by a late `charged` (§1.3)
- the verified signature formula (§1.4)

### P6 — Ship the client, then wait 48h
Rust and Angular siblings following the existing pattern. Specific traps:
- **This app is zoneless.** The `this.zone.run(...)` in `plans-dialog.ts` is vestigial and a no-op;
  the UI only repaints because `cdr.detectChanges()` is called explicitly. Copying the wrapper into
  the recurring branch without `detectChanges()` silently breaks it.
- **Recovery CTA is "Restart subscription", not "Update payment method."** There is no card-update
  flow for e-mandate/UPI Autopay — recovery is cancel + create new.
- **`daysRemaining` is wrong for recurring** — a 12-cycle subscriber would read "37 days remaining"
  forever. Show "Renews on \<date\>".
- Add a **"Refresh subscription status"** button (reuse the existing validate command); the 30-minute
  timer otherwise leaves a user who just fixed their mandate looking locked.
- Add a **renewal CTA from ~cycle 11**, or every annual customer silently churns at cycle 12.

Wait 48h after release — boot check plus the 6-hour timer gives near-total propagation.

### P7 — Go-live = one INSERT
Insert the live recurring plan rows. This is the launch **and the kill switch**: `is_active=false`
reverts instantly with no release.

---

## 5. Files

**Supabase — new:** `create-subscription`, `verify-subscription`, `razorpay-webhook`
**Supabase — patched in place:** `get-plans` (query param), `create-order` (reject non-`one_time`),
`validate-token-test` (pass through `past_due_since`)

**Rust** — per `.claude/rules/rust-layering.md` and `.claude/rules/auth-licensing.md`:
- `services/subscription/subscription_service.rs` — new fns beside `get_plans` (L13),
  `create_order` (L64), `verify_payment` (L125); reuse `message_of` / `network_error`
- `models/response/response_subscription.rs` — **`amount` must be `f64`, never `String`**
  (regression-tested at L132 after a real bug); `#[serde(alias)]` on every multi-word field
- `commands/subscription_commands.rs` — thin wrappers, thread `request_id: Option<String>`, emit
  `<command>_response`
- `lib.rs:365-368` — register in `invoke_handler![]`
- **Bump `APP_VERSION` in all three places** (`config.rs`, `Cargo.toml`, `tauri.conf.json`)

**Angular:**
- `core/tauri/tauri-commands.const.ts:19-23` — new `SUBSCRIPTION_*` keys
- `models/{request,response}/*Subscription.ts` — mirror the Rust structs
- `services/auth/auth.service.ts:61-92` — new methods
- `shared/plans-dialog/plans-dialog.ts:62-105` — recurring `subscription_id` checkout branch
- `views/profile/` — manage / restart / refresh surface (none exists today)

**Docs:** update `docs/backend/edge-functions.md` (the 20-function inventory) when the new
functions deploy.

---

## 6. Verification

- A debit that succeeds **with the app closed** advances `subscription_end`.
- The same event delivered twice advances it **once**; out-of-order cycles do not shorten it.
- A `subscription.charged` **revives** a user wrongly flipped to `expired`.
- The **final** cycle lapses on time — `subscription_end` neither nulled nor given a free buffer.
- `subscription.halted` does **not** stop search; the advisory banner appears.
- A customer who revokes the mandate from their UPI app keeps working to `subscription_end`.
- A debit 2 days late does not trip the auto-expire (§1.3).
- **An existing one-time user is unaffected by every phase.** Re-run the full existing purchase flow
  after P1, P2, and P4.
- `cargo test` and `npm test -- --watch=false` clean. Pre-existing Angular baseline is 4 failed /
  2 passed, recorded in `docs/plans/force-update.md`.

---

## 7. Settle in P0/P5 — do not infer

- **The subscription signature formula** (§1.4) — from SDK source plus a real test payment.
- Retry schedule and `halted` timing → confirms `BUFFER_DAYS = 7` is enough.
- Does `cancel_at_cycle_end` behave the same on UPI Autopay vs card e-mandate?
- What arrives when a customer revokes a UPI mandate from their own app, and how fast?
- Mandate `max_amount` must exceed the charge for headroom but stay ≤ ₹15,000 for the AFA
  exemption. At ₹8,333 there is room — **any future price rise breaks the exemption.**
- **Per-transaction fees.** Twelve ₹8,000 debits cost more than one ₹96,000 charge. Confirm before
  committing to a 20% annual discount — it may need to be 15%.
- Does our MCC qualify for a ceiling above ₹15,000? **Assume no.**
