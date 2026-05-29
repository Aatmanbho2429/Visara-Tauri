import { Injectable } from '@angular/core';
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
  topK         = 20;

  results:      SearchResult[] = [];
  failedFiles:  FailedFile[]   = [];
  searchError   = '';
  showScrollTop = false;

  progress: SearchProgress = {
    phase: '', percent: 0, done: 0, total: 0,
    current: '', eta_sec: -1, errors: 0, active: false,
  };

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
