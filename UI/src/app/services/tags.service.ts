import { Injectable } from '@angular/core';
import { Observable } from 'rxjs';
import { BaseResponse } from '../models/base-response.model';
import {
  TagFacetsData, TagQueryData, TagsGetData, TagSuggestData, TagSuggestion,
} from '../models/browse.model';
import { TauriService } from './tauri.service';

@Injectable({ providedIn: 'root' })
export class TagsService {
  constructor(private tauri: TauriService) {}

  /** Apply one tag to every selected image. */
  set(paths: string[], category: string, value: string): Observable<BaseResponse<null>> {
    return this.tauri.invoke<null>('tags_set', { paths, category, value });
  }

  remove(paths: string[], category: string, value: string): Observable<BaseResponse<null>> {
    return this.tauri.invoke<null>('tags_remove', { paths, category, value });
  }

  get(paths: string[]): Observable<BaseResponse<TagsGetData>> {
    return this.tauri.invoke<TagsGetData>('tags_get', { paths });
  }

  facets(): Observable<BaseResponse<TagFacetsData>> {
    return this.tauri.invoke<TagFacetsData>('tags_facets');
  }

  /** Files matching ALL of the given filters. */
  query(filters: TagSuggestion[]): Observable<BaseResponse<TagQueryData>> {
    return this.tauri.invoke<TagQueryData>('tags_query', { filters });
  }

  suggest(paths: string[]): Observable<BaseResponse<TagSuggestData>> {
    return this.tauri.invoke<TagSuggestData>('tags_suggest', { paths });
  }

  backfillColors(): Observable<BaseResponse<null>> {
    return this.tauri.invoke<null>('tags_backfill_colors');
  }
}
