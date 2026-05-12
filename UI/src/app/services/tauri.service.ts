import { Injectable, NgZone } from '@angular/core';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Observable } from 'rxjs';
import { BaseResponse } from '../models/base-response.model';

@Injectable({ providedIn: 'root' })
export class TauriService {
  constructor(private zone: NgZone) {}

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
}
