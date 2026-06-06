import { Injectable } from '@angular/core';
import { invoke } from '@tauri-apps/api/core';
import { Observable } from 'rxjs';
import { BaseResponse } from '../models/base-response.model';
import { BrowseData } from '../models/browse.model';
import { TauriService } from './tauri.service';

@Injectable({ providedIn: 'root' })
export class BrowseService {
  constructor(private tauri: TauriService) {}

  /** List folders + images at a path (empty path = the watched roots). */
  list(path: string): Observable<BaseResponse<BrowseData>> {
    return this.tauri.invoke<BrowseData>('browse_directory', { path });
  }

  /**
   * Cached thumbnail path for an image.  Returns directly (not via the event
   * loader) because the grid asks for many at once — the global-loader invoke
   * pattern would cross-wire them.
   */
  thumbnail(path: string): Promise<string> {
    return invoke<string>('get_thumbnail', { path });
  }
}
