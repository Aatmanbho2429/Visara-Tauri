import { Component, ElementRef, OnInit, ViewChild, inject } from '@angular/core';
import { CommonModule } from '@angular/common';
import { TranslateModule } from '@ngx-translate/core';
import { Router } from '@angular/router';
import { open } from '@tauri-apps/plugin-dialog';
import { convertFileSrc } from '@tauri-apps/api/core';
import { PrimengComponentsModule } from '../../shared/primeng-components-module';
import { BaseComponent } from '../../core/base.component';
import { TauriService } from '../../services/tauri.service';
import { AuthService } from '../../services/auth.service';
import { LibraryService } from '../../services/library.service';
import { BrowseService } from '../../services/browse.service';
import { UserStateService } from '../../services/user-state.service';
import { SearchStateService } from '../../services/search-state.service';
import { PlansDialog } from '../../shared/plans-dialog/plans-dialog';
import { WatchedFolder } from '../../models/library.model';

export interface SearchResult {
  rank:          number;
  path:          string;
  name:          string;
  similarity:    number;
  pattern_match: number;
  color_match:   number;
  folder:        string;
  thumbnailUrl:  string;
  imgError:      boolean;
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
export class Search extends BaseComponent implements OnInit {

  private tauri     = inject(TauriService);
  private auth      = inject(AuthService);
  private libSvc    = inject(LibraryService);
  private browseSvc = inject(BrowseService);
  private userState = inject(UserStateService);
  private router    = inject(Router);
  state             = inject(SearchStateService);

  @ViewChild('masonryGrid') masonryGridRef!: ElementRef<HTMLElement>;
  @ViewChild(PlansDialog)   plansDialog!: PlansDialog;

  readonly topKOptions = [10, 20, 50];

  /** Platform-aware label for the global hot-key shown in the empty-state hint. */
  readonly hotkeyLabel = navigator.platform.toLowerCase().includes('mac')
    ? '⌘ + Shift + V'
    : 'Ctrl + Shift + V';

  /** All watched folders pulled from backend.  Drives the empty state + scope picker. */
  watchedFolders: WatchedFolder[] = [];
  foldersLoading                  = true;
  scopeOpen                       = false;

  constructor() { super(); }

  ngOnInit(): void {
    this.refreshFolders();
  }

  refreshFolders(): void {
    this.foldersLoading = true;
    this.handle(this.libSvc.list(), res => {
      this.foldersLoading = false;
      this.watchedFolders = (res.success && res.data?.folders) ? res.data.folders : [];

      // Drop any cached scope entries that no longer exist (folder removed
      // from the Library while user was on this page).
      const valid = new Set(this.watchedFolders.map(f => f.path));
      this.state.scopePaths = this.state.scopePaths.filter(p => valid.has(p));
    });
  }

  get hasWatchedFolders(): boolean { return this.watchedFolders.length > 0; }
  get canSearch():       boolean   { return !!this.state.imagePath && this.hasWatchedFolders; }
  get isIdle():          boolean   { return this.state.searchState === 'idle'; }
  get isSearching():     boolean   { return this.state.searchState === 'searching'; }
  get hasResults():      boolean   { return this.state.searchState === 'results'; }

  get scopeLabel(): string {
    const n = this.state.scopePaths.length;
    if (n === 0) return `All ${this.watchedFolders.length} folders`;
    if (n === 1) return this.shortName(this.state.scopePaths[0]);
    return `${n} folders`;
  }

  get masonryColumns(): SearchResult[][] {
    const cols    = 3;
    const columns = Array.from({ length: cols }, (): SearchResult[] => []);
    this.state.results.forEach((item, i) => columns[i % cols].push(item));
    return columns;
  }

  shortName(path: string): string {
    return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
  }

  // ── Image picker ──────────────────────────────────────────────────

  async pickImage() {
    const selected = await open({
      multiple: false,
      filters:  [{ name: 'Images', extensions: ['jpg','jpeg','png','tif','tiff','psd','psb'] }]
    });
    if (selected) {
      this.state.imagePath    = selected as string;
      this.state.imageName    = (selected as string).split(/[\\/]/).pop() ?? selected as string;
      this.state.imagePreview = '';
      this.cdr.detectChanges();
    }
  }

  // ── Scope picker ──────────────────────────────────────────────────

  toggleScopeDropdown(): void {
    this.scopeOpen = !this.scopeOpen;
  }

  isFolderInScope(path: string): boolean {
    // Empty scope = all folders selected.
    return this.state.scopePaths.length === 0 || this.state.scopePaths.includes(path);
  }

  toggleFolderInScope(path: string): void {
    const isSelected = this.isFolderInScope(path);

    // If scope was empty (all-on) and user is unchecking one, materialize
    // the full list minus the clicked one.
    if (this.state.scopePaths.length === 0 && isSelected) {
      this.state.scopePaths = this.watchedFolders.map(f => f.path).filter(p => p !== path);
      this.cdr.detectChanges();
      return;
    }

    if (isSelected) {
      // Never let the user uncheck the LAST remaining folder — the empty
      // array would otherwise mean "all", flipping the UI's intent.
      if (this.state.scopePaths.length <= 1) return;
      this.state.scopePaths = this.state.scopePaths.filter(p => p !== path);
    } else {
      this.state.scopePaths = [...this.state.scopePaths, path];
    }

    // If the user re-selected every folder, collapse to empty (means "all").
    if (this.state.scopePaths.length === this.watchedFolders.length) {
      this.state.scopePaths = [];
    }
    this.cdr.detectChanges();
  }

  selectAllFolders(): void {
    this.state.scopePaths = [];
    this.cdr.detectChanges();
  }

  goToLibrary(): void {
    this.router.navigate(['/master/library']);
  }

  // ── Search execution ─────────────────────────────────────────────

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

    this.tauri.searchStream(this.state.imagePath, this.state.scopePaths, this.state.topK, onnxKey).subscribe({
      next: event => {
        if (event.type === 'progress') {
          if (event.data?.progress) this.state.progress = event.data.progress;
        } else if (event.type === 'complete') {
          this.state.results = (event.data?.results ?? []).map((r: any) => {
            const safe = this.isBrowserSafe(r.path);
            // Browser-safe formats load straight from disk; everything else
            // (PSB, PSD, TIFF…) has no thumbnail yet — leave it in the loading
            // state and generate one below, mirroring the Browse page.
            return { ...r, thumbnailUrl: safe ? convertFileSrc(r.path) : '', imgError: false };
          });
          // Kick off thumbnail generation for the non-browser-safe results so
          // they render as real previews instead of a gradient fallback.
          for (const item of this.state.results) {
            if (!item.thumbnailUrl) this.loadThumb(item);
          }
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

  /** Generate (or fetch the cached) thumbnail for a non-browser-safe format,
   *  then point the tile at it.  Falls back to the gradient on failure. */
  private loadThumb(item: SearchResult): void {
    this.browseSvc.thumbnail(item.path)
      .then(tp => { item.thumbnailUrl = convertFileSrc(tp); this.cdr.detectChanges(); })
      .catch(() => { item.imgError = true; this.cdr.detectChanges(); });
  }

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

  colorMatchClass(sim: number): string {
    if (sim >= 70) return 'high';
    if (sim >= 45) return 'mid';
    return 'low';
  }

  similarityGradient(sim: number): string {
    if (sim >= 88) return 'linear-gradient(135deg,#d946ef 0%,#fb923c 100%)';
    if (sim >= 72) return 'linear-gradient(135deg,#a21caf 0%,#f97316 100%)';
    return 'linear-gradient(135deg,#701a75 0%,#c2410c 100%)';
  }
}
