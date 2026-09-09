import { Component, ElementRef, HostListener, OnDestroy, OnInit, ViewChild, inject } from '@angular/core';
import { CommonModule } from '@angular/common';
import { TranslateModule } from '@ngx-translate/core';
import { Router } from '@angular/router';
import { open } from '@tauri-apps/plugin-dialog';
import { BaseComponent } from '../../core/base.component';
import { ZoneWrapperService } from '../../core/zone-wrapper/zone-wrapper.service';
import { SearchService } from '../../services/search/search.service';
import { AuthService } from '../../services/auth/auth.service';
import { LibraryService } from '../../services/library/library.service';
import { BrowseService } from '../../services/browse/browse.service';
import { FileService } from '../../services/file/file.service';
import { UserStateService } from '../../services/user/user-state.service';
import { SearchStateService, SearchResultVM } from '../../services/search/search-state.service';
import { PlansDialog } from '../../shared/plans-dialog/plans-dialog';
import { LoaderOrb } from '../../shared/loader-orb/loader-orb';
import { WatchedFolder } from '../../models/response/responseLibrary';
import { responseSidecarCrashed } from '../../models/response/responseMisc';

@Component({
  selector:    'app-search',
  imports:     [CommonModule, TranslateModule, PlansDialog, LoaderOrb],
  templateUrl: './search.html',
  styleUrl:    './search.scss',
})
export class Search extends BaseComponent implements OnInit, OnDestroy {

  private searchSvc  = inject(SearchService);
  private auth       = inject(AuthService);
  private libSvc     = inject(LibraryService);
  private browseSvc  = inject(BrowseService);
  private fileSvc    = inject(FileService);
  private zoneWrapper = inject(ZoneWrapperService);
  private userState  = inject(UserStateService);
  private router     = inject(Router);
  state              = inject(SearchStateService);

  @ViewChild('masonryGrid') masonryGridRef!: ElementRef<HTMLElement>;
  @ViewChild(PlansDialog)   plansDialog!: PlansDialog;

  // How many "Similar patterns" cards are revealed at a time.
  //
  // The backend no longer truncates: `search.rs` returns every member of the
  // near family (everything clearing the embedding-cosine floor), which can be
  // hundreds of cards. Painting them all at once is what this paging avoids —
  // it is a rendering budget, not a result limit, so nothing is discarded and
  // "Show more" can always reach the end of the set.
  readonly SIMILAR_PAGE = 20;

  // Upper bound on how many similar cards are currently rendered. Reset to one
  // page on every new search — otherwise a previous search's expanded state
  // would carry over and dump hundreds of cards into the first paint.
  similarShown = this.SIMILAR_PAGE;

  // Collapse state for the two result tiers. Both start expanded and are
  // re-opened for every fresh result set in `runSearch()` — collapsing is
  // something the user asks for, not a default the page imposes.
  similarExpanded = true;
  familyExpanded  = true;

  // Platform-aware label for the global hot-key shown in the empty-state hint.
  readonly hotkeyLabel = navigator.platform.toLowerCase().includes('mac')
    ? '⌘ + Shift + V'
    : 'Ctrl + Shift + V';

  // All watched folders pulled from backend.  Drives the empty state + scope picker.
  watchedFolders: WatchedFolder[] = [];
  foldersLoading                  = true;
  scopeOpen                       = false;

  // Drives the "Model is loading…" state. Two models sit behind it and they
  // arrive at different times: the bundled mobilenet (Gabor/Gram/SIFT) loads
  // at sidecar launch, while the licensed DINO model that produces the search
  // embeddings only loads once auth has handed the sidecar its key — so this
  // stays false until the health check passes, the embed model is up, AND the
  // session is confirmed active. Still no per-search key handoff.
  sidecarReady           = false;
  private sidecarPollId: ReturnType<typeof setInterval> | null = null;

  // True while the backend's watchdog (`core::sidecar::watchdog`) is
  // relaunching a sidecar that crashed mid-session — distinct from the
  // plain first-boot "Model is loading…" state so the empty-state message
  // can say what actually happened. `sidecarErrorMessage` is set instead
  // when the watchdog gives up after repeated crashes (see
  // `MAX_CONSECUTIVE_CRASHES` in `sidecar.rs`) — a state polling alone
  // can't recover from, since there's nothing left relaunching it.
  sidecarReconnecting  = false;
  sidecarErrorMessage  = '';
  private unlistenSidecarCrashed: (() => void) | null = null;

  constructor() { super(); }

  ngOnInit(): void {
    this.refreshFolders();
    this.pollSidecarStatus();
    this.unlistenSidecarCrashed = this.searchSvc.onSidecarCrashed(e => this.onSidecarCrashed(e));
  }

  ngOnDestroy(): void {
    if (this.sidecarPollId !== null) clearInterval(this.sidecarPollId);
    this.unlistenSidecarCrashed?.();
  }

  private onSidecarCrashed(e: responseSidecarCrashed): void {
    this.sidecarReady = false;
    this.sidecarReconnecting = e.recovering;
    this.sidecarErrorMessage = e.recovering ? '' : e.message;
    this.cdr.detectChanges();
    // The poll loop stops itself once it first sees `ready: true` (see
    // below) — a crash after that point needs it re-armed to notice the
    // sidecar coming back.
    if (e.recovering) this.pollSidecarStatus();
  }

  private pollSidecarStatus(): void {
    if (this.sidecarPollId !== null) return; // already polling
    const check = () => {
      this.searchSvc.sidecarStatus().subscribe({
        next: res => {
          this.sidecarReady = !!res?.ready;
          if (this.sidecarReady) this.sidecarReconnecting = false;
          this.cdr.detectChanges();
          if (this.sidecarReady && this.sidecarPollId !== null) {
            clearInterval(this.sidecarPollId);
            this.sidecarPollId = null;
          }
        },
        error: () => {
          this.sidecarReady = false;
          this.cdr.detectChanges();
        },
      });
    };
    check();
    this.sidecarPollId = setInterval(check, 700);
  }

  refreshFolders(): void {
    this.foldersLoading = true;
    this.handle(
      this.libSvc.list(),
      data => {
        this.foldersLoading = false;
        this.watchedFolders = data?.folders ?? [];

        // Drop any cached scope entries that no longer exist (folder removed
        // from the Library while the user was on this page). If that empties
        // the list, scope falls back to its default meaning — all folders.
        const valid = new Set(this.watchedFolders.map(f => f.path));
        this.state.scopePaths = this.state.scopePaths.filter(p => valid.has(p));
      },
      () => { this.foldersLoading = false; this.watchedFolders = []; },
    );
  }

  get hasWatchedFolders(): boolean { return this.watchedFolders.length > 0; }
  get canSearch():       boolean   {
    return !!this.state.imagePath && this.hasWatchedFolders && this.sidecarReady;
  }
  get isIdle():          boolean   { return this.state.searchState === 'idle'; }
  get isSearching():     boolean   { return this.state.searchState === 'searching'; }
  get hasResults():      boolean   { return this.state.searchState === 'results'; }

  // An empty `scopePaths` means "every watched folder" — both here and in
  // the backend (`search.rs` resolves an empty scope to the full Library),
  // so the default state searches everything.
  get scopeLabel(): string {
    const n = this.state.scopePaths.length;
    if (n === 0) return `All ${this.watchedFolders.length} folders`;
    if (n === 1) return this.shortName(this.state.scopePaths[0]);
    return `${n} folders`;
  }

  // SIFT/RANSAC-proven results — same texture family as the query, not
  // just similar-looking. Always shown in full; see `search.rs`'s
  // `family_count` split, which the backend never truncates.
  get familyResults(): SearchResultVM[] {
    return this.state.results.filter(r => r.verified);
  }

  // Everything else: in the near family by texture similarity, but not
  // geometrically confirmed. The *full* set — used for the section count and
  // for deciding whether "Show more" still has anything left to give.
  get similarResults(): SearchResultVM[] {
    return this.state.results.filter(r => !r.verified);
  }

  // The slice actually rendered — see `SIMILAR_PAGE`.
  get visibleSimilarResults(): SearchResultVM[] {
    return this.similarResults.slice(0, this.similarShown);
  }

  get hasMoreSimilar(): boolean {
    return this.similarShown < this.similarResults.length;
  }

  // How many are still hidden, so the button can say so rather than leave the
  // user guessing whether one more click ends it or fifty.
  get remainingSimilar(): number {
    return Math.max(0, this.similarResults.length - this.similarShown);
  }

  showMoreSimilar(): void {
    this.similarShown += this.SIMILAR_PAGE;
    this.cdr.detectChanges();
  }

  get familyColumns():  SearchResultVM[][] { return this.toColumns(this.familyResults); }
  get similarColumns(): SearchResultVM[][] { return this.toColumns(this.visibleSimilarResults); }

  private toColumns(items: SearchResultVM[]): SearchResultVM[][] {
    const cols    = 3;
    const columns = Array.from({ length: cols }, (): SearchResultVM[] => []);
    items.forEach((item, i) => columns[i % cols].push(item));
    return columns;
  }

  shortName(path: string): string {
    return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
  }

  // ── Image picker ──────────────────────────────────────────────────

  async pickImage() {
    const selected = await open({
      multiple: false,
      filters:  [{ name: 'Images', extensions: ['jpg','jpeg','png'] }]
    });
    if (selected) {
      this.state.imagePath    = selected as string;
      this.state.imageName    = (selected as string).split(/[\\/]/).pop() ?? selected as string;
      this.state.imagePreview = '';
      this.cdr.detectChanges();
    }
  }

  // ── Scope picker ──────────────────────────────────────────────────
  // A custom multi-select dropdown, not a native <select> — WebView2's
  // native <select> popup can't be styled (no rounded corners, no hover
  // state, wrong colors) and rendered badly against the rest of the page.
  // Every folder starts selected; an empty `scopePaths` is the encoding of
  // "all of them", which is also what the backend expects.

  toggleScopeDropdown(): void {
    this.scopeOpen = !this.scopeOpen;
  }

  // Bound to `(document:click)` so clicking anywhere outside the picker
  // closes it. `$event.stopPropagation()` on `.scope-picker` in the
  // template keeps clicks *inside* it from reaching this handler.
  @HostListener('document:click')
  closeScopeDropdown(): void {
    this.scopeOpen = false;
  }

  isFolderInScope(path: string): boolean {
    // Empty scope = all folders selected.
    return this.state.scopePaths.length === 0 || this.state.scopePaths.includes(path);
  }

  toggleFolderInScope(path: string): void {
    const isSelected = this.isFolderInScope(path);

    // If scope was empty (all-on) and the user is unchecking one, materialize
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

    this.handle(
      this.auth.validateToken(),
      data => {
        if (data?.user) this.userState.set(data.user);

        const status = (data?.user?.subscriptionStatus ?? '').toLowerCase();
        if (status === 'expired' || status === 'exhausted') {
          this.plansDialog.open();
          return;
        }

        this.runSearch();
      },
      err => {
        this.state.searchError = err.message || 'Session expired. Please login again.';
        this.cdr.detectChanges();
      },
    );
  }

  private readonly BROWSER_SAFE = new Set(['jpg', 'jpeg', 'png', 'gif', 'bmp', 'webp']);

  private isBrowserSafe(path: string): boolean {
    return this.BROWSER_SAFE.has(path.split('.').pop()?.toLowerCase() ?? '');
  }

  // True if any folder this search will touch is mid-index right now — the
  // only case where the sidecar's single-lane queue makes the query wait on
  // something already in flight (see search-loader status line below).
  private scopeIsIndexing(folders: WatchedFolder[]): boolean {
    const scope = this.state.scopePaths;
    return folders.some(f => f.status === 'indexing' && (scope.length === 0 || scope.includes(f.path)));
  }

  private runSearch(): void {
    this.state.searchState   = 'searching';
    this.state.searchError   = '';
    this.state.results       = [];
    this.state.failedFiles   = [];
    this.state.showScrollTop = false;
    this.state.progress      = { phase: 'Starting…', percent: 0, done: 0, total: 0, current: '', etaSec: -1, errors: 0, active: true };
    this.cdr.detectChanges();

    // Peek at current folder status (silent — no global loader flicker) so the
    // loader can say something honest if this search is about to sit behind
    // an in-flight indexing file, instead of just spinning unexplained.
    this.libSvc.listSilent().subscribe({
      next: data => {
        const folders = data?.folders ?? this.watchedFolders;
        if (this.state.searchState === 'searching' && this.scopeIsIndexing(folders)) {
          this.state.progress = { ...this.state.progress, phase: 'One moment — wrapping up indexing before your search…' };
          this.cdr.detectChanges();
        }
      },
      error: () => {},
    });

    // `topK` no longer trims anything server-side (see `services::search`);
    // it is logged there as "what the client asked for". Sending the page size
    // keeps that log honest now that the picker is gone.
    this.searchSvc.searchStream(this.state.imagePath, this.state.scopePaths, this.SIMILAR_PAGE).subscribe({
      next: event => {
        if (event.type === 'progress') {
          if (event.data?.progress) this.state.progress = event.data.progress;
        } else if (event.type === 'complete') {
          this.state.results = (event.data?.results ?? []).map((r: any) => {
            const safe = this.isBrowserSafe(r.path);
            // Browser-safe formats load straight from disk; everything else
            // (PSB, PSD, TIFF…) has no thumbnail yet — leave it in the loading
            // state and generate one below, mirroring the Browse page.
            return { ...r, thumbnailUrl: safe ? this.zoneWrapper.toAssetUrl(r.path) : '', imgError: false, matchBox: null };
          });
          // Kick off thumbnail generation for the non-browser-safe results so
          // they render as real previews instead of a gradient fallback.
          for (const item of this.state.results) {
            if (!item.thumbnailUrl) this.loadThumb(item);
          }
          this.state.failedFiles = event.data?.failedFiles ?? [];
          this.state.searchState = 'results';
          // Both tiers open on every fresh result set — "Similar" used to
          // auto-collapse whenever a verified tier existed, which hid 20 of
          // 22 results behind a control that was easy to miss.
          this.similarExpanded = true;
          this.similarShown    = this.SIMILAR_PAGE;
          this.familyExpanded  = true;
        } else if (event.type === 'error') {
          this.state.searchError = event.data?.message ?? 'Search failed. Please try again.';
          this.state.searchState = 'idle';
        }
        this.cdr.detectChanges();
      }
    });
  }

  newSearch(): void {
    // `reset()` clears `scopePaths`, which puts the scope back to its default
    // meaning — all watched folders — rather than inheriting the narrower
    // selection from the previous search.
    this.state.reset();
    this.cdr.detectChanges();
  }

  // Generate (or fetch the cached) thumbnail for a non-browser-safe format,
  // then point the tile at it.  Falls back to the gradient on failure.
  private loadThumb(item: SearchResultVM): void {
    this.browseSvc.thumbnail(item.path)
      .then(tp => { item.thumbnailUrl = this.zoneWrapper.toAssetUrl(tp); this.cdr.detectChanges(); })
      .catch(() => { item.imgError = true; this.cdr.detectChanges(); });
  }

  onImgError(item: SearchResultVM): void { item.imgError = true; this.cdr.detectChanges(); }

  // ── "Found inside" highlight ─────────────────────────────────────
  //
  // The backend reports the matched region in normalised coordinates of the
  // *source* image.  The thumbnail is rendered with object-fit: cover inside a
  // fixed-height card, so part of the image is cropped away and the mapping
  // from source coordinates to card coordinates depends on both aspect ratios.
  // We therefore measure the rendered image and convert to pixels.

  onImgLoad(item: SearchResultVM, ev: Event): void {
    item.matchBox = this.computeMatchBox(item, ev.target as HTMLImageElement);
    this.cdr.detectChanges();
  }

  // The card width changes with the window, so the boxes need re-deriving.
  @HostListener('window:resize')
  onWindowResize(): void {
    const root = this.masonryGridRef?.nativeElement;
    if (!root) return;
    root.querySelectorAll<HTMLImageElement>('img.pin-card__img[data-rank]').forEach(img => {
      const rank = Number(img.dataset['rank']);
      const item = this.state.results.find((r: SearchResultVM) => r.rank === rank);
      if (item) item.matchBox = this.computeMatchBox(item, img);
    });
    this.cdr.detectChanges();
  }

  private computeMatchBox(item: SearchResultVM, img: HTMLImageElement): Record<string, string> | null {
    if (!item.partial || !item.matchRegion) return null;

    const cw = img.clientWidth,  ch = img.clientHeight;
    const iw = img.naturalWidth, ih = img.naturalHeight;
    if (!cw || !ch || !iw || !ih) return null;

    // object-fit: cover scales by the *larger* ratio and centres the overflow,
    // so ox/oy go negative on the cropped axis — part of the image, and so
    // possibly part of the box, sits outside the card and is clipped.
    const scale = Math.max(cw / iw, ch / ih);
    const dw = iw * scale, dh = ih * scale;
    const ox = (cw - dw) / 2, oy = (ch - dh) / 2;

    const r = item.matchRegion;
    return {
      left:   `${ox + r.x * dw}px`,
      top:    `${oy + r.y * dh}px`,
      width:  `${r.w * dw}px`,
      height: `${r.h * dh}px`,
    };
  }

  openFile(path: string): void { this.fileSvc.openFilePath(path); }

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
