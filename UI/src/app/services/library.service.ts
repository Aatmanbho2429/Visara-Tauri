import { Injectable } from '@angular/core';
import { Observable } from 'rxjs';
import { BaseResponse } from '../models/base-response.model';
import { FolderTreeData, LibraryStats, ListFoldersData } from '../models/library.model';
import { TauriService } from './tauri.service';

@Injectable({ providedIn: 'root' })
export class LibraryService {
  constructor(private tauri: TauriService) {}

  list(): Observable<BaseResponse<ListFoldersData>> {
    return this.tauri.invoke<ListFoldersData>('library_list_folders');
  }

  /** Same call, without the global loading spinner — for background checks
   *  (e.g. search peeking at folder status before it starts). */
  listSilent(): Observable<BaseResponse<ListFoldersData>> {
    return this.tauri.invokeSilent<ListFoldersData>('library_list_folders');
  }

  add(path: string): Observable<BaseResponse<null>> {
    return this.tauri.invoke<null>('library_add_folder', { path });
  }

  /** `purge=true` also deletes every embedding under that folder. */
  remove(path: string, purge: boolean): Observable<BaseResponse<null>> {
    return this.tauri.invoke<null>('library_remove_folder', { path, purge });
  }

  setPaused(path: string, paused: boolean): Observable<BaseResponse<null>> {
    return this.tauri.invoke<null>('library_set_paused', { path, paused });
  }

  rescan(path: string): Observable<BaseResponse<null>> {
    return this.tauri.invoke<null>('library_rescan_folder', { path });
  }

  stats(): Observable<BaseResponse<LibraryStats>> {
    return this.tauri.invoke<LibraryStats>('library_stats');
  }

  /** Nested subfolder tree (with image counts) for one watched folder. */
  folderTree(path: string): Observable<BaseResponse<FolderTreeData>> {
    return this.tauri.invoke<FolderTreeData>('library_folder_tree', { path });
  }
}
