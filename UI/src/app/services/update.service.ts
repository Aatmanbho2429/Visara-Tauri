import { Injectable, computed, inject, signal } from '@angular/core';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { TauriService, UpdateInfo } from './tauri.service';

/**
 * App-wide updater state.
 *
 * Two things make this a service of signals rather than fields on `Master`:
 *
 * 1. Lifecycle. The check used to run in `Master.ngOnInit`, but `Master` only
 *    mounts at `/master/...`, so a cold start did no check until the user had
 *    logged in and reached the shell. The check now starts at app boot (see
 *    `App.ngOnInit`) and the answer is cached here, so the banner is already
 *    populated whenever the shell renders.
 *
 * 2. Change detection. This app runs **zoneless** — Angular 21 with no zone.js
 *    dependency at all — so `NgZone` is a no-op and `zone.run()` does not
 *    schedule change detection. Assigning to a plain field from a Tauri event
 *    callback updated the value but never repainted: the UI only caught up
 *    when something else triggered a render, which is why the progress bar
 *    appeared to move only on click and the banner only on navigation.
 *    Signals notify the scheduler directly, so writes from outside any
 *    framework context render immediately.
 *
 * State is deliberately in-memory: `dismissed` resetting on relaunch is what
 * makes the banner reappear on every app open, which is intended.
 */
@Injectable({ providedIn: 'root' })
export class UpdateService {
  readonly info = signal<UpdateInfo | null>(null);
  readonly dismissed = signal(false);
  readonly installing = signal(false);
  /** 0-100, or null when the download has no Content-Length (indeterminate). */
  readonly pct = signal<number | null>(0);
  readonly error = signal<string | null>(null);

  /** Banner shows when an update is known and the user hasn't waved it away. */
  readonly visible = computed(() => this.info() !== null && !this.dismissed());

  private tauri = inject(TauriService);

  private started = false;
  private unlisten: UnlistenFn[] = [];
  private timer: ReturnType<typeof setInterval> | null = null;

  /** Re-check every 6 hours so a long-running tray session still gets told. */
  private static readonly INTERVAL_MS = 6 * 60 * 60 * 1000;

  /** Idempotent — safe to call from more than one place. */
  start(): void {
    if (this.started) return;
    this.started = true;

    // Listeners are registered before the first check is triggered: the
    // backend can emit `update_available` faster than `listen()` resolves,
    // and an event fired before we are listening is simply lost.
    Promise.all([
      this.tauri.onUpdateAvailable(info => {
        this.info.set(info);
        this.error.set(null);
      }),
      this.tauri.onUpdateProgress(p => {
        this.installing.set(true);
        this.pct.set(p.total ? Math.min(100, Math.round((p.downloaded / p.total) * 100)) : null);
      }),
      this.tauri.onUpdateError(msg => {
        this.installing.set(false);
        this.error.set(msg);
      }),
    ]).then(fns => {
      this.unlisten = fns;
      this.check();
    });

    this.timer = setInterval(() => this.check(), UpdateService.INTERVAL_MS);
  }

  check(): void {
    this.tauri.checkForUpdate();
  }

  install(): void {
    this.installing.set(true);
    this.pct.set(0);
    this.error.set(null);
    this.tauri.installUpdate();
  }

  dismiss(): void {
    this.dismissed.set(true);
  }

  /** Only meaningful in tests — the service lives for the app's lifetime. */
  stop(): void {
    if (this.timer) clearInterval(this.timer);
    this.timer = null;
    this.unlisten.forEach(fn => fn());
    this.unlisten = [];
    this.started = false;
  }
}
