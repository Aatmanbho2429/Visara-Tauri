import { Injectable, inject } from '@angular/core';
import { ZoneWrapperService } from '../../core/zone-wrapper/zone-wrapper.service';
import { FailedFile, SearchProgressSnapshot, SearchResult } from '../../models/response/responseSearch';

// View-model shape for one rendered result card — the backend fields plus
// client-only state (thumbnail loading, the "found inside" highlight box).
export interface SearchResultVM extends SearchResult {
  thumbnailUrl: string;
  imgError: boolean;
  matchBox: Record<string, string> | null;
}

@Injectable({ providedIn: 'root' })
export class SearchStateService {
  private zoneWrapper = inject(ZoneWrapperService);

  searchState: 'idle' | 'searching' | 'results' = 'idle';

  imageName    = '';
  imagePath    = '';
  // Base-64 data-URL shown in the pick-card when the query came from
  // the clipboard via the global hot-key.  Empty for normal file picks.
  imagePreview = '';
  // Subset of watched-folder paths to search.  Empty array = all watched.
  scopePaths:   string[] = [];

  results:      SearchResultVM[] = [];
  failedFiles:  FailedFile[]     = [];
  searchError   = '';
  showScrollTop = false;

  progress: SearchProgressSnapshot = {
    phase: '', percent: 0, done: 0, total: 0,
    current: '', etaSec: -1, errors: 0, active: false,
  };

  // Capture a clipboard / hot-key image as the current query.
  // A file copied from Finder/Explorer keeps its real name; only a captured
  // bitmap (screenshot, browser "Copy Image") lands on the temp file, which we
  // still label generically.
  setClipboardImage(imagePath: string): void {
    const base = imagePath.split(/[\\/]/).pop() ?? imagePath;
    this.imagePath    = imagePath;
    this.imageName    = base === 'pictoria_clipboard.png' ? 'Clipboard image' : base;
    this.imagePreview = this.zoneWrapper.toAssetUrl(imagePath);
  }

  reset(): void {
    this.searchState   = 'idle';
    this.imageName     = '';
    this.imagePath     = '';
    this.imagePreview  = '';
    this.scopePaths    = [];
    this.results       = [];
    this.failedFiles   = [];
    this.searchError   = '';
    this.showScrollTop = false;
    this.progress      = { phase: '', percent: 0, done: 0, total: 0, current: '', etaSec: -1, errors: 0, active: false };
  }
}
