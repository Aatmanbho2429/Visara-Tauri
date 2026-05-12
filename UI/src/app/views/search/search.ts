import { Component } from '@angular/core';
import { CommonModule } from '@angular/common';
import { TranslateModule } from '@ngx-translate/core';
import { PrimengComponentsModule } from '../../shared/primeng-components-module';

interface SearchResult {
  id: number;
  name: string;
  path: string;
  similarity: number;
  gradient: string;
}

@Component({
  selector: 'app-search',
  imports: [CommonModule, TranslateModule, PrimengComponentsModule],
  templateUrl: './search.html',
  styleUrl: './search.scss',
})
export class Search {
  searchState: 'idle' | 'searching' | 'results' = 'idle';

  imageName  = '';
  folderPath = '';

  get canSearch()   { return !!this.imageName && !!this.folderPath; }
  get isIdle()      { return this.searchState === 'idle'; }
  get isSearching() { return this.searchState === 'searching'; }
  get hasResults()  { return this.searchState === 'results'; }

  private gradients = [
    'linear-gradient(135deg,#d946ef 0%,#fb923c 100%)',
    'linear-gradient(135deg,#a21caf 0%,#f97316 100%)',
    'linear-gradient(135deg,#c026d3 0%,#ea580c 100%)',
    'linear-gradient(135deg,#86198f 0%,#d946ef 100%)',
    'linear-gradient(135deg,#701a75 0%,#fb923c 100%)',
    'linear-gradient(135deg,#e879f9 0%,#f97316 100%)',
  ];

  results: SearchResult[] = Array.from({ length: 16 }, (_, i) => ({
    id:         i + 1,
    name:       `asset_${String(i + 1).padStart(3, '0')}.jpg`,
    path:       `C:\\Designs\\Project\\Exports\\asset_${i + 1}.jpg`,
    similarity: Math.max(54, 97 - i * 3),
    gradient:   this.gradients[i % this.gradients.length],
  }));

  skeletons = Array(12).fill(0);

  pickImage()  { this.imageName  = 'reference_image.jpg'; }
  pickFolder() { this.folderPath = 'C:\\Users\\Designer\\Projects\\Assets'; }

  doSearch() {
    if (!this.canSearch) return;
    this.searchState = 'searching';
    setTimeout(() => this.searchState = 'results', 2200);
  }

  newSearch() {
    this.searchState = 'idle';
    this.imageName   = '';
    this.folderPath  = '';
  }

  openFile(path: string) { console.log('Open:', path); }

  similarityClass(sim: number) {
    if (sim >= 88) return 'high';
    if (sim >= 72) return 'mid';
    return 'low';
  }
}
