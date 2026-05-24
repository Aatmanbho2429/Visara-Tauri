import { Component, ElementRef, ViewChild, inject } from '@angular/core';
import { CommonModule } from '@angular/common';
import { TranslateModule } from '@ngx-translate/core';
import { open } from '@tauri-apps/plugin-dialog';
import { convertFileSrc } from '@tauri-apps/api/core';
import { PrimengComponentsModule } from '../../shared/primeng-components-module';
import { BaseComponent } from '../../core/base.component';
import { TauriService } from '../../services/tauri.service';
import { AuthService } from '../../services/auth.service';
import { UserStateService } from '../../services/user-state.service';
import { SearchStateService } from '../../services/search-state.service';
import { PlansDialog } from '../../shared/plans-dialog/plans-dialog';

export interface SearchResult {
  rank:         number;
  path:         string;
  name:         string;
  similarity:   number;
  thumbnailUrl: string;
  imgError:     boolean;
}

export interface FailedFile {
  file:   string;
  reason: string;
}

export interface SearchProgress {
  phase:   string;
  percent: number;
  done:    number;
  total:   number;
  current: string;
  eta_sec: number;
  errors:  number;
  active:  boolean;
}

@Component({
  selector:    'app-search',
  imports:     [CommonModule, TranslateModule, PrimengComponentsModule, PlansDialog],
  templateUrl: './search.html',
  styleUrl:    './search.scss',
})
export class Search extends BaseComponent {

  private tauri     = inject(TauriService);
  private auth      = inject(AuthService);
  private userState = inject(UserStateService);
  state             = inject(SearchStateService);

  @ViewChild('masonryGrid') masonryGridRef!: ElementRef<HTMLElement>;
  @ViewChild(PlansDialog)   plansDialog!: PlansDialog;

  readonly topKOptions = [10, 20, 50];

  get canSearch()   { return !!this.state.imagePath && !!this.state.folderPath; }
  get isIdle()      { return this.state.searchState === 'idle'; }
  get isSearching() { return this.state.searchState === 'searching'; }
  get hasResults()  { return this.state.searchState === 'results'; }

  get masonryColumns(): SearchResult[][] {
    const cols    = 3;
    const columns = Array.from({ length: cols }, (): SearchResult[] => []);
    this.state.results.forEach((item, i) => columns[i % cols].push(item));
    return columns;
  }

  constructor() { super(); }

  async pickImage() {
    const selected = await open({
      multiple: false,
      filters:  [{ name: 'Images', extensions: ['jpg','jpeg','png','tif','tiff','psd','psb'] }]
    });
    if (selected) {
      this.state.imagePath = selected as string;
      this.state.imageName = (selected as string).split(/[\\/]/).pop() ?? selected as string;
      this.cdr.detectChanges();
    }
  }

  async pickFolder() {
    const selected = await open({ directory: true, multiple: false });
    if (selected) {
      this.state.folderPath = selected as string;
      this.cdr.detectChanges();
    }
  }

  // ── Validate subscription then search ─────────────────────────
  doSearch(): void {
    if (!this.canSearch) return;

    this.handle(this.auth.validateToken(), res => {
      if (!res.success) {
        this.state.searchError = res.message || 'Session expired. Please login again.';
        this.cdr.detectChanges();
        return;
      }

      if (res.data?.user) this.userState.set(res.data.user);

      const status = (res.data?.user?.subscription_status ?? '').toLowerCase();
      if (status === 'expired' || status === 'exhausted') {
        this.plansDialog.open();
        return;
      }

      // Pass the onnx_key received from Supabase to the search command.
      // Rust loads the model on first search using this key, then discards it.
      this.runSearch(res.data?.onnx_key ?? '');
    });
  }

  private readonly BROWSER_SAFE = new Set(['jpg', 'jpeg', 'png', 'gif', 'bmp', 'webp']);

  private isBrowserSafe(path: string): boolean {
    return this.BROWSER_SAFE.has(path.split('.').pop()?.toLowerCase() ?? '');
  }

  private runSearch(onnxKey: string): void {
    this.state.searchState   = 'searching';
    this.state.searchError   = '';
    this.state.results       = [];
    this.state.failedFiles   = [];
    this.state.showScrollTop = false;
    this.state.progress      = { phase: 'Starting…', percent: 0, done: 0, total: 0, current: '', eta_sec: -1, errors: 0, active: true };
    this.cdr.detectChanges();

    this.tauri.searchStream(this.state.imagePath, this.state.folderPath, this.state.topK, onnxKey).subscribe({
      next: event => {
        if (event.type === 'progress') {
          if (event.data?.progress) this.state.progress = event.data.progress;
        } else if (event.type === 'complete') {
          this.state.results = (event.data?.results ?? []).map((r: any) => {
            const safe = this.isBrowserSafe(r.path);
            return { ...r, thumbnailUrl: safe ? convertFileSrc(r.path) : '', imgError: !safe };
          });
          this.state.failedFiles = event.data?.failed_files ?? [];
          this.state.searchState = 'results';
        } else if (event.type === 'error') {
          this.state.searchError = event.data?.message ?? 'Search failed. Please try again.';
          this.state.searchState = 'idle';
        }
        this.cdr.detectChanges();
      }
    });
  }

  newSearch(): void { this.state.reset(); this.cdr.detectChanges(); }

  onImgError(item: SearchResult): void { item.imgError = true; this.cdr.detectChanges(); }

  openFile(path: string): void { this.tauri.openFilePath(path); }

  onGridScroll(event: Event): void {
    this.state.showScrollTop = (event.target as HTMLElement).scrollTop > 300;
    this.cdr.detectChanges();
  }

  scrollToTop(): void {
    this.masonryGridRef?.nativeElement.scrollTo({ top: 0, behavior: 'smooth' });
    this.state.showScrollTop = false;
  }

  similarityClass(sim: number): string {
    if (sim >= 88) return 'high';
    if (sim >= 72) return 'mid';
    return 'low';
  }

  similarityGradient(sim: number): string {
    if (sim >= 88) return 'linear-gradient(135deg,#d946ef 0%,#fb923c 100%)';
    if (sim >= 72) return 'linear-gradient(135deg,#a21caf 0%,#f97316 100%)';
    return 'linear-gradient(135deg,#701a75 0%,#c2410c 100%)';
  }
}
