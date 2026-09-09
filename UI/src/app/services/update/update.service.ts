import { Injectable, computed, signal } from '@angular/core';
import { Subscription } from 'rxjs';
import { TAURI_COMMANDS } from '../../core/tauri/tauri-commands.const';
import { TAURI_EVENTS } from '../../core/tauri/tauri-events.const';
import { ZoneWrapperService } from '../../core/zone-wrapper/zone-wrapper.service';
import { responseUpdateAvailable, responseUpdateProgress } from '../../models/response/responseMisc';

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

  // Banner shows when an update is known and the user hasn't waved it away.
  readonly visible = computed(() => this.info() !== null && !this.dismissed());

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
    this.dismissed.set(true);
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
