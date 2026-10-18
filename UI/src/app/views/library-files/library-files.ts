import { Component, ElementRef, OnDestroy, OnInit, ViewChild, inject } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { ActivatedRoute, Router } from '@angular/router';
import { Subscription } from 'rxjs';
import { TranslateModule, TranslateService } from '@ngx-translate/core';
import { MessageService } from 'primeng/api';
import { SelectButtonModule } from 'primeng/selectbutton';
import { TooltipModule } from 'primeng/tooltip';
import { BaseComponent } from '../../core/base.component';
import { ZoneWrapperService } from '../../core/zone-wrapper/zone-wrapper.service';
import { BrowseService } from '../../services/browse/browse.service';
import { FileService } from '../../services/file/file.service';
import { LibraryService } from '../../services/library/library.service';
import {
  ClientFileRow, ClientFileStatus, LibraryDirEntry, responseLibraryCrumb, responseLibraryDirCounts,
  WatchedFolderStatus,
} from '../../models/response/responseLibrary';
import { StatusFilter, filterCounts, filterImages, statusIcon } from './library-files.util';

type ViewMode = 'dir' | 'notLoaded';

// One filter chip: its value, label key and live count.
interface FilterOption {
  value: StatusFilter;
  label: string;
  count: number;
}

// Images rendered per page; more are added as the sentinel scrolls into view.
const PAGE_SIZE = 120;
// Minimum gap between refreshes driven by sync progress events.
const PROGRESS_RELOAD_MS = 3000;

@Component({
  selector: 'app-library-files',
  imports: [CommonModule, FormsModule, TranslateModule, SelectButtonModule, TooltipModule],
  templateUrl: './library-files.html',
  styleUrl: './library-files.scss',
})
export class LibraryFiles extends BaseComponent implements OnInit, OnDestroy {
  private route       = inject(ActivatedRoute);
  private router      = inject(Router);
  private libSvc      = inject(LibraryService);
  private browseSvc   = inject(BrowseService);
  private fileSvc     = inject(FileService);
  private zoneWrapper = inject(ZoneWrapperService);
  private messages    = inject(MessageService);
  private t           = inject(TranslateService);

  // Page state, mirrored from the URL query params so Back works.
  root = '';
  rel  = '';
  mode: ViewMode = 'dir';

  loading = true;
  folderStatus: WatchedFolderStatus = 'watching';

  // Directory view data.
  breadcrumb: responseLibraryCrumb[] = [];
  folders: LibraryDirEntry[] = [];
  images: ClientFileRow[] = [];
  counts: responseLibraryDirCounts = { loaded: 0, failed: 0, pending: 0, total: 0 };
  filter: StatusFilter = 'all';
  filterOptions: FilterOption[] = [];

  // Not-loaded list view data.
  notLoaded: ClientFileRow[] = [];

  // Rows currently rendered (a prefix of the filtered list) and how many are allowed.
  shown: ClientFileRow[] = [];
  private visibleCount = PAGE_SIZE;
  private total = 0;

  // Thumbnails (path -> asset src), requested only for rendered rows.
  thumbs   = new Map<string, string>();
  thumbErr = new Set<string>();
  private thumbRequested = new Set<string>();
  private readonly BROWSER_SAFE = new Set(['jpg', 'jpeg', 'png', 'gif', 'bmp', 'webp']);

  private subs = new Subscription();
  private unlistenSync: (() => void) | null = null;
  private observer: IntersectionObserver | null = null;
  // Guards against a slow earlier response overwriting a newer view.
  private loadSeq = 0;
  private lastProgressReload = 0;

  // The "load more" sentinel exists only while rows remain; (re)observe it whenever it appears.
  @ViewChild('sentinel') set sentinelRef(el: ElementRef<HTMLElement> | undefined) {
    this.observer?.disconnect();
    if (el && this.observer) this.observer.observe(el.nativeElement);
  }

  ngOnInit(): void {
    // Not available under the jsdom test runner, where paging simply never advances.
    if (typeof IntersectionObserver !== 'undefined') {
      this.observer = new IntersectionObserver(entries => {
        if (entries.some(e => e.isIntersecting)) this.showMore();
      });
    }

    this.subs.add(this.route.queryParamMap.subscribe(q => {
      this.root = q.get('root') ?? '';
      this.rel  = q.get('rel') ?? '';
      this.mode = q.get('mode') === 'notLoaded' ? 'notLoaded' : 'dir';
      if (!this.root) {
        this.router.navigate(['/master/library']);
        return;
      }
      this.filter = 'all';
      this.visibleCount = PAGE_SIZE;
      this.load(false);
    }));

    this.unlistenSync = this.libSvc.onLibrarySync({
      progress: ({ path }) => {
        if (path !== this.root || Date.now() - this.lastProgressReload < PROGRESS_RELOAD_MS) return;
        this.lastProgressReload = Date.now();
        this.load(true);
      },
      complete: ({ path }) => { if (path === this.root) this.load(true); },
    });
  }

  ngOnDestroy(): void {
    this.subs.unsubscribe();
    this.unlistenSync?.();
    this.observer?.disconnect();
  }

  // ── Data ────────────────────────────────────────────────────────

  // Fetch the current view; `silent` skips the global loader for event-driven refreshes.
  private load(silent: boolean): void {
    const seq = ++this.loadSeq;
    if (!silent) this.loading = true;

    if (this.mode === 'notLoaded') {
      const obs = silent ? this.libSvc.listNotLoadedSilent(this.root) : this.libSvc.listNotLoaded(this.root);
      this.handle(obs, data => {
        if (seq !== this.loadSeq) return;
        this.loading = false;
        this.notLoaded = data.images;
        this.refreshShown();
      }, err => this.fail(seq, err.message));
      return;
    }

    const obs = silent ? this.libSvc.listDirSilent(this.root, this.rel) : this.libSvc.listDir(this.root, this.rel);
    this.handle(obs, data => {
      if (seq !== this.loadSeq) return;
      this.loading = false;
      this.rel          = data.rel;
      this.breadcrumb   = data.breadcrumb;
      this.folders      = data.folders;
      this.images       = data.images;
      this.counts       = data.counts;
      this.folderStatus = data.folderStatus;
      this.buildFilterOptions();
      this.refreshShown();
    }, err => this.fail(seq, err.message));
  }

  private fail(seq: number, message: string): void {
    if (seq !== this.loadSeq) return;
    this.loading = false;
    this.toastErr(message || this.t.instant('libraryFiles.couldNotLoad'));
  }

  // Chips with their counts; rebuilt on each load so the numbers stay live.
  private buildFilterOptions(): void {
    const c = filterCounts(this.counts);
    this.filterOptions = (['all', 'loaded', 'failed', 'pending'] as StatusFilter[]).map(value => ({
      value,
      label: this.t.instant('libraryFiles.filter.' + value),
      count: c[value],
    }));
  }

  // Recompute the rendered prefix of the (filtered) list and request its thumbnails.
  private refreshShown(): void {
    const all = this.mode === 'notLoaded' ? this.notLoaded : filterImages(this.images, this.filter);
    this.total = all.length;
    this.shown = all.slice(0, this.visibleCount);
    this.loadThumbs(this.shown);
  }

  get hasMore(): boolean { return this.shown.length < this.total; }

  private showMore(): void {
    if (!this.hasMore) return;
    this.visibleCount += PAGE_SIZE;
    this.refreshShown();
    this.cdr.detectChanges();
  }

  onFilterChange(): void {
    this.visibleCount = PAGE_SIZE;
    this.refreshShown();
  }

  // ── Navigation ──────────────────────────────────────────────────

  openDir(rel: string): void {
    this.router.navigate([], { relativeTo: this.route, queryParams: { root: this.root, rel, mode: 'dir' } });
  }

  showNotLoaded(): void {
    this.router.navigate([], { relativeTo: this.route, queryParams: { root: this.root, rel: '', mode: 'notLoaded' } });
  }

  backToLibrary(): void {
    this.router.navigate(['/master/library']);
  }

  get rootName(): string {
    return this.basename(this.root);
  }

  // ── Badges ──────────────────────────────────────────────────────

  icon(status: ClientFileStatus): string { return statusIcon(status); }

  errorLabel(code?: string): string {
    if (!code) return '';
    const key = 'libraryFiles.error.' + code;
    const label = this.t.instant(key);
    return label === key ? code : label;
  }

  // Tooltip: the status, plus the translated error code and raw detail for a failed image.
  tip(img: ClientFileRow): string {
    if (img.status !== 'failed') return this.t.instant('libraryFiles.status.' + img.status);
    return img.errorReason ? `${this.errorLabel(img.errorCode)} — ${img.errorReason}` : this.errorLabel(img.errorCode);
  }

  // ── Thumbnails ──────────────────────────────────────────────────

  private ext(path: string): string { return path.split('.').pop()?.toLowerCase() ?? ''; }

  private loadThumbs(items: ClientFileRow[]): void {
    for (const it of items) {
      if (this.thumbRequested.has(it.path)) continue;
      this.thumbRequested.add(it.path);
      if (this.BROWSER_SAFE.has(this.ext(it.path))) {
        this.thumbs.set(it.path, this.zoneWrapper.toAssetUrl(it.path));
      } else {
        this.browseSvc.thumbnail(it.path)
          .then(tp => { this.thumbs.set(it.path, this.zoneWrapper.toAssetUrl(tp)); this.cdr.detectChanges(); })
          .catch(() => { this.thumbErr.add(it.path); this.cdr.detectChanges(); });
      }
    }
  }

  thumbOf(path: string): string | undefined { return this.thumbs.get(path); }
  hasThumbError(path: string): boolean { return this.thumbErr.has(path); }
  onThumbError(path: string): void { this.thumbErr.add(path); this.cdr.detectChanges(); }

  openFile(p: string): void { this.fileSvc.openFilePath(p); }

  basename(p: string): string { return p.split(/[\\/]/).filter(Boolean).pop() ?? p; }

  private toastErr(detail: string): void {
    this.messages.add({ key: 'app', severity: 'error', summary: this.t.instant('libraryFiles.toastTitle'), detail, life: 4000 });
  }
}
