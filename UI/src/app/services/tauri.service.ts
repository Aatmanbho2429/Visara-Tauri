import { Injectable, NgZone } from '@angular/core';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Observable } from 'rxjs';
import { BaseResponse } from '../models/base-response.model';

export interface SearchEvent {
  type: 'progress' | 'complete' | 'error';
  data: any;
}

@Injectable({ providedIn: 'root' })
export class TauriService {
  constructor(private zone: NgZone) {}

  // ── Generic auth invoke via events ────────────────────────────────
  invoke<T>(command: string, args?: Record<string, unknown>): Observable<BaseResponse<T>> {
    const eventName = `${command}_response`;
    return new Observable(observer => {
      let unlistenFn: (() => void) | null = null;

      this.zone.runOutsideAngular(() => {
        listen<BaseResponse<T>>(eventName, event => {
          this.zone.run(() => {
            observer.next(event.payload);
            observer.complete();
            unlistenFn?.();
          });
        }).then(unlisten => {
          unlistenFn = unlisten;
          invoke(command, args ?? {}).catch(err => {
            this.zone.run(() => {
              observer.next({ success: false, message: String(err), data: null as T });
              observer.complete();
              unlistenFn?.();
            });
          });
        });
      });
    });
  }

  // ── Search stream — emits progress/complete/error events ──────────
  searchStream(imagePath: string, folderPath: string, topK: number): Observable<SearchEvent> {
    return new Observable(observer => {
      let ulProgress: (() => void) | null = null;
      let ulComplete: (() => void) | null = null;
      let ulError:    (() => void) | null = null;

      const cleanup = () => { ulProgress?.(); ulComplete?.(); ulError?.(); };

      this.zone.runOutsideAngular(() => {
        Promise.all([
          listen<any>('search_progress', e => {
            this.zone.run(() => observer.next({ type: 'progress', data: e.payload }));
          }),
          listen<any>('search_complete', e => {
            this.zone.run(() => {
              observer.next({ type: 'complete', data: e.payload });
              observer.complete();
              cleanup();
            });
          }),
          listen<any>('search_error', e => {
            this.zone.run(() => {
              observer.next({ type: 'error', data: e.payload });
              observer.complete();
              cleanup();
            });
          }),
        ]).then(([p, c, er]) => {
          ulProgress = p;
          ulComplete = c;
          ulError    = er;

          invoke('start_search', { imagePath, folderPath, topK }).catch(err => {
            this.zone.run(() => {
              observer.next({ type: 'error', data: { message: String(err) } });
              observer.complete();
              cleanup();
            });
          });
        });
      });

      return cleanup;
    });
  }

  // ── Open file in OS explorer ──────────────────────────────────────
  openFilePath(path: string): void {
    invoke('open_file_path', { path }).catch(console.error);
  }
}
