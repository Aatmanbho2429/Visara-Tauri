import { Injectable, computed, isDevMode, signal } from '@angular/core';
import { Subscription } from 'rxjs';
import { TAURI_COMMANDS } from '../../core/tauri/tauri-commands.const';
import { TAURI_EVENTS } from '../../core/tauri/tauri-events.const';
import { ZoneWrapperService } from '../../core/zone-wrapper/zone-wrapper.service';
import { responseUpdateAvailable, responseUpdateProgress } from '../../models/response/responseMisc';

// docs/plans/force-update.md §1.5 — every release is mandatory, but only in
// a real build. `npx tauri dev` must never present an unclosable overlay
// just because the checked-out working copy has an older version number
// than whatever is on GitHub. `isDevMode()` is false only for an optimized
// (`ng build` / `npm run build`) bundle — there is no `environments/`
// directory in this project to key off instead.
//
// Dev-mode preview switch: a full signed release build is the only other
// way to see the forced overlay at all, which makes it painful to iterate
// on. A Tauri window has no address bar, so this is a `localStorage` flag
// (same convention as `pictoria_hotkey_tip_seen_v1` / the library-migrate
// tip) rather than a URL param — set it from DevTools' console and reload:
//   localStorage.setItem('pictoria_force_update_preview', '1')
// This can only ever ADD forcing to a dev build; a real release already has
// `!isDevMode()` true on its own, so there is no flag anywhere that can be
// used to switch forcing OFF in a shipped build. Using it also triggers a
// genuine download/install attempt against whatever GitHub actually offers
// — that's the point, but know that going in.
const FORCE_UPDATE_PREVIEW_KEY = 'pictoria_force_update_preview';
function forceUpdatePreviewEnabled(): boolean {
  try {
    return localStorage.getItem(FORCE_UPDATE_PREVIEW_KEY) === '1';
  } catch {
    return false;
  }
}
const FORCE_UPDATES = !isDevMode() || forceUpdatePreviewEnabled();

// App-wide updater state.
//
// A service of signals rather than fields on `Master`, for two reasons:
//
// 1. Lifecycle. The check used to run in `Master.ngOnInit`, but `Master` only
//    mounts at `/master/...`, so a cold start did no check until the user had
//    logged in and reached the shell. The check now starts at app boot (see
//    `App.ngOnInit`) and the answer is cached here, so the banner is already
//    populated whenever the shell renders.
//
// 2. Change detection. This app runs zoneless (Angular 21, no zone.js), so
//    `NgZone` is a no-op and assigning to a plain field from a Tauri event
//    callback updates the value but never repaints. Signals notify the
//    scheduler directly, so writes from outside any framework context render
//    immediately.
//
// State is deliberately in-memory: `dismissed` resetting on relaunch is what
// makes the banner reappear on every app open, which is intended.
@Injectable({ providedIn: 'root' })
export class UpdateService {
  readonly info = signal<responseUpdateAvailable | null>(null);
  readonly dismissed = signal(false);
  readonly installing = signal(false);
  // 0-100, or null when the download has no Content-Length (indeterminate).
  readonly pct = signal<number | null>(0);
  readonly error = signal<string | null>(null);

  // Force-update overlay (docs/plans/force-update.md) — an update existing
  // at all IS the mandatory signal in a real build; there is no separate
  // "this release is optional" flag from the server. See Phase 4 in that
  // plan if that ever needs to change.
  readonly required = computed(() => this.info() !== null && FORCE_UPDATES);

  // Banner shows when an update is known, forcing is off (or this is a dev
  // build), and the user hasn't waved it away. Mutually exclusive with
  // `required` — the overlay replaces the banner entirely once forcing is on.
  readonly visible = computed(() => this.info() !== null && !this.required() && !this.dismissed());

  private started = false;
  private subs: Subscription[] = [];
  private timer: ReturnType<typeof setInterval> | null = null;

  // Re-check every 6 hours so a long-running tray session still gets told.
  private static readonly INTERVAL_MS = 6 * 60 * 60 * 1000;

  constructor(private zoneWrapper: ZoneWrapperService) {}

  // Idempotent — safe to call from more than one place.
  start(): void {
    if (this.started) return;
    this.started = true;

    // Listeners are registered before the first check is triggered: the
    // backend can emit `update_available` faster than `listen()` resolves,
    // and an event fired before we are listening is simply lost.
    this.subs = [
      this.zoneWrapper.listen<responseUpdateAvailable>(TAURI_EVENTS.UPDATE_AVAILABLE).subscribe(res => {
        if (!res.data) return;
        this.info.set(res.data);
        this.error.set(null);
        // In a real build this is also what makes `required()` true — the
        // download starts on its own the instant an update is detected, no
        // click needed. Only the FIRST attempt is automatic: if it fails,
        // `error()` is set and the overlay's Retry button (force-update.html)
        // is what the user clicks for every attempt after that — an update
        // that keeps silently re-hammering a broken install on its own is
        // worse than one that stops and asks.
        if (FORCE_UPDATES) this.install();
      }),
      this.zoneWrapper.listen<responseUpdateProgress>(TAURI_EVENTS.UPDATE_PROGRESS).subscribe(res => {
        if (!res.data) return;
        this.installing.set(true);
        const p = res.data;
        this.pct.set(p.total ? Math.min(100, Math.round((p.downloaded / p.total) * 100)) : null);
      }),
      this.zoneWrapper.listen<null>(TAURI_EVENTS.UPDATE_ERROR).subscribe(res => {
        this.installing.set(false);
        this.error.set(res.message);
      }),
    ];

    this.check();
    this.timer = setInterval(() => this.check(), UpdateService.INTERVAL_MS);
  }

  check(): void {
    this.zoneWrapper.invokeFireAndForget(TAURI_COMMANDS.UPDATE_CHECK, {}, msg => console.warn('[updater] check failed:', msg));
  }

  install(): void {
    this.installing.set(true);
    this.pct.set(0);
    this.error.set(null);
    this.zoneWrapper.invokeFireAndForget(TAURI_COMMANDS.UPDATE_INSTALL, {}, msg => {
      this.installing.set(false);
      this.error.set(msg);
    });
  }

  dismiss(): void {
    // A mandatory update cannot be waved away — this method is public and
    // injectable from anywhere, so the no-op has to live here rather than
    // relying on the overlay simply not rendering a dismiss button.
    if (this.required()) return;
    this.dismissed.set(true);
  }

  // docs/plans/force-update.md §2.3 — opens the GitHub releases page in the
  // system browser as a manual-download fallback when the in-app installer
  // itself is broken (bad signature, missing artifact, etc). The URL is
  // hardcoded on the Rust side, never passed from here. `update_open_
  // releases_page` does emit a `_response` event (see tauri-ipc.md), so this
  // goes through `invokeSilent()` like `FileService.openFilePath()`, not
  // `invokeFireAndForget()`.
  openReleasesPage(): void {
    this.zoneWrapper.invokeSilent<null>(TAURI_COMMANDS.UPDATE_OPEN_RELEASES_PAGE, {})
      .subscribe({ error: err => console.warn('[updater] could not open releases page:', err) });
  }

  // §2.4 — the window's X hides Pictoria to the tray rather than quitting,
  // so a real exit needs its own command; this must never be
  // `window.close()`.
  quitApp(): void {
    this.zoneWrapper.invokeFireAndForget(TAURI_COMMANDS.UPDATE_QUIT_APP, {}, () => {});
  }

  // Only meaningful in tests — the service lives for the app's lifetime.
  stop(): void {
    if (this.timer) clearInterval(this.timer);
    this.timer = null;
    this.subs.forEach(s => s.unsubscribe());
    this.subs = [];
    this.started = false;
  }
}
