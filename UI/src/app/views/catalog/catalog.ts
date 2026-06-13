import { Component, OnInit, inject } from '@angular/core';
import { CommonModule } from '@angular/common';
import { Router } from '@angular/router';
import { TranslateModule, TranslateService } from '@ngx-translate/core';
import { BaseComponent } from '../../core/base.component';
import { CatalogService } from '../../services/catalog.service';
import { CatalogSummary } from '../../models/catalog.model';

@Component({
  selector: 'app-catalog',
  imports: [CommonModule, TranslateModule],
  templateUrl: './catalog.html',
  styleUrl: './catalog.scss',
})
export class Catalog extends BaseComponent implements OnInit {
  private catalogSvc = inject(CatalogService);
  private router     = inject(Router);
  private t          = inject(TranslateService);

  catalogs: CatalogSummary[] = [];
  loading = true;

  ngOnInit(): void { this.load(); }

  load(): void {
    this.loading = true;
    this.handle(this.catalogSvc.listThemes(), res => {
      this.loading = false;
      this.catalogs = (res.success && res.data?.themes) ? res.data.themes : [];
    });
  }

  newCatalog(): void { this.router.navigate(['/master/catalog/editor']); }
  openCatalog(id: string): void { this.router.navigate(['/master/catalog/editor'], { queryParams: { id } }); }

  deleteCatalog(id: string, ev: Event): void {
    ev.stopPropagation();
    if (!confirm(this.t.instant('catalog.deleteConfirm'))) return;
    this.handle(this.catalogSvc.deleteTheme(id), () => this.load());
  }

  ago(ts: number): string {
    if (!ts) return '';
    const d = Date.now() / 1000 - ts;
    if (d < 3600) return `${Math.floor(d / 60)}${this.t.instant('library.agoMin')}`;
    if (d < 86400) return `${Math.floor(d / 3600)}${this.t.instant('library.agoHour')}`;
    return `${Math.floor(d / 86400)}${this.t.instant('library.agoDay')}`;
  }
}
