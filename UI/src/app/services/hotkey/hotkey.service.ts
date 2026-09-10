import { Injectable } from '@angular/core';
import { TAURI_EVENTS } from '../../core/tauri/tauri-events.const';
import { ZoneWrapperService } from '../../core/zone-wrapper/zone-wrapper.service';
import { responseHotkeyPressed } from '../../models/response/responseMisc';

// The global hot-key (Ctrl+Shift+V / ⌘+Shift+V) captures whatever is on the
// clipboard and hands the resolved image path back through this event.
@Injectable({ providedIn: 'root' })
export class HotkeyService {
  constructor(private zoneWrapper: ZoneWrapperService) {}

  onHotkey(cb: (payload: responseHotkeyPressed) => void): () => void {
    const sub = this.zoneWrapper.listen<responseHotkeyPressed>(TAURI_EVENTS.HOTKEY_PRESSED).subscribe(res => {
      if (res.data) cb(res.data);
    });
    return () => sub.unsubscribe();
  }
}
