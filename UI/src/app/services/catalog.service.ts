import { Injectable } from '@angular/core';
import { invoke } from '@tauri-apps/api/core';
import { Observable } from 'rxjs';
import { BaseResponse } from '../models/base-response.model';
import { CatalogSummary } from '../models/catalog.model';
import { TauriService } from './tauri.service';

export interface ThemeRecord { id: string; name: string; json: string; updated_at: number; }

@Injectable({ providedIn: 'root' })
export class CatalogService {
  /** Tile paths handed over from the Browse "Generate Catalog" action. */
  selection: string[] = [];

  constructor(private tauri: TauriService) {}

  setSelection(paths: string[]): void { this.selection = [...paths]; }
  clearSelection(): void { this.selection = []; }

  saveTheme(id: string, name: string, json: string): Observable<BaseResponse<{ id: string }>> {
    return this.tauri.invoke<{ id: string }>('catalog_save_theme', { id, name, json });
  }
  listThemes(): Observable<BaseResponse<{ themes: CatalogSummary[] }>> {
    return this.tauri.invoke<{ themes: CatalogSummary[] }>('catalog_list_themes');
  }
  getTheme(id: string): Observable<BaseResponse<{ theme: ThemeRecord }>> {
    return this.tauri.invoke<{ theme: ThemeRecord }>('catalog_get_theme', { id });
  }
  deleteTheme(id: string): Observable<BaseResponse<null>> {
    return this.tauri.invoke<null>('catalog_delete_theme', { id });
  }

  /** Catalog-quality, colour-correct render path for an image. */
  catalogImage(path: string): Promise<string> {
    return invoke<string>('get_catalog_image', { path });
  }

  /** Write a base64 PDF to a user-chosen path. */
  savePdf(path: string, base64: string): Observable<BaseResponse<{ path: string }>> {
    return this.tauri.invoke<{ path: string }>('catalog_save_pdf', { path, base64 });
  }
}
