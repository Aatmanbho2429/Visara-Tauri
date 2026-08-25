import { Injectable } from '@angular/core';
import { convertFileSrc } from '@tauri-apps/api/core';
import { SearchResult, FailedFile, SearchProgress } from '../views/search/search';

@Injectable({ providedIn: 'root' })
export class SearchStateService {

  searchState: 'idle' | 'searching' | 'results' = 'idle';

  imageName    = '';
  imagePath    = '';
  /** Base-64 data-URL shown in the pick-card when the query came from
   *  the clipboard via the global hot-key.  Empty for normal file picks. */
  imagePreview = '';
  /** Subset of watched-folder paths to search.  Empty array = all watched. */
  scopePaths:   string[] = [];

  results:      SearchResult[] = [];
  failedFiles:  FailedFile[]   = [];
  searchError   = '';
  showScrollTop = false;

  progress: SearchProgress = {
    phase: '', percent: 0, done: 0, total: 0,
    current: '', eta_sec: -1, errors: 0, active: false,
  };

  /** Capture a clipboard / hot-key image as the current query.
   *  A file copied from Finder/Explorer keeps its real name; only a captured
   *  bitmap (screenshot, browser "Copy Image") lands on the temp file, which we
   *  still label generically. */
  setClipboardImage(imagePath: string): void {
    const base = imagePath.split(/[\\/]/).pop() ?? imagePath;
    this.imagePath    = imagePath;
    this.imageName    = base === 'pictoria_clipboard.png' ? 'Clipboard image' : base;
    this.imagePreview = convertFileSrc(imagePath);
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
    this.progress      = { phase: '', percent: 0, done: 0, total: 0, current: '', eta_sec: -1, errors: 0, active: false };
  }
}
