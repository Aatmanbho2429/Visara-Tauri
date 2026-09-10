import { Injectable, NgZone } from '@angular/core';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { listen as tauriListen } from '@tauri-apps/api/event';
import { Observable } from 'rxjs';
import { ApiError, apiResponse } from '../../models/response/apiResponse';
import { LoaderService } from '../../services/loader/loader.service';

// The only file in the app that imports `@tauri-apps/api` — every Angular
// service talks to Rust exclusively through this wrapper. See
// .claude/rules/zone-wrapper.md.
//
// `invoke()`/`listen()` from `@tauri-apps/api` resolve outside Angular's
// zone, so callbacks are run through `NgZone` here to push results back in.
@Injectable({ providedIn: 'root' })
export class ZoneWrapperService {
  constructor(private zone: NgZone, private loader: LoaderService) {}

  // Monotonic counter behind the correlation id — cheap and collision-free
  // within one process lifetime, which is all a single invoke/response pair
  // needs. This is what lets `browse_get_thumbnail` (dozens of concurrent
  // calls from the Browse grid) match each `_response` event back to the
  // exact call that asked for it, instead of the first arrival racing every
  // pending caller for the same command.
  private static nextRequestId = 0;
  private static newRequestId(): string {
    return `req_${Date.now()}_${++ZoneWrapperService.nextRequestId}`;
  }

  // Generic invoke — auto shows/hides the global loader. Resolves with the
  // envelope's plain `data`, or errors with an `ApiError` when `statusCode`
  // falls outside 2xx, so callers never see the envelope itself.
  //
  // `args` is typed `object` rather than `Record<string, unknown>` on
  // purpose: callers pass concrete `request*` interfaces (see models/request/),
  // which — unlike object literals — TypeScript does not treat as having an
  // implicit index signature, so `Record<string, unknown>` would reject them.
  invoke<T>(command: string, args?: object): Observable<T> {
    return this.invokeInternal<T>(command, args, true);
  }

  // Same plumbing as `invoke()` but never touches the global loading
  // spinner. Used for background/periodic checks (e.g. the silent license
  // re-validation timer, or a status peek) that shouldn't visibly interrupt
  // the UI.
  invokeSilent<T>(command: string, args?: object): Observable<T> {
    return this.invokeInternal<T>(command, args, false);
  }

  private invokeInternal<T>(command: string, args: object | undefined, showLoader: boolean): Observable<T> {
    const eventName = `${command}_response`;
    const requestId = ZoneWrapperService.newRequestId();

    return new Observable(observer => {
      let unlistenFn: (() => void) | null = null;
      let settled = false;

      if (showLoader) this.loader.show();

      const finish = () => {
        if (showLoader) this.loader.hide();
        unlistenFn?.();
      };

      this.zone.runOutsideAngular(() => {
        tauriListen<apiResponse<T>>(eventName, event => {
          // Ignore responses to a different in-flight call to this same
          // command — the whole reason `requestId` exists.
          if (event.payload?.requestId !== requestId || settled) return;
          settled = true;

          this.zone.run(() => {
            const res = event.payload;
            if (res.statusCode >= 200 && res.statusCode < 300) {
              observer.next(res.data as T);
              observer.complete();
            } else {
              observer.error({ statusCode: res.statusCode, message: res.message } as ApiError);
            }
            finish();
          });
        }).then(unlisten => {
          unlistenFn = unlisten;
          invoke(command, { ...(args ?? {}), requestId }).catch(err => {
            if (settled) return;
            settled = true;
            this.zone.run(() => {
              observer.error({ statusCode: 0, message: String(err) } as ApiError);
              finish();
            });
          });
        });
      });

      return finish;
    });
  }

  // A local file path isn't loadable by the webview directly — this turns it
  // into the `asset://` URL that is. Pure/synchronous, but routed through
  // here anyway so this stays the only file importing `@tauri-apps/api`.
  toAssetUrl(path: string): string {
    return convertFileSrc(path);
  }

  // Fire a command with no matching `<command>_response` event — used only
  // by commands whose entire outcome is reported through broadcast events
  // instead (e.g. `search_start` reports via search_progress/_complete/_error).
  // Transport-level failures (the invoke promise itself rejecting) are
  // reported to `onTransportError` since there is no response envelope to
  // carry them.
  invokeFireAndForget(command: string, args: object, onTransportError: (message: string) => void): void {
    invoke(command, { ...args }).catch(err => onTransportError(String(err)));
  }

  // Subscribe to a broadcast event (search_progress, library_sync_*, …).
  // Returns the full envelope — unlike invoke(), a stream can carry more
  // than one outcome over its lifetime, so the caller decides what a given
  // `statusCode` means for that particular event rather than this wrapper
  // guessing on its behalf.
  listen<T>(event: string): Observable<apiResponse<T>> {
    return new Observable(observer => {
      let unlistenFn: (() => void) | null = null;

      this.zone.runOutsideAngular(() => {
        tauriListen<apiResponse<T>>(event, e => {
          this.zone.run(() => observer.next(e.payload));
        }).then(unlisten => { unlistenFn = unlisten; });
      });

      return () => unlistenFn?.();
    });
  }
}
