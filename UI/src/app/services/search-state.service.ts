import { Injectable } from '@angular/core';
import { SearchResult, FailedFile, SearchProgress } from '../views/search/search';

@Injectable({ providedIn: 'root' })
export class SearchStateService {

  searchState: 'idle' | 'searching' | 'results' = 'idle';

  imageName  = '';
  imagePath  = '';
  folderPath = '';
  topK       = 20;

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
    this.folderPath    = '';
    this.results       = [];
    this.failedFiles   = [];
    this.searchError   = '';
    this.showScrollTop = false;
    this.progress      = { phase: '', percent: 0, done: 0, total: 0, current: '', eta_sec: -1, errors: 0, active: false };
  }
}
