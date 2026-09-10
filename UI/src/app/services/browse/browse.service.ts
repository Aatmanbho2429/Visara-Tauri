import { Injectable } from '@angular/core';
import { Observable } from 'rxjs';
import { TAURI_COMMANDS } from '../../core/tauri/tauri-commands.const';
import { ZoneWrapperService } from '../../core/zone-wrapper/zone-wrapper.service';
import { responseBrowseDirectory, responseGetThumbnail } from '../../models/response/responseBrowse';

@Injectable({ providedIn: 'root' })
export class BrowseService {
  constructor(private zoneWrapper: ZoneWrapperService) {}

  // List folders + images at a path (empty path = the watched roots).
  list(path: string): Observable<responseBrowseDirectory> {
    return this.zoneWrapper.invoke<responseBrowseDirectory>(TAURI_COMMANDS.BROWSE_DIRECTORY, { path });
  }

  // Cached thumbnail path for an image. The Browse grid fires dozens of
  // these concurrently — safe now that every invoke() is requestId-correlated
  // (see ZoneWrapperService), so it goes through the normal invoke() path
  // rather than a special-cased direct call.
  thumbnail(path: string): Promise<string> {
    return new Promise((resolve, reject) => {
      this.zoneWrapper.invoke<responseGetThumbnail>(TAURI_COMMANDS.BROWSE_GET_THUMBNAIL, { path })
        .subscribe({ next: res => resolve(res.path), error: reject });
    });
  }
}
