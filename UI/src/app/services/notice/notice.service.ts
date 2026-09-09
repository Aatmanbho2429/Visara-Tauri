import { Injectable } from '@angular/core';
import { Observable } from 'rxjs';
import { TAURI_COMMANDS } from '../../core/tauri/tauri-commands.const';
import { ZoneWrapperService } from '../../core/zone-wrapper/zone-wrapper.service';

// Backs the one-time post-reset banner (see release-notice.ts) — whether a
// full library wipe happened this release, and recording that the user
// dismissed the notice.
@Injectable({ providedIn: 'root' })
export class NoticeService {
  constructor(private zoneWrapper: ZoneWrapperService) {}

  // True while the post-reset banner still needs showing.
  resetNoticePending(): Observable<boolean> {
    return this.zoneWrapper.invoke<boolean>(TAURI_COMMANDS.NOTICE_RESET_PENDING);
  }

  // Persist the dismissal so the banner does not return on next launch.
  dismissResetNotice(): Observable<null> {
    return this.zoneWrapper.invoke<null>(TAURI_COMMANDS.NOTICE_DISMISS_RESET);
  }
}
