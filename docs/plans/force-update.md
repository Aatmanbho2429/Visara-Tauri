# Force update — Implementation Plan

> **Status:** approved, not started.
>
> - [x] Phase 1 — Blocking update gate, client-only (no server changes)
> - [x] Phase 2 — Escape hatches (the part that stops this bricking people)
> - [x] Phase 3 — Code-level verification done (`cargo check`/`test`, `npm run
>       build`/`test` all clean, same pre-existing 4-failed/2-passed Angular
>       baseline). **The manual walkthrough in §3.2 was NOT run** — it needs a
>       release-mode build signed with the updater's private key, which isn't
>       available in this environment. Run it yourself before shipping.
> - [ ] Phase 4 — *Optional, deferred:* server-issued `minVersion` + kill switch
>
> **Phase 2 is not optional polish.** A force-update feature without an escape
> hatch is a remote brick switch pointed at every user. Phases 1 and 2 ship
> together or not at all.

---

## Amendment (post-implementation): download starts automatically

The plan as originally written required a click on "Update now" to start the
download — deliberately, per §2.5's reasoning about not surprising someone
with an unannounced large download. The user asked for it to start with no
click at all instead, so `UpdateService` now calls `install()` itself the
instant `update_available` fires, gated on the same `FORCE_UPDATES` check.
Only the *first* attempt is automatic — a failed install still lands on the
idle/error state with a manual Retry, rather than looping the download on its
own. See the comment at the `UPDATE_AVAILABLE` subscription in
`update.service.ts`.

**Also added:** a `localStorage` dev-mode preview switch, because §3.1's "do a
real release build" is the only other way to see the overlay at all, which is
painful to iterate on. Setting `pictoria_force_update_preview` to `'1'` from
DevTools and reloading makes `FORCE_UPDATES` true under `npx tauri dev` too —
it can only ever add forcing to a dev session, never remove it from a real
build (a release already has `!isDevMode()` true on its own). See the
constant's comment at the top of `update.service.ts`.

---

## What "force update" means here

Today an available update raises a **dismissible banner** (`UpdateService.dismiss()`
sets a flag, the banner hides, the user keeps working, and it reappears next
launch because the flag is in-memory only).

After this change: when an update is available, a **non-dismissible overlay**
covers the entire app — including the login screen — until the user installs it
or quits. Every released version becomes mandatory.

That is the literal reading of the request, and it needs no server, no database
column and no new edge function. The trade-off is that you lose the ability to
say "this particular release is optional" — see Phase 4 if you ever want that
back.

---

## Facts established before writing this (do not re-derive)

| Fact | Where | Consequence |
|---|---|---|
| `update_check` emits `update_available {version, notes}` / `update_not_available`; **check failure emits nothing, only `log::warn!`** | [update_commands.rs](../../UI/src-tauri/src/commands/update_commands.rs) | Fail-open is already the behaviour on a failed check. Keep it that way — §1.4 |
| `UpdateService.start()` runs from `App.ngOnInit`, **not** `Master` | [app.ts](../../UI/src/app/app.ts), [update.service.ts](../../UI/src/app/services/update/update.service.ts) | The check already runs pre-login, so an overlay at `App` level gets its signal with no re-plumbing |
| `app.html` is `<router-outlet /> + <app-global-loader /> + <p-toast />` | [app.html](../../UI/src/app/app.html) | The overlay goes here, as a sibling of the outlet — that is what makes it cover the login screen too |
| The updater endpoint is GitHub `releases/latest/download/latest.json`, `"dialog": false` | [tauri.conf.json](../../UI/src-tauri/tauri.conf.json) | Tauri's own dialog is off, so the UI is entirely ours. A broken `latest.json` is the main brick risk (§2) |
| `update_install` re-runs `check()` and reports every outcome, including `Ok(None)` | update_commands.rs | Already correct; no change needed to the install path itself |
| Angular runs **zoneless** (Angular 21) — `UpdateService` uses signals for this reason | update.service.ts | The overlay must read signals, never plain fields, or it will not repaint |
| **No `semver` crate dependency**, direct or otherwise declared | [Cargo.toml](../../UI/src-tauri/Cargo.toml) | Only matters for Phase 4. Phase 1 needs no version comparison at all |
| **No `opener`/`shell` plugin**; `file_open_path` shells out to `open`/`explorer` for *file paths* only | [file_commands.rs](../../UI/src-tauri/src/commands/file_commands.rs), [capabilities/default.json](../../UI/src-tauri/capabilities/default.json) | Opening the releases page in a browser needs a new command (§2.3) — do not assume a plugin exists |
| App version lives in **three** files that must match | CLAUDE.md | Phase 3's local test procedure edits all three |
| Closing the window hides to tray; it does not quit | [lib.rs](../../UI/src-tauri/src/lib.rs) | The overlay's "Quit" must call a real exit, not `window.close()` — §2.4 |

---

## Phase 1 — The blocking gate

### 1.1 `UpdateService` — add the mandatory state

In [update.service.ts](../../UI/src/app/services/update/update.service.ts):

- Add `readonly required = computed(() => this.info() !== null && FORCE_UPDATES);`
  where `FORCE_UPDATES` is a compile-time constant (see 1.5). With forcing on,
  "an update exists" *is* the mandatory signal — no extra field from the backend.
- `visible` (the old banner) becomes `computed(() => this.info() !== null && !this.required() && !this.dismissed())`
  so the banner and the overlay are mutually exclusive rather than both rendering.
- `dismiss()` must become a no-op when `required()` is true. Do not rely on the
  template simply not showing a dismiss button — the method is public and the
  service is injectable from anywhere.

Keep `installing`, `pct`, `error` exactly as they are; the overlay reuses them.

### 1.2 New component — `shared/force-update/force-update.ts`

Modelled on the existing dialogs, but **not** `p-dialog`: PrimeNG dialogs are
closable by Escape/mask-click by default and that is precisely what must not
happen here. A plain fixed-position overlay `div` is less code and has no
escape affordance to disable.

States it renders, all off existing signals:

| State | Shows |
|---|---|
| idle (`!installing()`) | version, notes, **Update now** button, Quit button |
| downloading (`installing()`, `pct() !== null`) | determinate bar at `pct()%` |
| downloading, no Content-Length (`pct() === null`) | indeterminate bar (the existing banner already handles this case — copy that logic) |
| failed (`error()`) | the error text, **Retry**, plus the §2 escape hatches |

Rules:
- No close button, no `(click)` on the backdrop, no Escape handler.
- `role="alertdialog"` + `aria-modal="true"`.
- It renders **above** `p-toast` and `app-global-loader` — set its `z-index`
  above both, and verify against the actual values in
  `assets/styles/components/_global-loader.scss` / `_toast.scss` rather than
  guessing a number.

### 1.3 Mount it in `app.html`

```html
<router-outlet />
<app-global-loader />
<p-toast position="top-center" key="app" />
<app-force-update />
```

Last so it paints over the rest; `App` imports it in the standalone `imports`
array. It self-hides when `updates.required()` is false, so no `@if` is needed
at the call site.

### 1.4 Do NOT make a failed check blocking

`update_check`'s `Err(e)` arm currently logs and emits nothing. **Leave it.**
No network, GitHub down, rate-limited, corporate proxy → no `update_available`
→ no overlay → the app works. Fail-open is the correct default for a gate whose
own signal depends on a third party being reachable.

Do not "improve" this by emitting `update_error` from the check path either:
`UpdateService` binds `UPDATE_ERROR` to the *install* error state, and a failed
background check would then render as a failed install in the overlay.

### 1.5 Release builds only

Forcing must not fire under `npm start` / `npx tauri dev`, or every dev session
with an older local version gets an unclosable overlay.

Gate `FORCE_UPDATES` on `environment.production` (or `isDevMode()` from
`@angular/core` — this project has no `environments/` directory, so check
before assuming one exists; `isDevMode()` needs nothing new).

---

## Phase 2 — Escape hatches

The failure this must survive: a release ships with a broken `latest.json`,
a bad signature, or a missing platform artifact. Every user is now staring at a
modal whose only button fails. Without this phase there is no way out short of
them finding the GitHub page themselves.

### 2.1 Always show the real error

`error()` already carries the backend's message. Render it verbatim — not a
friendly euphemism. "Signature verification failed" is what makes a support
email solvable in one round trip.

### 2.2 Retry must be available forever

Retry calls `updates.install()` again. No attempt counter, no lockout.

### 2.3 Manual download link — **needs a new Rust command**

There is no way to open an external URL today. Add
`commands/update_commands.rs::update_open_releases_page`, following the
existing `file_open_path` shape (shell out per-platform: `open` / `explorer` /
`xdg-open`) with the URL **hardcoded in Rust**, not passed from the webview —
never give the frontend an "open any URL" primitive.

- Register in `invoke_handler![]` in `lib.rs`.
- Add `UPDATE_OPEN_RELEASES_PAGE` to `tauri-commands.const.ts`.
- Emits `update_open_releases_page_response` like every other command
  (see `.claude/rules/tauri-ipc.md`).
- Also render the URL as selectable text, so it survives the command itself
  failing.

### 2.4 Quit button

Must actually terminate — the window's X hides to tray. Add
`update_quit_app` (or reuse an existing exit path) that calls
`crate::core::sidecar::shutdown()` then `app.exit(0)`, matching the tray's
`tray_quit` handler in [lib.rs](../../UI/src-tauri/src/lib.rs). Do not call
`window.close()` from Angular.

### 2.5 Accept the residual risk, in writing

With no server switch, a bad release is fixed by publishing a *good* release —
users' next check then offers the working one. Note this in the component's
header comment so the next person understands the recovery path without
re-deriving it. If that residual risk is unacceptable, do Phase 4.

---

## Phase 3 — Dev ergonomics and verification

### 3.1 How to test without publishing a release

Lower the local version below the published one so the real GitHub release
looks like an upgrade. All three must match (CLAUDE.md invariant):

- `UI/src-tauri/tauri.conf.json` → `"version": "0.0.1"`
- `UI/src-tauri/Cargo.toml` → `package.version = "0.0.1"`
- `UI/src-tauri/src/config.rs` → `APP_VERSION = "0.0.1"`

Then build a **release** bundle (`npx tauri build`) — a dev build won't force
(§1.5). Revert all three afterwards.

### 3.2 Cases to actually walk

1. Update available → overlay covers login screen **before** signing in.
2. Escape key, clicking the backdrop, and calling `updates.dismiss()` from the
   console all fail to dismiss it.
3. Install → progress bar moves → app restarts on the new version.
4. Offline (turn off wifi) → no overlay, app fully usable. **The most important
   case in this plan.**
5. Broken install: point `endpoints` at a URL returning a bad signature → error
   text, Retry, releases-page link and Quit all work.
6. Dev build (`npx tauri dev`) with a lowered version → banner, not overlay.

### 3.3 Existing tests

`cargo test` (24) and `npm test` must still pass. Note the Angular suite has a
**pre-existing** 4-failed/2-passed baseline from a jsdom/Tauri-mock issue
unrelated to this work — match that baseline, don't chase it.

---

## Phase 4 — *Optional, deferred:* server-issued `minVersion`

Only do this if "every release is mandatory" proves too blunt, or if the
Phase 2 residual risk is unacceptable and you want a remote off-switch.

Shape, reusing the pattern already proven in this codebase (`SUBSCRIPTION_OK`,
and the quota work's `enforce` kill switch):

- `validate-token-test` returns `"update": { "minVersion": "1.2.0", "force": true }`.
- `services::auth::validate_saved_token` parses it, compares against
  `config::APP_VERSION`, and stores the verdict — **absence of the field must
  leave behaviour unchanged**, same deploy-ordering rule as the quota block.
- Version comparison needs `semver` (not currently a dependency) — add it
  rather than hand-rolling string comparison, which gets `1.1.10` vs `1.1.9`
  wrong.

Advantages over Phase 1: forces can be applied *retroactively* to versions
already in the wild, and `force: false` is a one-field remote kill switch.

Costs: a Supabase schema + edge function change, and it only works for
logged-in users who can reach Supabase — so it **complements** Phase 1's
updater-driven signal rather than replacing it.

---

## Things that will bite

- **Blocking the login screen is deliberate**, not an accident of where the
  component mounts. An obsolete client is usually being forced *because* it no
  longer talks to the backend correctly; letting it log in first is worse.
- **`p-dialog` is the wrong tool** — its Escape/mask-close defaults are exactly
  the behaviour being designed out.
- **Zoneless Angular**: plain fields assigned from Tauri event callbacks do not
  repaint. Everything the overlay reads must be a signal.
- **The tray**: the app survives window close. A user can hide the overlay by
  closing the window; it returns on reopen. Acceptable — they cannot *use*
  anything meanwhile — but do not mistake it for a bypass to fix.
- **`dismiss()` is public.** Neutering it in the service matters as much as
  omitting the button.
- **Don't gate the Rust backend on this too** (no `UPDATE_OK` atomic alongside
  `SUBSCRIPTION_OK`). It buys nothing here — the overlay already blocks every
  path to those commands — and it adds a second way to lock the app out.
