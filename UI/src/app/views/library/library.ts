import { Component, OnInit, inject } from '@angular/core';
import { CommonModule } from '@angular/common';
import { TranslateModule } from '@ngx-translate/core';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { MessageService } from 'primeng/api';
import { BaseComponent } from '../../core/base.component';
import { LibraryService } from '../../services/library.service';
import { WatchedFolder, WatchedFolderStatus } from '../../models/library.model';

@Component({
  selector: 'app-library',
  imports: [CommonModule, TranslateModule],
  templateUrl: './library.html',
  styleUrl: './library.scss',
})
export class Library extends BaseComponent implements OnInit {
  private libSvc   = inject(LibraryService);
  private messages = inject(MessageService);

  folders: WatchedFolder[] = [];
  loading                  = true;
  /** Path currently being added — disables the Add button while running. */
  addingPath: string | null = null;
  /** Path with an in-flight action (remove/pause/rescan) — keeps card busy. */
  busyPath: string | null   = null;

  ngOnInit(): void {
    this.refresh();
  }

  refresh(): void {
    this.loading = true;
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
        this.toastSuccess('Folder added — indexing has started.');
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
