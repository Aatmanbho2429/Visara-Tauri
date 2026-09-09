import { Injectable } from '@angular/core';
import { Observable } from 'rxjs';
import { TAURI_COMMANDS } from '../../core/tauri/tauri-commands.const';
import { TAURI_EVENTS } from '../../core/tauri/tauri-events.const';
import { ZoneWrapperService } from '../../core/zone-wrapper/zone-wrapper.service';
import { responseSearchComplete, responseSearchProgress, responseSidecarStatus } from '../../models/response/responseSearch';
import { responseSidecarCrashed } from '../../models/response/responseMisc';

export interface SearchEvent {
  type: 'progress' | 'complete' | 'error';
  data: any;
}

@Injectable({ providedIn: 'root' })
export class SearchService {
  constructor(private zoneWrapper: ZoneWrapperService) {}

  // Polled by the search screen to show/hide "Model is loading…" and gate
  // the Search button.
  sidecarStatus(): Observable<responseSidecarStatus> {
    return this.zoneWrapper.invokeSilent<responseSidecarStatus>(TAURI_COMMANDS.SIDECAR_STATUS);
  }

  // Search progress/complete/error, multiplexed into one stream by event
  // type — mirrors the three Tauri events `search_start` drives on the
  // Rust side.
  //
  // @param scopePaths Watched-folder paths to restrict the search to.  Empty
  //                    array means "search every folder in the Library".
  searchStream(imagePath: string, scopePaths: string[], topK: number): Observable<SearchEvent> {
    return new Observable(observer => {
      const subs = [
        this.zoneWrapper.listen<responseSearchProgress>(TAURI_EVENTS.SEARCH_PROGRESS).subscribe(res => {
          observer.next({ type: 'progress', data: res.data });
        }),
        this.zoneWrapper.listen<responseSearchComplete>(TAURI_EVENTS.SEARCH_COMPLETE).subscribe(res => {
          observer.next({ type: 'complete', data: res.data });
        }),
        this.zoneWrapper.listen<null>(TAURI_EVENTS.SEARCH_ERROR).subscribe(res => {
          observer.next({ type: 'error', data: { message: res.message } });
        }),
      ];

      // Search results are delivered entirely through the three broadcast
      // events above, not a `search_start_response` — fire-and-forget.
      this.zoneWrapper.invokeFireAndForget(
        TAURI_COMMANDS.SEARCH_START,
        { imagePath, scopePaths, topK },
        message => observer.next({ type: 'error', data: { message } }),
      );

      return () => subs.forEach(s => s.unsubscribe());
    });
  }

  onSidecarCrashed(cb: (payload: responseSidecarCrashed) => void): () => void {
    const sub = this.zoneWrapper.listen<responseSidecarCrashed>(TAURI_EVENTS.SIDECAR_CRASHED).subscribe(res => {
      if (res.data) cb(res.data);
    });
    return () => sub.unsubscribe();
  }
}
