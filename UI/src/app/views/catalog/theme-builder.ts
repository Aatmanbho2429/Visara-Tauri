// Canva-style catalog editor. (File name kept for route stability.)
import { Component, HostListener, OnInit, inject } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { ActivatedRoute, Router } from '@angular/router';
import { convertFileSrc } from '@tauri-apps/api/core';
import { save as saveDialog } from '@tauri-apps/plugin-dialog';
import { MessageService } from 'primeng/api';
import { BaseComponent } from '../../core/base.component';
import { BrowseService } from '../../services/browse.service';
import { CatalogService } from '../../services/catalog.service';
import { TagsService } from '../../services/tags.service';
import {
  CatalogDoc, DocElement, FONTS, PAGE_PRESETS, PagePreset, PageDoc,
  blankPage, clonePage, defaultCatalog, fontCss, uid,
} from '../../models/catalog.model';
import { TagFacet } from '../../models/browse.model';
import { blobToDataUrl, imageDims, pdfBase64, resolveDoc } from './catalog-render';

@Component({
  selector: 'app-editor',
  imports: [CommonModule, FormsModule],
  templateUrl: './theme-builder.html',
  styleUrl: './theme-builder.scss',
})
export class Editor extends BaseComponent implements OnInit {
  private catalogSvc = inject(CatalogService);
  private tagsSvc    = inject(TagsService);
  private browseSvc  = inject(BrowseService);
  private route      = inject(ActivatedRoute);
  private router     = inject(Router);
  private messages   = inject(MessageService);

  doc: CatalogDoc = defaultCatalog();
  pageIndex = 0;
  selectedId: string | null = null;
  leftPanel: 'elements' | 'images' = 'elements';

  // image library panel
  facetGroups: { category: string; label: string; facets: TagFacet[] }[] = [];
  activeFilters: { category: string; value: string }[] = [];
  libImages: string[] = [];
  libLoading = false;
  thumbs = new Map<string, string>();

  exporting = false;

  readonly fonts = FONTS;
  readonly pagePresets = PAGE_PRESETS;
  private readonly BROWSER_SAFE = new Set(['jpg', 'jpeg', 'png', 'gif', 'bmp', 'webp']);

  // drag/resize state
  private mode: 'move' | 'resize' | null = null;
  private sx = 0; private sy = 0;
  private sEl = { x: 0, y: 0, w: 0, h: 0 };
  private rect: DOMRect | null = null;

  // smart-guide snapping
  guidesV: number[] = [];
  guidesH: number[] = [];
  private readonly SNAP = 1.2; // % threshold

  ngOnInit(): void {
    const id = this.route.snapshot.queryParamMap.get('id');
    if (id) {
      this.handle(this.catalogSvc.getTheme(id), res => {
        if (res.success && res.data?.theme) {
          try {
            this.doc = JSON.parse(res.data.theme.json);
            this.doc.pages.forEach(p => p.elements.forEach(e => { if (e.type === 'image' && e.src) this.loadThumb(e.src); }));
          } catch { /* keep blank */ }
        }
      });
    }
    this.loadFacets();
    this.loadLibrary();
  }

  // ── Pages ────────────────────────────────────────────────────────
  get page(): PageDoc { return this.doc.pages[this.pageIndex]; }
  get selected(): DocElement | null { return this.page?.elements.find(e => e.id === this.selectedId) ?? null; }
  get canvasAspect(): number { return this.doc.pageW / this.doc.pageH; }
  /** 1pt as a fraction of page height (×1cqh gives px that scale with the canvas). */
  get edPt(): number { return 35.28 / this.doc.pageH; }

  selectPage(i: number): void { this.pageIndex = i; this.selectedId = null; this.cdr.detectChanges(); }
  addPage(): void { this.doc.pages.push(blankPage()); this.selectPage(this.doc.pages.length - 1); }
  duplicatePage(): void {
    this.doc.pages.splice(this.pageIndex + 1, 0, clonePage(this.page));
    this.selectPage(this.pageIndex + 1);
  }
  deletePage(i: number, ev?: Event): void {
    ev?.stopPropagation();
    if (this.doc.pages.length <= 1) return;
    this.doc.pages.splice(i, 1);
    this.pageIndex = Math.max(0, Math.min(this.pageIndex, this.doc.pages.length - 1));
    this.selectedId = null;
    this.cdr.detectChanges();
  }
  movePage(i: number, dir: number, ev: Event): void {
    ev.stopPropagation();
    const j = i + dir;
    if (j < 0 || j >= this.doc.pages.length) return;
    const [p] = this.doc.pages.splice(i, 1);
    this.doc.pages.splice(j, 0, p);
    this.pageIndex = j;
    this.cdr.detectChanges();
  }

  setPageSize(p: PagePreset): void { this.doc.pageW = p.w; this.doc.pageH = p.h; this.cdr.detectChanges(); }

  // ── Elements ─────────────────────────────────────────────────────
  addText(heading = false): void {
    this.page.elements.push({
      id: uid(), type: 'text', x: 12, y: 14, w: 60, h: heading ? 10 : 7,
      content: heading ? 'Heading' : 'Add your text',
      font: 'grotesk', size: heading ? 28 : 13, bold: heading, align: 'left', color: '#222222',
    });
    this.selectLast();
  }
  addBox(): void {
    this.page.elements.push({ id: uid(), type: 'box', x: 20, y: 20, w: 30, h: 20, fill: '#f0e9f5', radius: 4 });
    this.selectLast();
  }
  addLine(): void {
    this.page.elements.push({ id: uid(), type: 'line', x: 20, y: 30, w: 40, h: 2, thickness: 2, lineColor: '#d946ef' });
    this.selectLast();
  }
  addImage(path: string): void {
    this.loadThumb(path);
    this.page.elements.push({ id: uid(), type: 'image', x: 15, y: 15, w: 45, h: 45, src: path, fit: 'cover', radius: 4 });
    this.selectLast();
  }
  private selectLast(): void {
    const arr = this.page.elements;
    this.selectedId = arr[arr.length - 1].id;
    this.cdr.detectChanges();
  }
  deleteSelected(): void {
    if (!this.selectedId) return;
    const i = this.page.elements.findIndex(e => e.id === this.selectedId);
    if (i >= 0) this.page.elements.splice(i, 1);
    this.selectedId = null;
    this.cdr.detectChanges();
  }
  selectEl(id: string, ev?: Event): void { ev?.stopPropagation(); this.selectedId = id; }
  clearSel(): void { this.selectedId = null; }

  bringForward(): void { this.shift(1); }
  sendBackward(): void { this.shift(-1); }
  private shift(dir: number): void {
    const arr = this.page.elements;
    const i = arr.findIndex(e => e.id === this.selectedId);
    const j = i + dir;
    if (i < 0 || j < 0 || j >= arr.length) return;
    [arr[i], arr[j]] = [arr[j], arr[i]];
    this.cdr.detectChanges();
  }

  // ── Drag / resize ────────────────────────────────────────────────
  startMove(ev: PointerEvent, el: DocElement, canvas: HTMLElement): void {
    ev.preventDefault(); ev.stopPropagation();
    this.selectedId = el.id; this.mode = 'move';
    this.sx = ev.clientX; this.sy = ev.clientY;
    this.sEl = { x: el.x, y: el.y, w: el.w, h: el.h };
    this.rect = canvas.getBoundingClientRect();
  }
  startResize(ev: PointerEvent, el: DocElement, canvas: HTMLElement): void {
    ev.preventDefault(); ev.stopPropagation();
    this.selectedId = el.id; this.mode = 'resize';
    this.sx = ev.clientX; this.sy = ev.clientY;
    this.sEl = { x: el.x, y: el.y, w: el.w, h: el.h };
    this.rect = canvas.getBoundingClientRect();
  }
  @HostListener('document:pointermove', ['$event'])
  onMove(ev: PointerEvent): void {
    if (!this.mode || !this.rect || !this.selected) return;
    const dx = (ev.clientX - this.sx) / this.rect.width * 100;
    const dy = (ev.clientY - this.sy) / this.rect.height * 100;
    const el = this.selected;
    if (this.mode === 'move') {
      const nx = clamp(this.sEl.x + dx, 0, 100 - el.w);
      const ny = clamp(this.sEl.y + dy, 0, 100 - el.h);
      const snap = this.computeSnap(el, nx, ny);
      el.x = snap.x; el.y = snap.y;
    } else {
      el.w = clamp(this.sEl.w + dx, 3, 100 - el.x);
      el.h = clamp(this.sEl.h + dy, 1, 100 - el.y);
    }
    this.cdr.detectChanges();
  }
  @HostListener('document:pointerup')
  onUp(): void { this.mode = null; this.rect = null; this.guidesV = []; this.guidesH = []; this.cdr.detectChanges(); }

  /** Snap the moving element's edges/centre to the page and other elements,
   *  recording guide lines to display. */
  private computeSnap(el: DocElement, nx: number, ny: number): { x: number; y: number } {
    const others = this.page.elements.filter(e => e.id !== el.id);
    const vT = [0, 50, 100];
    const hT = [0, 50, 100];
    for (const o of others) {
      vT.push(o.x, o.x + o.w / 2, o.x + o.w);
      hT.push(o.y, o.y + o.h / 2, o.y + o.h);
    }
    let x = nx, y = ny;
    const gv: number[] = [];
    const gh: number[] = [];

    outerV:
    for (const off of [0, el.w / 2, el.w]) {
      for (const t of vT) {
        if (Math.abs((nx + off) - t) <= this.SNAP) { x = t - off; gv.push(t); break outerV; }
      }
    }
    outerH:
    for (const off of [0, el.h / 2, el.h]) {
      for (const t of hT) {
        if (Math.abs((ny + off) - t) <= this.SNAP) { y = t - off; gh.push(t); break outerH; }
      }
    }

    this.guidesV = gv;
    this.guidesH = gh;
    return { x: clamp(x, 0, 100 - el.w), y: clamp(y, 0, 100 - el.h) };
  }

  // ── Image library panel ──────────────────────────────────────────
  loadFacets(): void {
    this.handle(this.tagsSvc.facets(), res => {
      const facets = (res.success && res.data?.facets) ? res.data.facets : [];
      const order = ['color', 'material', 'finish', 'size', 'design', 'collection', 'custom'];
      const labels: Record<string, string> = { color: 'Color', material: 'Material', finish: 'Finish', size: 'Size', design: 'Design', collection: 'Collection', custom: 'My Tags' };
      this.facetGroups = order
        .map(c => ({ category: c, label: labels[c] ?? c, facets: facets.filter(f => f.category === c) }))
        .filter(g => g.facets.length > 0);
    });
  }
  isFilterActive(f: TagFacet): boolean { return this.activeFilters.some(a => a.category === f.category && a.value === f.value); }
  toggleFilter(f: TagFacet): void {
    this.activeFilters = this.isFilterActive(f)
      ? this.activeFilters.filter(a => !(a.category === f.category && a.value === f.value))
      : [...this.activeFilters, { category: f.category, value: f.value }];
    this.loadLibrary();
  }
  loadLibrary(): void {
    this.libLoading = true;
    this.handle(this.tagsSvc.query(this.activeFilters), res => {
      this.libLoading = false;
      this.libImages = (res.success && res.data?.paths) ? res.data.paths : [];
      this.libImages.slice(0, 80).forEach(p => this.loadThumb(p));
    });
  }

  // ── Thumbnails ───────────────────────────────────────────────────
  private ext(p: string): string { return p.split('.').pop()?.toLowerCase() ?? ''; }
  loadThumb(path: string): void {
    if (this.thumbs.has(path)) return;
    if (this.BROWSER_SAFE.has(this.ext(path))) {
      this.thumbs.set(path, convertFileSrc(path));
    } else {
      this.browseSvc.thumbnail(path)
        .then(tp => { this.thumbs.set(path, convertFileSrc(tp)); this.cdr.detectChanges(); })
        .catch(() => {});
    }
  }
  thumbOf(path: string): string | undefined { return this.thumbs.get(path); }
  shortName(p: string): string { return p.split(/[\\/]/).pop() ?? p; }

  // ── Styles ───────────────────────────────────────────────────────
  elStyle(el: DocElement): Record<string, string> {
    const s: Record<string, string> = { left: el.x + '%', top: el.y + '%', width: el.w + '%', height: el.h + '%' };
    if (el.type === 'text') {
      s['fontFamily'] = fontCss(el.font);
      s['fontSize'] = 'calc(' + (el.size ?? 12) + ' * var(--ed-pt) * 1cqh)';
      s['fontWeight'] = el.bold ? '700' : '400';
      s['fontStyle'] = el.italic ? 'italic' : 'normal';
      s['textAlign'] = el.align ?? 'left';
      s['color'] = el.color ?? '#222';
    }
    return s;
  }

  // ── Save / export ────────────────────────────────────────────────
  save(): void {
    if (!this.doc.name?.trim()) this.doc.name = 'Untitled catalog';
    this.handle(this.catalogSvc.saveTheme(this.doc.id, this.doc.name, JSON.stringify(this.doc)), res => {
      if (res.success) this.toast('success', 'Saved.');
      else this.toast('error', res.message || 'Save failed.');
    });
  }

  async exportPdf(): Promise<void> {
    this.exporting = true; this.cdr.detectChanges();
    try {
      const pages = await resolveDoc(this.doc, p => this.resolveImg(p));
      const b64 = pdfBase64(pages, this.doc.pageW, this.doc.pageH);
      const name = (this.doc.name || 'catalog').replace(/[^a-z0-9]+/gi, '_').toLowerCase();
      const path = await saveDialog({ defaultPath: `${name}.pdf`, filters: [{ name: 'PDF', extensions: ['pdf'] }] });
      if (path) {
        this.handle(this.catalogSvc.savePdf(path, b64), res => {
          this.toast(res.success ? 'success' : 'error', res.success ? 'Catalog exported.' : (res.message || 'Export failed.'));
        });
      }
    } catch (e) {
      this.toast('error', 'Export failed: ' + e);
    }
    this.exporting = false; this.cdr.detectChanges();
  }

  private async resolveImg(path: string) {
    const renderPath = await this.catalogSvc.catalogImage(path);
    const url = convertFileSrc(renderPath);
    const blob = await (await fetch(url)).blob();
    const dataUrl = await blobToDataUrl(blob);
    const dims = await imageDims(dataUrl);
    return { dataUrl, w: dims.w, h: dims.h };
  }

  back(): void { this.router.navigate(['/master/catalog']); }

  private toast(severity: string, detail: string): void {
    this.messages.add({ key: 'app', severity, summary: 'Catalog', detail, life: 3000 });
  }
}

function clamp(v: number, lo: number, hi: number): number { return Math.max(lo, Math.min(hi, v)); }
