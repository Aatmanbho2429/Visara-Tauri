import { Injectable, NgZone, inject } from '@angular/core';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { TauriService, UpdateInfo } from './tauri.service';

/**
 * App-wide updater state.
 *
 * This lives in a root service rather than in `Master` because the check used
 * to run in `Master.ngOnInit`, and `Master` only mounts at `/master/...` —
 * so on a cold start nothing checked for updates until the user had logged in
 * and landed on the shell. Worse, the result was held in `Master`'s own
 * fields: the check fired once, and if the reply arrived while the component
 * was mid-route-change the banner could be missed entirely, which is why it
 * looked like navigating was what "made the update button appear".
 *
 * Now the check starts as soon as the app boots (see `App.ngOnInit`) and the
 * answer is cached here, so whenever the shell renders the banner is already
 * populated — no navigation required. State is deliberately in-memory only:
 * `dismissed` resetting on relaunch is what makes the banner reappear on every
 * app open, which is the intended behaviour.
 */
@Injectable({ providedIn: 'root' })
export class UpdateService {
  info: UpdateInfo | null = null;
  dismissed = false;
  installing = false;
  /** 0-100, or null when the server sent no Content-Length (indeterminate). */
  pct: number | null = 0;
  error: string | null = null;

  private tauri = inject(TauriService);
  private zone = inject(NgZone);

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
      this.tauri.onUpdateAvailable(info =>
        this.zone.run(() => {
          this.info = info;
          this.error = null;
        }),
      ),
      this.tauri.onUpdateProgress(p =>
        this.zone.run(() => {
          this.installing = true;
          this.pct = p.total ? Math.min(100, Math.round((p.downloaded / p.total) * 100)) : null;
        }),
      ),
      this.tauri.onUpdateError(msg =>
        this.zone.run(() => {
          this.installing = false;
          this.error = msg;
        }),
      ),
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
    this.installing = true;
    this.pct = 0;
    this.error = null;
    this.tauri.installUpdate();
  }

  dismiss(): void {
    this.dismissed = true;
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
