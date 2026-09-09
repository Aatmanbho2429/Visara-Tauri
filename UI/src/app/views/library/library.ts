import { Component, OnDestroy, OnInit, inject } from '@angular/core';
import { CommonModule } from '@angular/common';
import { TranslateModule, TranslateService } from '@ngx-translate/core';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { MessageService } from 'primeng/api';
import { BaseComponent } from '../../core/base.component';
import { LibraryService } from '../../services/library/library.service';
import { FolderTreeNode, WatchedFolder, WatchedFolderStatus } from '../../models/response/responseLibrary';

// Live progress for one folder currently being indexed.
interface FolderProgress {
  phase:   string;
  done:    number;
  total:   number;
  percent: number;
  current: string;
  etaSec:  number;
  errors:  number;
}

@Component({
  selector: 'app-library',
  imports: [CommonModule, TranslateModule],
  templateUrl: './library.html',
  styleUrl: './library.scss',
})
export class Library extends BaseComponent implements OnInit, OnDestroy {
  private libSvc   = inject(LibraryService);
  private messages = inject(MessageService);
  private t        = inject(TranslateService);

  folders: WatchedFolder[] = [];
  loading                  = true;
  // Path currently being added — disables the Add button while running.
  addingPath: string | null = null;
  // Path with an in-flight action (remove/pause/rescan) — keeps card busy.
  busyPath: string | null   = null;

  // Live per-folder indexing progress, keyed by folder path.
  syncProgress: Record<string, FolderProgress> = {};
  // Last error summary per folder, keyed by folder path.
  syncError:    Record<string, string> = {};
  // Per-folder list of files that could not be indexed, with the reason.
  syncFailed:   Record<string, { file: string; reason: string }[]> = {};
  // Which folders have their failure list expanded in the UI.
  errorsExpanded: Record<string, boolean> = {};

  // Subfolder-tree panel state, keyed by folder path.
  treeOpen:    Record<string, boolean> = {};
  treeLoading: Record<string, boolean> = {};
  tree:        Record<string, FolderTreeNode | null> = {};

  private unlistenSync: (() => void) | null = null;

  ngOnInit(): void {
    this.refresh();
    this.subscribeToSyncEvents();
  }

  ngOnDestroy(): void {
    this.unlistenSync?.();
  }

  // ── Live sync events from the background watcher ──────────────────

  private subscribeToSyncEvents(): void {
    this.unlistenSync = this.libSvc.onLibrarySync({
      started: ({ path }) => {
        this.syncProgress[path] = { phase: 'Starting…', done: 0, total: 0, percent: 0, current: '', etaSec: -1, errors: 0 };
        delete this.syncError[path];
        this.patchFolder(path, { status: 'indexing' });
        this.cdr.detectChanges();
      },
      progress: ({ path, progress }) => {
        this.syncProgress[path] = {
          phase:   progress.phase,
          done:    progress.done,
          total:   progress.total,
          percent: progress.percent,
          current: progress.current,
          etaSec:  progress.etaSec,
          errors:  progress.errors,
        };
        this.patchFolder(path, { status: 'indexing' });
        this.cdr.detectChanges();
      },
      complete: ({ path, errors, failed, imageCount }) => {
        delete this.syncProgress[path];
        this.patchFolder(path, {
          status:      'watching',
          imageCount,
          lastEventAt: Date.now() / 1000,
        });
        if (errors > 0) {
          const noun = this.t.instant(errors === 1 ? 'library.file' : 'library.files');
          this.syncError[path]  = this.t.instant('library.filesCouldNotIndex', { count: errors, noun });
          this.syncFailed[path] = failed ?? [];
          this.toastWarn(this.t.instant('library.folderSkipped', { name: this.shortName(path), count: errors, noun }));
        } else {
          delete this.syncError[path];
          delete this.syncFailed[path];
          delete this.errorsExpanded[path];
          this.toastSuccess(this.t.instant('library.upToDate', { name: this.shortName(path) }));
        }
        this.cdr.detectChanges();
      },
      error: ({ path, message }) => {
        delete this.syncProgress[path];
        this.syncError[path] = message;
        this.patchFolder(path, { status: 'error' });
        this.toastError(this.shortName(path) + ': ' + message);
        this.cdr.detectChanges();
      },
    });
  }

  // Merge partial updates into the matching folder card, if present.
  private patchFolder(path: string, patch: Partial<WatchedFolder>): void {
    const folder = this.folders.find(f => f.path === path);
    if (folder) Object.assign(folder, patch);
  }

  shortName(path: string): string {
    return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
  }

  isSyncing(path: string): boolean {
    return !!this.syncProgress[path];
  }

  // Toggle the expanded list of files that failed to index for a folder.
  toggleErrorDetails(path: string): void {
    this.errorsExpanded[path] = !this.errorsExpanded[path];
    this.cdr.detectChanges();
  }

  // Toggle the subfolder-tree panel; lazily loads the tree on first open.
  toggleTree(folder: WatchedFolder): void {
    const path = folder.path;
    if (this.treeOpen[path]) {
      this.treeOpen[path] = false;
      this.cdr.detectChanges();
      return;
    }
    this.treeOpen[path] = true;
    if (this.tree[path] === undefined) {
      this.treeLoading[path] = true;
      this.handle(
        this.libSvc.folderTree(path),
        data => {
          this.treeLoading[path] = false;
          this.tree[path] = data?.tree ?? null;
        },
        err => {
          this.treeLoading[path] = false;
          this.tree[path] = null;
          this.toastError(err.message || this.t.instant('library.couldNotLoadSubfolders'));
        },
      );
    }
    this.cdr.detectChanges();
  }

  refresh(): void {
    this.loading = true;
    // Drop cached subfolder trees so a reopened panel reflects fresh counts
    // after an add / remove / re-scan.
    this.treeOpen = {};
    this.treeLoading = {};
    this.tree = {};
    this.handle(
      this.libSvc.list(),
      data => {
        this.loading = false;
        this.folders = data.folders;
      },
      err => {
        this.loading = false;
        this.folders = [];
        this.toastError(err.message || this.t.instant('library.couldNotLoad'));
      },
    );
  }

  async addFolder(): Promise<void> {
    const picked = await openDialog({ directory: true, multiple: false });
    if (!picked) return;
    const path = picked as string;

    if (this.folders.some(f => f.path === path)) {
      this.toastWarn(this.t.instant('library.alreadyWatched'));
      return;
    }

    this.addingPath = path;
    this.handle(
      this.libSvc.add(path),
      () => {
        this.addingPath = null;
        // Backend message may note that redundant subfolders were merged in.
        this.toastSuccess(this.t.instant('library.addedDefault'));
        this.refresh();
      },
      err => {
        this.addingPath = null;
        this.toastError(err.message || this.t.instant('library.couldNotAdd'));
      },
    );
  }

  removeFolder(folder: WatchedFolder, purge: boolean): void {
    const confirmMsg = this.t.instant(purge ? 'library.removeConfirmPurge' : 'library.removeConfirm', { path: folder.path });
    if (!confirm(confirmMsg)) return;

    this.busyPath = folder.path;
    this.handle(
      this.libSvc.remove(folder.path, purge),
      () => {
        this.busyPath = null;
        this.toastSuccess(this.t.instant(purge ? 'library.removedPurge' : 'library.removed'));
        this.refresh();
      },
      err => {
        this.busyPath = null;
        this.toastError(err.message || this.t.instant('library.couldNotRemove'));
      },
    );
  }

  togglePause(folder: WatchedFolder): void {
    const willPause = folder.status !== 'paused';
    this.busyPath = folder.path;
    this.handle(
      this.libSvc.setPaused(folder.path, willPause),
      () => { this.busyPath = null; this.refresh(); },
      err => { this.busyPath = null; this.toastError(err.message || this.t.instant('library.couldNotChangeState')); },
    );
  }

  rescan(folder: WatchedFolder): void {
    this.busyPath = folder.path;
    this.handle(
      this.libSvc.rescan(folder.path),
      () => {
        this.busyPath = null;
        this.toastSuccess(this.t.instant('library.rescanStarted'));
        // Refresh after a short delay so the status flips to "indexing".
        setTimeout(() => this.refresh(), 800);
      },
      err => {
        this.busyPath = null;
        this.toastError(err.message || this.t.instant('library.couldNotRescan'));
      },
    );
  }

  // ── View helpers ─────────────────────────────────────────────────

  totalImages(): number {
    return this.folders.reduce((sum, f) => sum + f.imageCount, 0);
  }

  // Returns a translation key; the template pipes it through `translate`.
  statusLabel(status: WatchedFolderStatus): string {
    return 'library.status.' + status;
  }

  // Relative-time helper for "last update X ago".
  agoLabel(timestampSec: number): string {
    if (!timestampSec) return '';
    const diff = Date.now() / 1000 - timestampSec;
    if (diff < 60)    return `${Math.floor(diff)}${this.t.instant('library.agoSec')}`;
    if (diff < 3600)  return `${Math.floor(diff / 60)}${this.t.instant('library.agoMin')}`;
    if (diff < 86400) return `${Math.floor(diff / 3600)}${this.t.instant('library.agoHour')}`;
    return `${Math.floor(diff / 86400)}${this.t.instant('library.agoDay')}`;
  }

  // ── Toast helpers ────────────────────────────────────────────────

  private toastSuccess(detail: string): void {
    this.messages.add({ key: 'app', severity: 'success', summary: this.t.instant('library.toastTitle'), detail, life: 3500 });
  }
  private toastWarn(detail: string): void {
    this.messages.add({ key: 'app', severity: 'warn', summary: this.t.instant('library.toastTitle'), detail, life: 4000 });
  }
  private toastError(detail: string): void {
    this.messages.add({ key: 'app', severity: 'error', summary: this.t.instant('library.toastTitle'), detail, life: 5000 });
  }
}
