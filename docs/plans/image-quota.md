# Image Quota — Implementation Plan

> **Status:** approved, not yet started. Implementation begins at Phase 0.
> Phases are independently shippable — tick them off here as they land.
>
> - [ ] Phase 0 — Safety refactor (`SyncReport`, always-save, marker fix)
> - [ ] Phase 1 — Ledgers + meter, no enforcement
> - [ ] Phase 2 — Server plumbing (DDL, edge functions, trial-unlimited)
> - [ ] Phase 3 — Purchase card shows live library size
> - [ ] Phase 4 — Keep-selection flow (**must precede enforcement**)
> - [ ] Phase 5 — Enforcement gate + kill switch
> - [ ] Phase 6 — Library UI (quota strip, `blocked` status)
> - [ ] Phase 7 — Rollout close-out

---

## Context

Pictoria sells one plan tier (Monthly ₹9,999 / Quarterly ₹24,999) granting identical capability to a
large Morbi tile factory and a one-person design studio. That prices out the small end of the market
and undercharges the large end.

The fix is to meter **distinct images indexed** and cap it per plan, so pricing can ladder by library
size — the number that correlates almost perfectly with who the customer is.

The naive meter (count rows in `files`) is trivially defeated: index 2k, search, remove, index the
next 2k, repeat — covering a 10k library on a 2k plan.

**Product shape:** trial users index their **whole library, uncapped**. They see the tool work on
their real archive, and at purchase time the plan cards show their actual library size so choosing a
plan is obvious rather than a guess. On purchase they select which folders to keep within the plan
they bought; the rest are removed. From that moment the cap is enforced normally.

---

## 1. The core rule

> **The real ledger has exactly one birth event. It is never decremented. The birth count sets a
> floor on the cap.**

The ledger counts **distinct content hashes**. `files.hash` already exists and is content-derived —
`SHA-256(file_size ‖ first 64 KiB)` via `utils::file_utils::fast_hash`
([file_utils.rs:52](../../UI/src-tauri/src/utils/file_utils.rs)) — so it is path-independent.
Renaming, moving, or re-adding the same image costs nothing; rotating a *new* folder in costs quota.

One rule, three cases:

| Case | Ledger born | From | Cap floor |
|---|---|---|---|
| Pre-rollout paying user (the 12) | At rollout, automatically | Their **entire existing library** | Their library — they lose nothing, no selection UI |
| Trial user who purchases | At purchase | The folders they **choose to keep** | Kept set (≤ cap, so moot) |
| Expired, then re-purchase **smaller** | **Rebirth** when `effective_cap < used` | A new selection | New kept set |

`effective_cap = MAX(plan_quota, birth_count)`.

**Plans cannot be changed mid-term.** A new plan is only purchasable once the current one has
expired. That constraint does a lot of work here and the implementation should lean on it:

- There is no mid-subscription upgrade or downgrade to handle. Every plan transition happens from
  `trial` or `expired`.
- During `expired` the account is already fully blocked — `SUBSCRIPTION_OK` false →
  `has_active_session()` false → `sidecar::is_ready()` false → no indexing, no search. The library
  just sits frozen on disk.
- **The billing cycle is therefore the rate limit, for free.** Swapping a working set costs a full
  plan period *plus* the dead time between expiry and re-purchase, during which the tool does not
  work at all. No separate rebirth throttle is needed; keep `quota_rebirth_count` and
  `quota_last_rebirth_at` purely as telemetry so you can spot anything strange.

A rebirth is still a real capability — a designer who retires one collection and starts another is a
legitimate customer — it is simply one that cannot be abused at any useful rate.

---

## 2. Two ledgers, and what each is actually for

| Ledger | Lives | Records | Used for |
|---|---|---|---|
| `trial_ledger` | Trial only | Every hash ever indexed during trial, append-only | **Abuse signal only.** Reported to the server. Never shown to the user, never enforced against, never used for plan sizing |
| `usage_ledger` | Post-birth | Every hash indexed since birth, append-only | **The meter.** Enforcement, the quota strip, `used` |

`usage_ledger` is **empty during trial** and is populated once, at birth.

**Important — the purchase card must show the LIVE library size, not the trial ledger total.**
Live size is `SELECT COUNT(DISTINCT hash) FROM files`. If a trial user indexes 50k and removes 20k,
they need a 30k plan, not a 50k plan. Showing the trial ledger total would oversell them and read as
a bait-and-switch the first time someone checks. The trial ledger exists so *you* can see that an
account churned 500k images through a 14-day trial — that is fraud signal, not a sales input.

---

## 3. Storage: a separate `usage.db`

Put all three tables in **`~/.pictoria/usage.db`**, not `meta.db`. One decision, four problems solved:

| Problem | Resolution |
|---|---|
| `run_library_reset()` deletes `meta.db` ([migrate.rs:121](../../UI/src-tauri/src/core/migrate.rs)) — a future release would silently zero everyone's meter | `usage.db` is not in its delete list |
| Tests hand-duplicate the schema in `mem()` ([database.rs:750-757](../../UI/src-tauri/src/core/database.rs)) and again at `:822-825` | `files` schema untouched — `mem()` needs **no edit** |
| The ledgers must never be decremented | A file whose tables are append-only makes the invariant physically obvious |
| Migration risk to `open()`'s batch ([database.rs:27-64](../../UI/src-tauri/src/core/database.rs)) | Zero — `open()` is not modified |

```sql
-- ~/.pictoria/usage.db
CREATE TABLE IF NOT EXISTS usage_ledger (      -- the meter; born once
    hash       TEXT PRIMARY KEY,
    first_seen REAL NOT NULL
);
CREATE TABLE IF NOT EXISTS trial_ledger (      -- abuse signal only
    hash       TEXT PRIMARY KEY,
    first_seen REAL NOT NULL
);
CREATE TABLE IF NOT EXISTS app_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

`app_meta` keys: `quota_mode`, `quota_born`, `quota_birth_count`, `quota_birth_at`, `quota_cap`,
`quota_cap_source`, `quota_cap_at`, `quota_enforce`, `quota_rebirth_grant`, `quota_keep_deadline`.

**Rule:** expose `fn init_schema(con: &Connection) -> Result<()>` called by **both** `open()` and the
test helper. Never hand-duplicate DDL in `#[cfg(test)]` — that is the mistake `database.rs` already made.

---

## 4. Four corrections — verified against source, do not skip

**4.1 — A quota `Err` from `sync_folder` would destroy the library on a re-embed release.**
Verified: `sync_one` runs `reembed_folder` at [watcher.rs:526](../../UI/src-tauri/src/core/watcher.rs)
(tombstoning every vector via `store.remove(&existing_ids)` at
[sync_service.rs:281](../../UI/src-tauri/src/services/sync/sync_service.rs), rebuilding **in memory
only**), then `sync_folder` at `:532`. `store.save()` happens **only in the `Ok` arm**; the `Err` arm
logs, sets status `"error"`, emits, and returns **without saving**. Meanwhile `reconcile_all()`
clears `.reembed_pending` **unconditionally** at [watcher.rs:204](../../UI/src-tauri/src/core/watcher.rs).
Net effect for an over-cap user on a schema-bump release: `vectors.bin` deleted by
[migrate.rs:184](../../UI/src-tauri/src/core/migrate.rs), rebuild discarded, marker retired, search
permanently empty.

→ **The refusal must be an `Ok` outcome.** Change `sync_folder` to return `Result<SyncReport>` where
`SyncReport { errors: Vec<FileError>, quota_block: Option<QuotaBlock> }`. `sync_one` always saves on
`Ok` and branches on `quota_block` for the terminal event.

**4.2 — There is no sound cheap pre-flight for `add_folder`.** See §6.

**4.3 — A 402 will never reach Angular's `err.statusCode`.** `add_folder` spawns a thread
([library_service.rs:114](../../UI/src-tauri/src/services/library/library_service.rs)) and returns 200
immediately at `:129`; the thread's result is discarded. Add `PictoriaError::QuotaExceeded` anyway
(correct modelling; `status_code()`'s exhaustive match forces the mapping), but **the Angular contract
is an event, not a status code**.

**4.4 — Do NOT ship this together with a `LIBRARY_RESET_VERSION` bump.** `run_library_reset()` deletes
`meta.db` at [lib.rs:278](../../UI/src-tauri/src/lib.rs), *before* `run_startup()` at `:301`. If both
fire on one launch, `files` is empty at birth → the 12 pre-rollout users get birth count `0` → all 12
break on upgrade day. Hard release constraint; §11 adds a safety net.

---

## 5. The gate: exact placement

Insert at **[sync_service.rs:69](../../UI/src-tauri/src/services/sync/sync_service.rs)** — between the
parallel hash pass and the `needs_describe` loop.

```
sync_folder(store, folder):
  :22   if !sidecar::is_ready()  -> Err(ModelNotReady)          [unchanged]
  :30   con = database::open();  usage = quota::open()          [NEW]
  :32   cleanup_missing_in_folder                               [unchanged]
  :38   current_files = scan_images(folder)
  :49   hash_results  = par_iter hash pass                      [unchanged]
  ──────────────────────────────────────────────────── NEW GATE
        let scanned: HashSet<String> = hash_results.iter()
              .filter_map(|(_, h, _, _, _)| h.clone()).collect();
        // Trial short-circuits: unlimited, and only trial_ledger records.
        if quota::enforced() {
            let ledger = quota::ledger_hashes(&usage)?;
            let snap   = quota::snapshot(&usage)?;
            if let Decision::Refuse(block) =
                   quota::decide(&scanned, &ledger, snap.used, snap.effective_cap) {
                return Ok(SyncReport { errors, quota_block: Some(block) });
            }
        }
  ─────────────────────────────────────────────────────────────
  :70   needs_describe loop                                     [unchanged]
  :128  index_chunks(store, &con, &usage, &needs_describe, ...)
        quota::heal_from_files(&usage, &con, &folder_str)?      [NEW]
  :132  deleted-hash sweep                                      [unchanged]
        Ok(SyncReport { errors, quota_block: None })
```

`quota::enforced()` is false whenever mode is `trial`, whenever the server kill switch is off, and
during the keep-selection grace window (§7).

**Why line 69 and not after the loop at 124.** Accuracy is identical — the charge is
`scanned − ledger`, and every `needs_describe.push` site resolves correctly:

| Push site | Ledger state | Charged? | Correct because |
|---|---|---|---|
| `already_indexed` `:85-87` | present | no | already paid for |
| `move_file` rename `:109` | present (old path) | no | allocates no `next_vector_id` |
| same-folder dup `:107` | present | no | 2 rows, 1 hash, 1 unit |
| cross-folder dup `:114` | present | no | same |
| genuinely new `:122` | absent | **yes** | correct |
| hash failure `:77-81` | `None`, excluded | no | never indexed |

Safety decides it: the loop **mutates before it finishes** — `move_file` renames at `:109`, and
`:118-121` does `store.remove` + `delete_file` for changed content. Gating at 69 means nothing has
been mutated when the refusal is made. Gating at 124 leaves partial mutation behind.

**Why this does not block re-embed.** `reembed_folder` runs from `sync_one` *before* `sync_folder`.
The gate is inside `sync_folder`, so re-embed always completes and — with §4.1 — its rebuilt vectors
are always saved even when the subsequent sync refuses.

**Recording.** In `index_chunks`, accumulate successfully-inserted hashes and write them right after
the existing `COMMIT; BEGIN;` at `:244` via `quota::record_many`, which writes to **`trial_ledger`
when mode is trial, `usage_ledger` otherwise**. A file whose sidecar `describe` failed (`:193-200`)
never reaches `insert_file` and is never charged.

**Heal sweep** (`quota::heal_from_files`) closes the reverse gap (crash, or a pre-quota build): read
hashes from `meta.files` under the folder prefix into a `Vec`, batch `INSERT OR IGNORE`. Use
`normalise_folder_prefix` + `LIKE`, matching `folder_hashes`'s predicate. Run it **before** the
deleted-hash sweep at `:132` so hashes about to be swept are banked first. Heals into whichever
ledger the current mode writes to; **never** into `usage_ledger` before birth.

**`sync_one` branch:** always save the store on `Ok`, then if `quota_block.is_some()` set status
`"blocked"` and emit `library_quota_blocked` instead of `library_sync_complete`.

**New status `blocked`.** Do not auto-remove the `watched_folders` row on refusal — one code path
then covers both a too-big new folder and an existing folder that grew past the cap.
`watched_folder_paths()` excludes only `'paused'`, so blocked folders keep reconciling and unblock
automatically after an upgrade.

---

## 6. How the refusal surfaces: event-only, no hashing pre-flight

Leave `add_folder` behaviourally unchanged. The refusal arrives as a `library_quota_blocked` event
from `sync_one`.

**Why no inline pre-flight:**

1. **Doubles the happy-path work for nothing.** The gate already fires before any embedding — the only
   expensive step. Time-to-refusal is `scan_images` + `fast_hash`, exactly what a pre-flight redoes.
2. **`fast_hash` is sub-ms but `scan_images` on a NAS is not.** 20k files over SMB is tens of seconds
   and ~1.25 GB over the wire. This project ships a macOS NAS remount recovery loop
   ([watcher.rs:320-355](../../UI/src-tauri/src/core/watcher.rs)) — network shares are first-class.
3. **`library_add_folder` is a synchronous command** — blocking it freezes an IPC worker and spins the
   global loader with no progress. The event path streams "Scanning files" through the existing
   `library_sync_progress` ticker and then refuses: a progress bar beats a frozen button.

**And no *cheap* pre-flight exists.** "Refuse when `used >= cap`" is wrong, because re-adding ledgered
images must stay free. The carve-out "…unless the folder already has `files` rows" fails because
`remove_folder(purge=true)` deletes those rows — and **`purge=true` is the only delete button wired
up** (`removeFolder(f, false)` is commented out at
[library.html:181-188](../../UI/src/app/views/library/library.html)). So the common re-add path would
be falsely refused. Testing membership against the *ledger* requires hashing, returning to point 1.

**What replaces the 402:** an **advisory** client guard. `library.ts::addFolder()` calls
`libSvc.quota()` before opening the folder picker; if `remaining === 0`, show `PlansDialog` but let
the user proceed anyway (they may be re-adding). Advisory ⇒ no false-refusal risk.

**Accepted residual risk:** `add_folder` absorbs redundant child watched-folders at `:89-94` before any
quota decision. If the parent is later blocked, those rows are gone. Structurally mitigated — absorbed
children's images are already ledgered, so a parent containing only absorbed content adds zero new
hashes and cannot block. `log::warn!` and accept.

---

## 7. The keep-selection flow (birth)

Triggered when the server reports `mode: "select"` — i.e. the user has just purchased
and their live library exceeds the plan they bought.

**Server decides the mode**, client never infers it:

```
if subscription_status == 'trial'                    -> "trial"     (enforce=false, unlimited)
else if !born && created_at < ROLLOUT_TS             -> "birth_auto"(keep everything, §11)
else if !born                                        -> "select"    (first purchase)
else if effective_cap < used                         -> "select"    (they must shed)
else if rebirth_grant                                -> "select"    (manual support override)
else                                                 -> "enforced"
```

**`select` is a state (`effective_cap < used`), not an event.** Deriving it rather than firing it from
`verify-payment` makes every transition fall out correctly with no special cases:

Every transition is `trial → buy` or `expired → buy`; there is no mid-term plan change:

| Transition | Cap vs used | Mode | Dialog? |
|---|---|---|---|
| Trial → buy a plan that covers the library | used ≤ cap | `birth_auto` | no |
| Trial → buy a plan that does not | used > cap | **`select`** | **yes** |
| Expired → re-buy the same plan | unchanged | `enforced` | no |
| Expired → buy **larger** | cap rises above used | `enforced` | no |
| **Expired → buy smaller** | cap drops below used | **`select`** | **yes** |

A client that under-reports `used` to dodge `select` gains nothing: enforcement is local, against its
own ledger, which still holds the real count. `quota_rebirth_grant` survives only as a **manual support
override** (re-encoded catalogue, disk failure, goodwill), not as the primary trigger.

**Client flow on `select`:**

1. A blocking `ReconcileDialog` opens over the Library view — modelled on `PlansDialog`
   (`shared/plans-dialog/`), the app's only existing modal.
2. It lists every watched folder with its image count, a checkbox, a running
   **selected / cap** total, and an Apply button disabled until `selected <= cap`.
3. Apply calls a new `library_apply_keep_selection` command with the kept folder paths.

**Rust `quota::birth(kept_folders)`:**

```
verify app_meta['quota_rebirth_grant'] == "1"  (or !born)   -- else refuse, 403
for each folder NOT in kept:  library_service::remove_folder(path, purge = true)
BEGIN
  DELETE FROM usage_ledger                     -- only sanctioned clear, gated above
  INSERT INTO usage_ledger(hash, first_seen)
    <- SELECT DISTINCT hash FROM meta.files    -- the surviving (kept) set
  app_meta['quota_born']        = "1"
  app_meta['quota_birth_count'] = COUNT(*)
  app_meta['quota_birth_at']    = now
  DELETE app_meta['quota_rebirth_grant']
COMMIT
report birth_count to the server on the next validate; server clears its grant
```

The `DELETE FROM usage_ledger` is the **only** place the ledger is ever cleared, and it is
unreachable without a server-issued grant. Guard it with a loud `//` comment.

**Grace window.** Blocking search the instant someone pays is hostile; letting them search an over-cap
library forever is the loophole. So: on entering `select`, `app_meta['quota_keep_deadline'] = now + 7
days`. Inside the window everything works and the dialog is dismissible (re-prompted each launch).
Past it, a new `QUOTA_OK` atomic in `services::auth` — alongside the existing `SUBSCRIPTION_OK`
([auth_service.rs:32](../../UI/src-tauri/src/services/auth/auth_service.rs)) — flips false. Because
`has_active_session()` ([auth_service.rs:69](../../UI/src-tauri/src/services/auth/auth_service.rs))
feeds `sidecar::is_ready()`
([sidecar.rs:366](../../UI/src-tauri/src/core/sidecar.rs)), that single atomic gates **both indexing
and search** with no new plumbing — exactly how `exhausted` already works.

**Trial users who never purchase** need none of this: trial ends → `expired` → `SUBSCRIPTION_OK`
already blocks everything. Birth only ever happens on a purchase.

---

## 8. The purchase card

`PlansDialog` gains a live library figure and per-plan fit, turning plan choice into arithmetic the
customer can check.

- Header: **"Your library: 47,320 images"** — from `COUNT(DISTINCT hash) FROM files`, **not** the
  trial ledger (§2).
- Per plan card: `✓ Covers your library` when `image_quota >= live`, otherwise
  `Covers 25,000 — you'd remove 22,320`.
- Cheapest covering plan gets a "Recommended" ribbon.

Requires `plans.image_quota` to reach the client: add `imageQuota` to `Plan` in
`models/response/response_subscription.rs` **and** `responseSubscription.ts`, and return it from the
`get-plans` edge function (which currently returns only five fields).

Honesty note worth keeping: never show a plan as covering the library when it does not. The number on
this card is the single most load-bearing figure in the whole feature — if a customer buys a plan
believing it covers them and then gets a selection dialog, that is a refund and a bad review.

---

## 9. Supabase changes

```sql
ALTER TABLE public.plans
  ADD COLUMN image_quota integer;                    -- NULL => edge-function default

ALTER TABLE public.users
  ADD COLUMN image_quota_override    integer,        -- support/enterprise lever
  ADD COLUMN quota_born              boolean NOT NULL DEFAULT false,
  ADD COLUMN quota_birth_count       integer NOT NULL DEFAULT 0,
  ADD COLUMN quota_birth_at          timestamptz,
  ADD COLUMN quota_rebirth_grant     boolean NOT NULL DEFAULT false,  -- manual support override only
  ADD COLUMN quota_rebirth_count     integer NOT NULL DEFAULT 0,
  ADD COLUMN quota_last_rebirth_at   timestamptz,
  ADD COLUMN indexed_image_count     integer NOT NULL DEFAULT 0,   -- live, reported
  ADD COLUMN trial_ledger_count      integer NOT NULL DEFAULT 0,   -- abuse signal
  ADD COLUMN indexed_image_count_at  timestamptz,
  ADD COLUMN current_plan_id         uuid REFERENCES public.plans(id);

-- PLACEHOLDERS — these are what you change later, not the app.
UPDATE public.plans SET image_quota = 2000 WHERE duration = 30;
UPDATE public.plans SET image_quota = 2000 WHERE duration = 90;

UPDATE public.users u SET current_plan_id = s.plan_id
FROM (SELECT DISTINCT ON (user_id) user_id, plan_id
      FROM public.subscriptions WHERE status = 'active'
      ORDER BY user_id, end_date DESC) s
WHERE s.user_id = u.id;
```

**`current_plan_id` is not optional.** `verify-payment` currently writes only `subscription_status`
and `subscription_end`; there is no plan reference on `users` at all. **`verify-payment` must also
write `current_plan_id`** — without it the cap cannot be resolved and every purchase silently leaves
the buyer on the default quota.

`verify-payment` does **not** set `quota_rebirth_grant`. Mode is derived from `effective_cap < used`
in `validate-token-test` (§7), so buying a smaller plan produces `select` on the very next validate with no
event plumbing. The grant column exists only for manual support overrides.

RLS needs no work — edge functions use the service role. Do not add a client-writable path to any
quota column. Do not touch `search_count`; it is inert and its functions are broken.

**`validate-token-test` accepts** three new optional headers (stays a `GET`; add all three to the CORS
`Access-Control-Allow-Headers` beside `x-device-id`): `x-indexed-count` (live),
`x-trial-count`, `x-birth-count`.

**`validate-token-test` returns** a sibling of `onnx_key` (not a field on `user`, which mirrors the DB row):

```json
"quota": {
  "mode": "trial|select|enforced|birth_auto",
  "cap": 25000, "birthCount": 0, "effectiveCap": 25000,
  "source": "plan", "enforce": true, "keepDeadline": "2026-09-17T00:00:00Z"
}
```

Cap resolution: `users.image_quota_override` → `plans.image_quota` via `current_plan_id` → default
(`DEFAULT_PLAN_QUOTA` 2000; trial is uncapped so no trial default is needed). Then
`effective = MAX(planQuota, quota_birth_count)`.

**`enforce` is a server-controlled kill switch.** Ship Phase 5 with it `true` but flippable — a bad
rollout is then disabled everywhere without shipping a build.

---

## 10. Where the cap lives

| Layer | Holds | Purpose |
|---|---|---|
| `plans.image_quota` | **The real number** | Single source of truth. Change here, no release |
| `users.image_quota_override` | Per-account exception | Support lever |
| Edge function constant | `DEFAULT_PLAN_QUOTA` | A NULL plan must never mean unlimited or zero |
| `config.rs::QUOTA_FALLBACK_CAP` | Fallback only | Used **only** before the first successful validate on a fresh install |
| `app_meta['quota_cap']` | Last server-issued cap | Honoured during the offline window |
| Angular | **Nothing** | Reads `library_quota` / `get-plans`. No number hardcoded in TS, ever |

```rust
// config.rs
// Used only until the first successful validate-token-test on a fresh install —
// plans.image_quota on Supabase is the source of truth.
pub const QUOTA_FALLBACK_CAP: usize = 2_000;
// How long a server-issued cap is honoured without a refresh. Matched to
// OFFLINE_GRACE_SECS: past that the session itself is already gone.
pub const QUOTA_CACHE_MAX_AGE_SECS: i64 = OFFLINE_GRACE_SECS;
// Grace period after a purchase before an unreconciled over-cap library
// stops indexing AND search.
pub const QUOTA_KEEP_GRACE_SECS: i64 = 7 * 24 * 3600;
```

Turning a placeholder into a real number = one `UPDATE public.plans`. Clients pick it up on their next
validate. **This holds only as long as no cap number is compiled into Rust or TS except
`QUOTA_FALLBACK_CAP` — treat it as an invariant and enforce it in review.**

---

## 11. Birth for the 12 pre-rollout users (`birth_auto`)

They are mid-subscription and must never see a selection dialog — they already paid. Their ledger is
born automatically from their whole library, and the birth count becomes their floor.

`quota::birth_auto()`, called from `lib.rs setup()` right after `migrate::run_startup()` at
[lib.rs:301](../../UI/src-tauri/src/lib.rs), guarded by `app_meta['quota_born']`, one transaction:

```
if app_meta['quota_born'] -> return
if server mode != "birth_auto" -> return          -- never guess locally
BEGIN
  INSERT OR IGNORE INTO usage_ledger(hash, first_seen)
    <- SELECT DISTINCT hash FROM meta.files       (whole table, no folder predicate)
  app_meta['quota_born']        = "1"
  app_meta['quota_birth_count'] = COUNT(*)
  app_meta['quota_birth_at']    = now
COMMIT
```

Ordering, all load-bearing: **after** `run_library_reset()` (`:278`, else it births from a `meta.db`
about to be deleted); **after** `run_startup()` (`:301`); **before** anything can index (indexing only
starts via the readiness fan-in from `auth_service.rs:207/288`, strictly later than `setup()`).

The birth count is **captured once and never recomputed**. Recomputing would make the floor track the
ledger, and the cap would be infinite for everyone forever.

Server side accepts a birth count once per user, only for `created_at < ROLLOUT_TS`, bounded by
`FLOOR_SANITY_MAX` (100 000), then sets `quota_born = true`.

**Safety net for §4.4:** in `run_library_reset()`, *before* `remove_file(DB_PATH)` at `:121`, write
`SELECT COUNT(DISTINCT hash) FROM files` to `~/.pictoria/.quota_carry`; `birth_auto` uses
`max(ledger_count, carried_count)`. Cheap insurance — take it.

**Close-out:** ~a week after release, audit `users.quota_birth_count` for the 12 by hand, then set real
`plans.image_quota` values. With 12 accounts this is cheap and is the strongest possible guarantee on
a client-reported number.

---

## 12. Edge cases

| # | Case | Behaviour |
|---|---|---|
| 12.1 | Trial + unlimited = trial abuse | Sign up, index 500k, churn, repeat. `device_id` binding limits it to one machine per account; `trial_ledger_count` is the detection signal. **Watch it, don't pre-engineer against it** |
| 12.2 | Trial user removes folders before buying | Purchase card shows live count, so they are sized on what they kept. Correct and deliberate (§2) |
| 12.3 | Purchase of a plan that already covers the library | Server returns `birth_auto`, no dialog. Birth from everything |
| 12.4 | User dismisses the selection dialog | Re-prompted each launch; everything works until `keepDeadline`, then `QUOTA_OK` false blocks indexing **and** search |
| 12.5 | Expired → re-buy same or larger | `used <= cap` → `enforced`, no dialog. Ledger keeps growing under the new cap |
| 12.6 | Expired → buy smaller | `effective_cap < used` → `select`. Ledger reborn from the kept set. **This is the case worth testing by hand** |
| 12.7 | Working-set rotation via re-purchase | Buy big → let it expire → buy small → shed → let that expire → buy big again. Each swap costs a **full plan period plus the dead gap** where nothing works. Naturally rate-limited by the no-mid-term-change rule; no extra throttle needed. Keep `quota_rebirth_count` / `quota_last_rebirth_at` as telemetry only |
| 12.8 | **Purchase card must work while `expired`** | Expired accounts are exactly who is buying, and `sidecar::is_ready()` is false for them. The live-count command **must not** be gated on readiness or session — follow the `list_folders` / `stats` precedent ([library_service.rs:14-42](../../UI/src-tauri/src/services/library/library_service.rs)), which read SQLite directly with no gate. Gate it by mistake and every expiring customer sees `0 images` and buys the smallest plan |
| 12.9 | Long lapse before re-purchase | Library and ledger sit frozen on disk through the whole gap; folders may have drifted on disk meanwhile. The post-purchase reconcile picks that up on the next sync — nothing special to do, but the counts shown in the selection dialog come from the last sync, so refresh folder counts when the dialog opens |
| 12.10 | Grandfathered floor vs a smaller re-purchase | A pre-rollout user with `birth_count` 50k keeps `effective_cap = MAX(plan, 50k)` forever — so buying a smaller plan later would be meaningless for them. **Recommendation: re-purchasing at a smaller plan retires the floor** (`effective_cap = plan_quota` from then on). They chose it. Flagged as a decision — override if you'd rather keep the promise unconditional |
| 12.11 | Paused folders | Never sync, but rows + ledger persist and still count. Correct — they stay searchable. **Say so in UI copy: pausing does not free quota** |
| 12.12 | `purge=false` removal | Rows stay, `used` unchanged. Dead path — button commented out |
| 12.13 | `purge=true` removal post-birth | Rows deleted, ledger keeps every hash, `used` unchanged. **This is the anti-rotation property.** Unit test + manual step 4 |
| 12.14 | `move_file` rename | No new id, hash ledgered → zero cost |
| 12.15 | Deleted-hash sweep `:132-144` | Ledger untouched → `used` unchanged. On a block the gate returns before it runs |
| 12.16 | `run_library_reset()` | Solved by §3 + `.quota_carry`. **Add a `//` comment there: `usage.db` is deliberately not deleted** |
| 12.17 | `EMBED_SCHEMA_VERSION` bump | Safe *provided* §4.1 lands. Also fix `reconcile_all()` to clear the marker only when every folder completed clean and unblocked |
| 12.18 | Offline grace | Needs no handling: past the window the session tears down, `is_ready()` false, `sync_folder` returns `ModelNotReady` **before** the gate. Honour a stale cap, mark it stale for display |
| 12.19 | Concurrent syncs | No TOCTOU — `sync_one` holds `store_io_guard()` ([watcher.rs:470](../../UI/src-tauri/src/core/watcher.rs)) for its whole body and every caller funnels through it. **Do not narrow this guard** |
| 12.20 | Re-encoded catalogue | Different bytes → different hash → charged twice. Inherent; this is why `image_quota_override` exists |
| 12.21 | Blocked-folder churn | Every dropped file retriggers scan+hash after debounce. Emit `library_quota_blocked` only when `(used, cap, folder_new)` changed, or throttle to 1/60s per path |
| 12.22 | `'exhausted'` status | Already blocks via `SUBSCRIPTION_OK`. Keep orthogonal — quota must **not** write `subscription_status`; use the separate `QUOTA_OK` atomic |
| 12.23 | Missing SCSS / i18n | `folder-status--{{ f.status }}` ([library.html:58](../../UI/src/app/views/library/library.html)) renders unstyled without `.folder-status--blocked`; a missing `library.status.blocked` key renders the raw key |
| 12.24 | Footer vs quota mismatch | After a purge, client-computed `totalImages()` drops while `used` doesn't → "1,200 images" beside "2,000 / 2,000 used". **Handle in copy, not code**: "counted against your plan (includes removed images)" |

**Two pre-existing bugs found nearby** (orthogonal — fix or ticket, don't silently inherit):

- `files_by_hashes` ([database.rs:423-439](../../UI/src-tauri/src/core/database.rs)) builds an
  unbounded `IN (?,?,…)`. Emptying a 40k folder exceeds `SQLITE_MAX_VARIABLE_NUMBER` and errors the
  whole sync. Chunk it.
- The deleted-hash sweep has **no folder predicate** — removing a duplicate from folder A un-indexes
  the copy in folder B.

---

## 13. Phases

**Dependency rule: enforcement (Phase 5) must not ship before keep-selection (Phase 4)**, or a user
who purchases while over cap is trapped — blocked with no way to shed.

**Phase 0 — Safety refactor, zero behaviour change.** `sync_folder` → `Result<SyncReport>`; `sync_one`
saves on every `Ok`; `reconcile_all` clears the marker only on a fully-clean pass; add
`PictoriaError::QuotaExceeded { new, used, cap }` → 402. *First because it makes §4.1 impossible to get
wrong later and fixes a live re-embed bug on its own.*

**Phase 1 — Ledgers + meter, no enforcement.** `config.rs` constants; new `core/quota.rs`
(`open`/`init_schema`/`used`/`live_count`/`ledger_hashes`/`record_many`/`heal_from_files`/`birth`/
`birth_auto`/`decide`/`snapshot`); `birth_auto()` in `lib.rs`; `index_chunks` records into the
mode-appropriate ledger; `sync_folder` heals; `library_quota` command + `ResponseLibraryQuota`.
Register in `invoke_handler![]` in `lib.rs`. *No gate.*

**Phase 2 — Server plumbing, still no enforcement.** All DDL; `verify-payment` writes
`current_plan_id`; `validate-token-test` derives `mode` from `effective_cap < used` and accepts the
three headers and returns the `quota` block with `mode`; `validate_saved_token` sends them and calls
`quota::store_server_quota(...)` — **absence of a `quota` block (older server) must leave the cached
cap untouched**; add the `QUOTA_OK` atomic beside `SUBSCRIPTION_OK`.

**Phase 3 — Purchase card.** `imageQuota` on `Plan` (Rust + TS) and in `get-plans`; live library figure
and per-plan fit in `PlansDialog`. *Standalone value — ships before anything can block anyone.*

**Phase 4 — Keep-selection flow.** `ReconcileDialog` in `shared/`; `library_apply_keep_selection`
command; `quota::birth(kept)`; the grace window and `QUOTA_OK` enforcement past `keepDeadline`.

**Phase 5 — Enforcement.** The gate; `QuotaBlock`; `blocked` status; `ResponseLibraryQuotaBlocked` +
`library_quota_blocked` event + dedupe; `quota::enforced()`. **Verify the kill switch before shipping.**

**Phase 6 — Library UI.** `'blocked'` in `WatchedFolderStatus` + SCSS + i18n key; `LIBRARY_QUOTA` /
`LIBRARY_QUOTA_BLOCKED` in both const registries; `library.service.ts::quota()` + a fifth `listen` in
`onLibrarySync`; quota strip in the Library **header**
([library.html:3-15](../../UI/src/app/views/library/library.html) — visible with zero folders, unlike
the footer); `library.ts` imports `PlansDialog` into its `imports` array (currently only
`CommonModule, TranslateModule`), `@ViewChild`, opens **once** not per event; advisory pre-check in
`addFolder()`; new `library.*` keys in `en.json`. Pass a pre-translated `reason` into
`plansDialog.open(reason?)` — `plans-dialog.html` has no `TranslateModule`. Use `p-progressbar` per
`.claude/rules/ui-framework.md`.

**Phase 7 — Close-out.** Audit the 12 birth counts, set real `plans.image_quota` values.

---

## 14. Verification

**Design requirement:** extract `quota::decide(scanned, ledger, used, cap) -> Decision` as a **pure
function** so the gate is testable without a sidecar.

Rust unit tests in `core/quota.rs`, in the existing in-memory `Connection` style:

- `trial_mode_records_only_to_trial_ledger` — usage_ledger stays empty
- `trial_mode_never_refuses` — `enforced()` false regardless of counts
- `birth_seeds_usage_ledger_from_kept_files_only`
- `birth_requires_a_grant` — `birth()` when already born, with no grant and `used <= cap`, refuses
- `repurchase_below_used_enters_select` — born at 50k, new plan caps at 20k → mode `select`
- `repurchase_above_used_stays_enforced` — born at 20k, new plan caps at 50k → mode `enforced`, no dialog
- `renewal_same_plan_stays_enforced`
- `live_count_is_readable_while_subscription_blocked` — the purchase-card count works with
  `SUBSCRIPTION_OK` false (guards §12.8)
- `ledger_survives_folder_purge_after_birth` — 3 hashes, purge rows, `used()` still 3
- `readding_the_same_images_costs_nothing` — fully-ledgered set admits `new: 0` even at `used == cap`
- `duplicate_hashes_count_once`
- `refusal_is_all_or_nothing_at_the_boundary` — cap 2000/used 1999: `new=1` admits, `new=2` refuses with `shortfall=1`
- `birth_count_raises_a_smaller_plan_cap` / `birth_count_never_lowers_a_larger_plan_cap`
- `birth_auto_is_idempotent`, `birth_auto_counts_distinct_hashes_not_rows`
- `live_count_reflects_deletions_but_ledger_does_not` — guards the §2 purchase-card distinction
- `cap_falls_back_when_no_server_cap_cached`, `stale_cached_cap_is_still_honoured`

Also: serde round-trip on `ResponseLibraryQuota` asserting camelCase (`effectiveCap`), per the
`response_auth.rs` precedent; and an auth test that `validate-token-test` JSON **without** a `quota`
block still parses and leaves the cached cap untouched (guards the deploy-ordering race).

Angular (`npm test`): purchase card shows live count and marks the correct plan as covering;
`ReconcileDialog` disables Apply until `selected <= cap`; `library_quota_blocked` flips the card to
`blocked` and opens `PlansDialog` **once**, not per repeated event.

**Manual end-to-end** — set `plans.image_quota = 5` in Supabase (this itself proves the no-release property):

1. **Trial, unlimited.** Fresh trial account. Add 3 folders totalling 12 images → all index, no block,
   `usage_ledger` empty, `trial_ledger` = 12.
2. **Purchase card.** Open plans → header shows 12 images; a cap-5 plan shows "you'd remove 7"; a
   cap-20 plan shows "✓ Covers your library" and the Recommended ribbon.
3. **Buy the cap-5 plan → selection.** Dialog opens listing 3 folders. Apply is disabled until the
   selection is ≤ 5. Keep one 4-image folder → other two purged, `usage_ledger` = 4, `quota_born`.
4. **The attack.** Remove the kept folder with `purge=true`. Strip still shows **4 / 5** while `files`
   is empty. Re-add it → indexes, 0 new. Add a 3-image folder → **blocked**. ✅
5. **Nothing partial.** On that block verify `COUNT(*) FROM files` is unchanged and `vectors.bin` is
   untouched.
6. **Cap change, no release.** `UPDATE plans SET image_quota = 20`; wait for the revalidate tick;
   re-scan → indexes.
7. **Expiry → re-purchase smaller — the case that prompted this section.** Expire the subscription
   (`subscription_end` in the past) and confirm everything blocks. Then purchase a plan whose
   `image_quota` is below the ledger count. Next validate → mode flips to `select`, the dialog opens,
   Apply is disabled until the selection fits. Keep one folder → `usage_ledger` **rebuilt** from just
   that folder, `quota_birth_count` updated, `quota_rebirth_count` incremented.
8. **Expiry → re-purchase same or larger takes no dialog.** From that state, expire again and buy a
   plan at or above the ledger count → next validate is `enforced`, no dialog, ledger untouched,
   remaining rises.
9. **Purchase card while expired.** With the subscription expired (so `sidecar::is_ready()` is
   false), open the plans dialog and confirm the live library figure is still correct and non-zero.
   This is the §12.8 trap — an expired user is exactly who is buying.
10. **`birth_auto`.** On a machine with 8 images from the pre-quota build, install with cap 5:
   `quota_birth_count = 8`, no dialog, re-scan succeeds, one new image refused.
11. **Grace expiry.** Enter `select`, dismiss the dialog, set `keepDeadline` to the past → indexing
   **and search** both stop; complete the selection → both resume.
12. **Re-embed safety** (the one that would have bitten). On an over-cap machine bump
   `EMBED_SCHEMA_VERSION` locally: re-embed completes, `vectors.bin` rewritten, `.reembed_pending`
   cleared, search still returns results.
13. **Offline.** Pull the network after a successful validate → indexing continues on the cached cap;
    past the grace window it stops on `ModelNotReady`, never on a stale cap.
14. **Reset survival.** Delete `meta.db` only, leaving `usage.db` → re-add folders charges 0 new.
15. **Kill switch.** Set `quota.enforce = false` server-side → a blocked folder indexes after the next
    revalidate + re-scan, with no client rebuild.

---

## Critical files

| File | Change |
|---|---|
| `UI/src-tauri/src/core/quota.rs` | **New** — `usage.db`, both ledgers, `app_meta`, `birth`/`birth_auto`, pure `decide` |
| `UI/src-tauri/src/services/sync/sync_service.rs` | Gate at `:69`, mode-aware ledger write in `index_chunks`, heal sweep, `SyncReport` |
| `UI/src-tauri/src/core/watcher.rs` | `sync_one` always-save + `quota_block` branch; `reconcile_all` marker fix |
| `UI/src-tauri/src/services/auth/auth_service.rs` | Report headers, parse/cache the `quota` block, `QUOTA_OK` atomic |
| `UI/src-tauri/src/services/library/library_service.rs` | `apply_keep_selection` — purge unkept, then `quota::birth` |
| `UI/src-tauri/src/core/migrate.rs` | `.quota_carry` write in `run_library_reset()`, plus the "don't delete usage.db" comment |
| `UI/src-tauri/src/error.rs`, `config.rs`, `lib.rs` | New variant, constants, `birth_auto()` + command registration |
| `UI/src/app/shared/reconcile-dialog/` | **New** — folder keep-selection modal |
| `UI/src/app/shared/plans-dialog/` | Live library figure + per-plan fit |
| `UI/src/app/views/library/library.ts` / `.html` | Quota strip, `blocked` status, advisory pre-check |
| Supabase | DDL migration; `validate-token-test`, `verify-payment`, `get-plans` edge functions |
