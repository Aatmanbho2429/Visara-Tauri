import { Injectable } from '@angular/core';
import { Observable } from 'rxjs';
import { TAURI_COMMANDS } from '../../core/tauri/tauri-commands.const';
import { TAURI_EVENTS } from '../../core/tauri/tauri-events.const';
import { ZoneWrapperService } from '../../core/zone-wrapper/zone-wrapper.service';
import { requestRemoveFolder, requestSetPaused } from '../../models/request/requestLibrary';
import { responseFolderTree, responseLibraryStats, responseListFolders } from '../../models/response/responseLibrary';
import {
  responseLibrarySyncComplete, responseLibrarySyncError,
  responseLibrarySyncProgress, responseLibrarySyncStarted,
} from '../../models/response/responseSync';

export interface LibrarySyncHandlers {
  started?:  (p: responseLibrarySyncStarted) => void;
  progress?: (p: responseLibrarySyncProgress) => void;
  complete?: (p: responseLibrarySyncComplete) => void;
  error?:    (p: responseLibrarySyncError) => void;
}

@Injectable({ providedIn: 'root' })
export class LibraryService {
  constructor(private zoneWrapper: ZoneWrapperService) {}

  list(): Observable<responseListFolders> {
    return this.zoneWrapper.invoke<responseListFolders>(TAURI_COMMANDS.LIBRARY_LIST_FOLDERS);
  }

  // Same call, without the global loading spinner — for background checks
  // (e.g. search peeking at folder status before it starts).
  listSilent(): Observable<responseListFolders> {
    return this.zoneWrapper.invokeSilent<responseListFolders>(TAURI_COMMANDS.LIBRARY_LIST_FOLDERS);
  }

  add(path: string): Observable<null> {
    return this.zoneWrapper.invoke<null>(TAURI_COMMANDS.LIBRARY_ADD_FOLDER, { path });
  }

  // `purge=true` also deletes every embedding under that folder.
  remove(path: string, purge: boolean): Observable<null> {
    return this.zoneWrapper.invoke<null>(TAURI_COMMANDS.LIBRARY_REMOVE_FOLDER, { path, purge } as requestRemoveFolder);
  }

  setPaused(path: string, paused: boolean): Observable<null> {
    return this.zoneWrapper.invoke<null>(TAURI_COMMANDS.LIBRARY_SET_PAUSED, { path, paused } as requestSetPaused);
  }

  rescan(path: string): Observable<null> {
    return this.zoneWrapper.invoke<null>(TAURI_COMMANDS.LIBRARY_RESCAN_FOLDER, { path });
  }

  stats(): Observable<responseLibraryStats> {
    return this.zoneWrapper.invoke<responseLibraryStats>(TAURI_COMMANDS.LIBRARY_STATS);
  }

  // Nested subfolder tree (with image counts) for one watched folder.
  folderTree(path: string): Observable<responseFolderTree> {
    return this.zoneWrapper.invoke<responseFolderTree>(TAURI_COMMANDS.LIBRARY_FOLDER_TREE, { path });
  }

  // Subscribe to live per-folder sync events emitted by the background
  // watcher. Returns an unsubscribe function that detaches every handler.
  onLibrarySync(handlers: LibrarySyncHandlers): () => void {
    const subs = [
      this.zoneWrapper.listen<responseLibrarySyncStarted>(TAURI_EVENTS.LIBRARY_SYNC_STARTED)
        .subscribe(res => { if (res.data && handlers.started) handlers.started(res.data); }),
      this.zoneWrapper.listen<responseLibrarySyncProgress>(TAURI_EVENTS.LIBRARY_SYNC_PROGRESS)
        .subscribe(res => { if (res.data && handlers.progress) handlers.progress(res.data); }),
      this.zoneWrapper.listen<responseLibrarySyncComplete>(TAURI_EVENTS.LIBRARY_SYNC_COMPLETE)
        .subscribe(res => { if (res.data && handlers.complete) handlers.complete(res.data); }),
      this.zoneWrapper.listen<responseLibrarySyncError>(TAURI_EVENTS.LIBRARY_SYNC_ERROR)
        .subscribe(res => { if (res.data && handlers.error) handlers.error(res.data); }),
    ];
    return () => subs.forEach(s => s.unsubscribe());
  }
}
