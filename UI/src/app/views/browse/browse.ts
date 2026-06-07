import { Component, OnDestroy, OnInit, inject } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { convertFileSrc } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { MessageService } from 'primeng/api';
import { BaseComponent } from '../../core/base.component';
import { BrowseService } from '../../services/browse.service';
import { TagsService } from '../../services/tags.service';
import { TauriService } from '../../services/tauri.service';
import {
  BrowseEntry, FileTag, TagFacet, TagSuggestion, TAG_PRESETS, CATEGORY_LABEL,
} from '../../models/browse.model';

@Component({
  selector: 'app-browse',
  imports: [CommonModule, FormsModule],
  templateUrl: './browse.html',
  styleUrl: './browse.scss',
})
export class Browse extends BaseComponent implements OnInit, OnDestroy {
  private browseSvc  = inject(BrowseService);
  private tagsSvc    = inject(TagsService);
  private tauri      = inject(TauriService);
  private messages   = inject(MessageService);

  loading = true;

  // Folder navigation
  current = '';
  breadcrumb: BrowseEntry[] = [];
  folders:   BrowseEntry[] = [];
  images:    BrowseEntry[] = [];

  // Tag filtering
  facetGroups:   { category: string; label: string; facets: TagFacet[] }[] = [];
  activeFilters: TagSuggestion[] = [];
  filterImages:  BrowseEntry[] = [];

  // Selection
  selected = new Set<string>();

  // Thumbnails (path -> asset src)
  thumbs   = new Map<string, string>();
  thumbErr = new Set<string>();

  // Categorize panel
  showPanel    = false;
  selTags:     FileTag[] = [];
  suggestions: TagSuggestion[] = [];
  customInput  = '';

  readonly panelCategories = ['color', 'material', 'finish', 'size', 'design'];
  readonly presets  = TAG_PRESETS;
  readonly catLabel = CATEGORY_LABEL;

  private readonly BROWSER_SAFE = new Set(['jpg', 'jpeg', 'png', 'gif', 'bmp', 'webp']);
  private unlistenTags: UnlistenFn | null = null;

  ngOnInit(): void {
    this.loadFacets();
    this.openPath('');
    listen('tags_updated', () => this.loadFacets()).then(u => (this.unlistenTags = u));
  }

  ngOnDestroy(): void {
    this.unlistenTags?.();
  }

  get filtering(): boolean { return this.activeFilters.length > 0; }
  get visibleImages(): BrowseEntry[] { return this.filtering ? this.filterImages : this.images; }

  // ── Navigation ──────────────────────────────────────────────────
  openPath(path: string): void {
    this.loading = true;
    this.handle(this.browseSvc.list(path), res => {
      this.loading = false;
      if (res.success && res.data) {
        this.current    = res.data.current;
        this.breadcrumb = res.data.breadcrumb;
        this.folders    = res.data.folders;
        this.images     = res.data.images;
        this.loadThumbs(this.images);
      } else {
        this.toastErr(res.message || 'Could not open folder.');
      }
    });
  }

  // ── Facets / filtering ──────────────────────────────────────────
  loadFacets(): void {
    this.handle(this.tagsSvc.facets(), res => {
      const facets = (res.success && res.data?.facets) ? res.data.facets : [];
      const order = ['color', 'material', 'finish', 'size', 'design', 'collection', 'custom'];
      this.facetGroups = order
        .map(cat => ({ category: cat, label: this.catLabel[cat] ?? cat, facets: facets.filter(f => f.category === cat) }))
        .filter(g => g.facets.length > 0);
    });
  }

  isFilterActive(f: TagFacet): boolean {
    return this.activeFilters.some(a => a.category === f.category && a.value === f.value);
  }

  toggleFilter(f: TagFacet): void {
    this.activeFilters = this.isFilterActive(f)
      ? this.activeFilters.filter(a => !(a.category === f.category && a.value === f.value))
      : [...this.activeFilters, { category: f.category, value: f.value }];
    if (this.filtering) this.runQuery();
    else this.clearSelection();
  }

  clearFilters(): void {
    this.activeFilters = [];
    this.clearSelection();
  }

  runQuery(): void {
    this.loading = true;
    this.handle(this.tagsSvc.query(this.activeFilters), res => {
      this.loading = false;
      const paths = (res.success && res.data?.paths) ? res.data.paths : [];
      this.filterImages = paths.map(p => ({ name: this.basename(p), path: p }));
      this.loadThumbs(this.filterImages);
    });
  }

  // ── Thumbnails ──────────────────────────────────────────────────
  private ext(path: string): string { return path.split('.').pop()?.toLowerCase() ?? ''; }

  loadThumbs(items: BrowseEntry[]): void {
    for (const it of items) {
      if (this.thumbs.has(it.path)) continue;
      if (this.BROWSER_SAFE.has(this.ext(it.path))) {
        this.thumbs.set(it.path, convertFileSrc(it.path));
      } else {
        this.browseSvc.thumbnail(it.path)
          .then(tp => { this.thumbs.set(it.path, convertFileSrc(tp)); this.cdr.detectChanges(); })
          .catch(() => { this.thumbErr.add(it.path); this.cdr.detectChanges(); });
      }
    }
  }

  thumbOf(path: string): string | undefined { return this.thumbs.get(path); }
  hasThumbError(path: string): boolean { return this.thumbErr.has(path); }
  onThumbError(path: string): void { this.thumbErr.add(path); this.cdr.detectChanges(); }

  // ── Selection ───────────────────────────────────────────────────
  isSelected(p: string): boolean { return this.selected.has(p); }

  toggleSelect(p: string): void {
    if (this.selected.has(p)) this.selected.delete(p);
    else this.selected.add(p);
    this.cdr.detectChanges();
  }

  selectAllVisible(): void {
    for (const i of this.visibleImages) this.selected.add(i.path);
    this.cdr.detectChanges();
  }

  clearSelection(): void { this.selected.clear(); this.cdr.detectChanges(); }
  get selectedPaths(): string[] { return [...this.selected]; }

  openFile(p: string): void { this.tauri.openFilePath(p); }

  // ── Categorize panel ────────────────────────────────────────────
  openCategorize(): void {
    if (this.selected.size === 0) return;
    this.showPanel = true;
    this.customInput = '';
    this.refreshPanel();
    this.handle(this.tagsSvc.suggest(this.selectedPaths), res => {
      this.suggestions = (res.success && res.data?.suggestions) ? res.data.suggestions : [];
    });
  }

  closePanel(): void { this.showPanel = false; }

  refreshPanel(): void {
    this.handle(this.tagsSvc.get(this.selectedPaths), res => {
      this.selTags = (res.success && res.data?.tags) ? res.data.tags : [];
    });
  }

  private appliedCount(cat: string, val: string): number {
    return new Set(this.selTags.filter(t => t.category === cat && t.value === val).map(t => t.path)).size;
  }
  isFullyApplied(cat: string, val: string): boolean {
    return this.selected.size > 0 && this.appliedCount(cat, val) === this.selected.size;
  }
  isPartlyApplied(cat: string, val: string): boolean {
    const c = this.appliedCount(cat, val);
    return c > 0 && c < this.selected.size;
  }

  togglePreset(cat: string, val: string): void {
    const obs = this.isFullyApplied(cat, val)
      ? this.tagsSvc.remove(this.selectedPaths, cat, val)
      : this.tagsSvc.set(this.selectedPaths, cat, val);
    this.handle(obs, () => { this.refreshPanel(); this.loadFacets(); });
  }

  applySuggestion(s: TagSuggestion): void {
    this.handle(this.tagsSvc.set(this.selectedPaths, s.category, s.value), () => { this.refreshPanel(); this.loadFacets(); });
  }

  isSuggestionApplied(s: TagSuggestion): boolean { return this.isFullyApplied(s.category, s.value); }

  customTags(): string[] {
    return [...new Set(this.selTags.filter(t => t.category === 'custom').map(t => t.value))];
  }

  addCustom(): void {
    const v = this.customInput.trim();
    if (!v) return;
    this.handle(this.tagsSvc.set(this.selectedPaths, 'custom', v), () => {
      this.customInput = '';
      this.refreshPanel();
      this.loadFacets();
    });
  }

  removeCustom(v: string): void {
    this.handle(this.tagsSvc.remove(this.selectedPaths, 'custom', v), () => { this.refreshPanel(); this.loadFacets(); });
  }

  // ── Misc ────────────────────────────────────────────────────────
  analyzeColors(): void {
    this.handle(this.tagsSvc.backfillColors(), () => this.toastOk('Analyzing colors in the background…'));
  }

  basename(p: string): string { return p.split(/[\\/]/).filter(Boolean).pop() ?? p; }

  private toastOk(detail: string): void {
    this.messages.add({ key: 'app', severity: 'success', summary: 'Tags', detail, life: 3000 });
  }
  private toastErr(detail: string): void {
    this.messages.add({ key: 'app', severity: 'error', summary: 'Browse', detail, life: 4000 });
  }
}
