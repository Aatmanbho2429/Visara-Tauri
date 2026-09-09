import { Injectable } from '@angular/core';
import { TAURI_COMMANDS } from '../../core/tauri/tauri-commands.const';
import { ZoneWrapperService } from '../../core/zone-wrapper/zone-wrapper.service';

// Reveals a file in the OS's file manager (Explorer / Finder).
@Injectable({ providedIn: 'root' })
export class FileService {
  constructor(private zoneWrapper: ZoneWrapperService) {}

  openFilePath(path: string): void {
    // Silent — this is a fire-and-forget UI action, never worth the global
    // loader flicker.
    this.zoneWrapper.invokeSilent<null>(TAURI_COMMANDS.FILE_OPEN_PATH, { path })
      .subscribe({ error: err => console.error('[file] open failed:', err) });
  }
}
