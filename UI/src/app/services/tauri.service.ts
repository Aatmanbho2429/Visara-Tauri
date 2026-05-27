import { Injectable, NgZone } from '@angular/core';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { Observable } from 'rxjs';
import { BaseResponse } from '../models/base-response.model';
import { LoaderService } from './loader.service';

export interface UpdateInfo   { version: string; notes: string; }
export interface UpdateProgress { downloaded: number; total: number | null; }
export interface HotkeyEvent  { has_image: boolean; image_path: string; }

export interface SearchEvent {
  type: 'progress' | 'complete' | 'error';
  data: any;
}

@Injectable({ providedIn: 'root' })
export class TauriService {
  constructor(private zone: NgZone, private loader: LoaderService) {}

  // ── Generic invoke — auto shows/hides global loader ───────────────
  invoke<T>(command: string, args?: Record<string, unknown>): Observable<BaseResponse<T>> {
    const eventName = `${command}_response`;
    return new Observable(observer => {
      let unlistenFn: (() => void) | null = null;

      this.loader.show();

      this.zone.runOutsideAngular(() => {
        listen<BaseResponse<T>>(eventName, event => {
          this.zone.run(() => {
            this.loader.hide();
            observer.next(event.payload);
            observer.complete();
            unlistenFn?.();
          });
        }).then(unlisten => {
          unlistenFn = unlisten;
          invoke(command, args ?? {}).catch(err => {
            this.zone.run(() => {
              this.loader.hide();
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
  searchStream(imagePath: string, folderPath: string, topK: number, onnxKey: string): Observable<SearchEvent> {
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

          invoke('start_search', { imagePath, folderPath, topK, onnxKey }).catch(err => {
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

  // ── Updater ───────────────────────────────────────────────────────
  checkForUpdate(): void {
    invoke('check_for_update').catch(console.error);
  }

  installUpdate(): void {
    invoke('install_update').catch(console.error);
  }

  onUpdateAvailable(cb: (info: UpdateInfo) => void): Promise<UnlistenFn> {
    return listen<UpdateInfo>('update_available', e => cb(e.payload));
  }

  onUpdateProgress(cb: (p: UpdateProgress) => void): Promise<UnlistenFn> {
    return listen<UpdateProgress>('update_progress', e => cb(e.payload));
  }

  onUpdateError(cb: (msg: string) => void): Promise<UnlistenFn> {
    return listen<{ message: string }>('update_error', e => cb(e.payload.message));
  }

  // ── Global hot-key ────────────────────────────────────────────────
  onHotkey(cb: (payload: HotkeyEvent) => void): Promise<UnlistenFn> {
    return listen<HotkeyEvent>('hotkey_pressed', e => cb(e.payload));
  }
}
