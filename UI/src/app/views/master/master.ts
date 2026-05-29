import { Component, inject, NgZone, OnDestroy, OnInit } from '@angular/core';
import { Router, RouterLink, RouterLinkActive, RouterOutlet } from '@angular/router';
import { TranslateModule } from '@ngx-translate/core';
import { MessageService } from 'primeng/api';
import { RippleModule } from 'primeng/ripple';
import { AuthService } from '../../services/auth.service';
import { LibraryService } from '../../services/library.service';
import { UserStateService } from '../../services/user-state.service';
import { SearchStateService } from '../../services/search-state.service';
import { TauriService, UpdateInfo } from '../../services/tauri.service';
import type { UnlistenFn } from '@tauri-apps/api/event';

@Component({
  selector: 'app-master',
  imports: [RouterLink, RouterLinkActive, RouterOutlet, TranslateModule, RippleModule],
  templateUrl: './master.html',
  styleUrl: './master.scss',
})
export class Master implements OnInit, OnDestroy {
  /** Persistent flag — shown once to legacy users with an existing index
   *  but no watched folders so they understand to migrate. */
  private static readonly LIBRARY_MIGRATE_KEY = 'visara_library_migrate_tip_v1';

  expanded  = false;
  userState = inject(UserStateService);

  updateInfo:     UpdateInfo | null = null;
  updateDismissed = false;
  installing      = false;
  installPct      = 0;
  updateError:    string | null = null;

  private auth        = inject(AuthService);
  private libSvc      = inject(LibraryService);
  private router      = inject(Router);
  private searchState = inject(SearchStateService);
  private tauri       = inject(TauriService);
  private zone        = inject(NgZone);
  private messages    = inject(MessageService);
  private unlisten:   UnlistenFn[] = [];

  ngOnInit(): void {
    Promise.all([
      this.tauri.onUpdateAvailable(info => this.zone.run(() => { this.updateInfo = info; })),
      this.tauri.onUpdateProgress(p    => this.zone.run(() => {
        this.installing  = true;
        this.installPct  = p.total ? Math.round((p.downloaded / p.total) * 100) : 0;
      })),
      this.tauri.onUpdateError(msg => this.zone.run(() => {
        this.installing   = false;
        this.updateError  = msg;
      })),
    ]).then(fns => { this.unlisten = fns; });

    this.tauri.checkForUpdate();
    this.maybeShowMigrationTip();
  }

  /** Detect legacy users (rows in `files` but no `watched_folders` row).
   *  Show a one-time toast pointing them to the new Library page. */
  private maybeShowMigrationTip(): void {
    if (localStorage.getItem(Master.LIBRARY_MIGRATE_KEY)) return;

    this.libSvc.stats().subscribe(res => {
      if (!res.success || !res.data) return;
      const { watched_folder_count, total_indexed_files } = res.data;
      if (watched_folder_count === 0 && total_indexed_files > 0) {
        this.messages.add({
          key: 'app',
          severity: 'info',
          summary:  'Library is here',
          detail:   'Your existing index is preserved. Add your folders to the Library to enable auto-sync — existing embeddings are reused.',
          life:     12000,
        });
      }
      localStorage.setItem(Master.LIBRARY_MIGRATE_KEY, '1');
    });
  }

  ngOnDestroy(): void {
    this.unlisten.forEach(fn => fn());
  }

  installUpdate(): void {
    this.installing     = true;
    this.installPct     = 0;
    this.updateError    = null;
    this.tauri.installUpdate();
  }

  dismissUpdate(): void {
    this.updateDismissed = true;
  }

  logout(): void {
    this.auth.logout().subscribe();
    this.userState.clear();
    this.searchState.reset();
    this.router.navigate(['/']);
  }
}
