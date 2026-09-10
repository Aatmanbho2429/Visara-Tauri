import { Injectable } from '@angular/core';
import { Observable } from 'rxjs';
import { TAURI_COMMANDS } from '../../core/tauri/tauri-commands.const';
import { TAURI_EVENTS } from '../../core/tauri/tauri-events.const';
import { ZoneWrapperService } from '../../core/zone-wrapper/zone-wrapper.service';
import { requestTagsQuery, tagFilterDto } from '../../models/request/requestTags';
import {
  responseTagFacets, responseTagQuery, responseTagsGet, responseTagSuggest, responseTagsUpdated,
} from '../../models/response/responseTags';

@Injectable({ providedIn: 'root' })
export class TagsService {
  constructor(private zoneWrapper: ZoneWrapperService) {}

  // Apply one tag to every selected image.
  set(paths: string[], category: string, value: string): Observable<null> {
    return this.zoneWrapper.invoke<null>(TAURI_COMMANDS.TAGS_SET, { paths, category, value });
  }

  remove(paths: string[], category: string, value: string): Observable<null> {
    return this.zoneWrapper.invoke<null>(TAURI_COMMANDS.TAGS_REMOVE, { paths, category, value });
  }

  get(paths: string[]): Observable<responseTagsGet> {
    return this.zoneWrapper.invoke<responseTagsGet>(TAURI_COMMANDS.TAGS_GET, { paths });
  }

  facets(): Observable<responseTagFacets> {
    return this.zoneWrapper.invoke<responseTagFacets>(TAURI_COMMANDS.TAGS_FACETS);
  }

  // Files matching ALL of the given filters.
  query(filters: tagFilterDto[]): Observable<responseTagQuery> {
    return this.zoneWrapper.invoke<responseTagQuery>(TAURI_COMMANDS.TAGS_QUERY, { filters } as requestTagsQuery);
  }

  suggest(paths: string[]): Observable<responseTagSuggest> {
    return this.zoneWrapper.invoke<responseTagSuggest>(TAURI_COMMANDS.TAGS_SUGGEST, { paths });
  }

  backfillColors(): Observable<null> {
    return this.zoneWrapper.invoke<null>(TAURI_COMMANDS.TAGS_BACKFILL_COLORS);
  }

  // Fired whenever the backend finishes (re)computing colour tags in the
  // background — either the explicit "Analyze colors" action or the
  // automatic backfill that runs after every folder sync.
  onTagsUpdated(cb: (payload: responseTagsUpdated) => void): () => void {
    const sub = this.zoneWrapper.listen<responseTagsUpdated>(TAURI_EVENTS.TAGS_UPDATED).subscribe(res => {
      if (res.data) cb(res.data);
    });
    return () => sub.unsubscribe();
  }
}
