import { Component, OnDestroy, OnInit, inject } from '@angular/core';
import { CommonModule } from '@angular/common';
import { TranslateModule } from '@ngx-translate/core';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { MessageService } from 'primeng/api';
import { BaseComponent } from '../../core/base.component';
import { LibraryService } from '../../services/library.service';
import { TauriService } from '../../services/tauri.service';
import { FolderTreeNode, WatchedFolder, WatchedFolderStatus } from '../../models/library.model';

/** Live progress for one folder currently being indexed. */
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
  private tauri    = inject(TauriService);
  private messages = inject(MessageService);

  folders: WatchedFolder[] = [];
  loading                  = true;
  /** Path currently being added — disables the Add button while running. */
  addingPath: string | null = null;
  /** Path with an in-flight action (remove/pause/rescan) — keeps card busy. */
  busyPath: string | null   = null;

  /** Live per-folder indexing progress, keyed by folder path. */
  syncProgress: Record<string, FolderProgress> = {};
  /** Last error summary per folder, keyed by folder path. */
  syncError:    Record<string, string> = {};
  /** Per-folder list of files that could not be indexed, with the reason. */
  syncFailed:   Record<string, { file: string; reason: string }[]> = {};
  /** Which folders have their failure list expanded in the UI. */
  errorsExpanded: Record<string, boolean> = {};

  /** Subfolder-tree panel state, keyed by folder path. */
  treeOpen:    Record<string, boolean> = {};
  treeLoading: Record<string, boolean> = {};
  tree:        Record<string, FolderTreeNode | null> = {};

  private unlistenSync: UnlistenFn | null = null;

  ngOnInit(): void {
    this.refresh();
    this.subscribeToSyncEvents();
  }

  ngOnDestroy(): void {
    this.unlistenSync?.();
  }

  // ── Live sync events from the background watcher ──────────────────

  private subscribeToSyncEvents(): void {
    this.tauri.onLibrarySync({
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
          etaSec:  progress.eta_sec,
          errors:  progress.errors,
        };
        this.patchFolder(path, { status: 'indexing' });
        this.cdr.detectChanges();
      },
      complete: ({ path, errors, failed, image_count }) => {
        delete this.syncProgress[path];
        this.patchFolder(path, {
          status:        'watching',
          image_count,
          last_event_at: Date.now() / 1000,
        });
        if (errors > 0) {
          this.syncError[path]  = `${errors} ${errors === 1 ? 'file' : 'files'} could not be indexed.`;
          this.syncFailed[path] = failed ?? [];
          this.toastWarn(`${this.shortName(path)}: ${errors} ${errors === 1 ? 'file' : 'files'} skipped.`);
        } else {
          delete this.syncError[path];
          delete this.syncFailed[path];
          delete this.errorsExpanded[path];
          this.toastSuccess(`${this.shortName(path)} is up to date.`);
        }
        this.cdr.detectChanges();
      },
      error: ({ path, message }) => {
        delete this.syncProgress[path];
        this.syncError[path] = message;
        this.patchFolder(path, { status: 'error' });
        this.toastError(`${this.shortName(path)}: ${message}`);
        this.cdr.detectChanges();
      },
    }).then(fn => this.unlistenSync = fn);
  }

  /** Merge partial updates into the matching folder card, if present. */
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

  /** Toggle the expanded list of files that failed to index for a folder. */
  toggleErrorDetails(path: string): void {
    this.errorsExpanded[path] = !this.errorsExpanded[path];
    this.cdr.detectChanges();
  }

  /** Toggle the subfolder-tree panel; lazily loads the tree on first open. */
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
      this.handle(this.libSvc.folderTree(path), res => {
        this.treeLoading[path] = false;
        this.tree[path] = (res.success && res.data) ? res.data.tree : null;
        if (!res.success) this.toastError(res.message || 'Could not load subfolders.');
        this.cdr.detectChanges();
      });
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
    this.handle(this.libSvc.list(), res => {
      this.loading = false;
      if (res.success && res.data) {
        this.folders = res.data.folders;
      } else {
        this.folders = [];
        this.toastError(res.message || 'Could not load folders.');
      }
    });
  }

  async addFolder(): Promise<void> {
    const picked = await openDialog({ directory: true, multiple: false });
    if (!picked) return;
    const path = picked as string;

    if (this.folders.some(f => f.path === path)) {
      this.toastWarn('That folder is already being watched.');
      return;
    }

    this.addingPath = path;
    this.handle(this.libSvc.add(path), res => {
      this.addingPath = null;
      if (res.success) {
        // Backend message may note that redundant subfolders were merged in.
        this.toastSuccess(res.message || 'Folder added — indexing has started.');
        this.refresh();
      } else {
        this.toastError(res.message || 'Could not add folder.');
      }
    });
  }

  removeFolder(folder: WatchedFolder, purge: boolean): void {
    const confirmMsg = purge
      ? `Remove "${folder.path}" and delete its index? This cannot be undone.`
      : `Stop watching "${folder.path}"? Index data will be kept.`;
    if (!confirm(confirmMsg)) return;

    this.busyPath = folder.path;
    this.handle(this.libSvc.remove(folder.path, purge), res => {
      this.busyPath = null;
      if (res.success) {
        this.toastSuccess(purge ? 'Folder and index removed.' : 'Folder removed.');
        this.refresh();
      } else {
        this.toastError(res.message || 'Could not remove folder.');
      }
    });
  }

  togglePause(folder: WatchedFolder): void {
    const willPause = folder.status !== 'paused';
    this.busyPath = folder.path;
    this.handle(this.libSvc.setPaused(folder.path, willPause), res => {
      this.busyPath = null;
      if (res.success) this.refresh();
      else this.toastError(res.message || 'Could not change folder state.');
    });
  }

  rescan(folder: WatchedFolder): void {
    this.busyPath = folder.path;
    this.handle(this.libSvc.rescan(folder.path), res => {
      this.busyPath = null;
      if (res.success) {
        this.toastSuccess('Re-scan started.');
        // Refresh after a short delay so the status flips to "indexing".
        setTimeout(() => this.refresh(), 800);
      } else {
        this.toastError(res.message || 'Could not start re-scan.');
      }
    });
  }

  // ── View helpers ─────────────────────────────────────────────────

  totalImages(): number {
    return this.folders.reduce((sum, f) => sum + f.image_count, 0);
  }

  statusLabel(status: WatchedFolderStatus): string {
    const map: Record<WatchedFolderStatus, string> = {
      watching: 'Watching',
      indexing: 'Indexing',
      paused:   'Paused',
      error:    'Error',
      missing:  'Path missing',
    };
    return map[status] ?? status;
  }

  /** Relative-time helper for "last update X ago". */
  agoLabel(timestampSec: number): string {
    if (!timestampSec) return '';
    const diff = Date.now() / 1000 - timestampSec;
    if (diff < 60)        return `${Math.floor(diff)}s ago`;
    if (diff < 3600)      return `${Math.floor(diff / 60)}m ago`;
    if (diff < 86400)     return `${Math.floor(diff / 3600)}h ago`;
    return `${Math.floor(diff / 86400)}d ago`;
  }

  // ── Toast helpers ────────────────────────────────────────────────

  private toastSuccess(detail: string): void {
    this.messages.add({ key: 'app', severity: 'success', summary: 'Library', detail, life: 3500 });
  }
  private toastWarn(detail: string): void {
    this.messages.add({ key: 'app', severity: 'warn', summary: 'Library', detail, life: 4000 });
  }
  private toastError(detail: string): void {
    this.messages.add({ key: 'app', severity: 'error', summary: 'Library', detail, life: 5000 });
  }
}
