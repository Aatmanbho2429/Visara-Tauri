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
  private updateTimer: ReturnType<typeof setInterval> | null = null;
  private revalidateTimer: ReturnType<typeof setInterval> | null = null;

  /** Re-check for updates every 6 hours so long-running tray sessions still
   *  get notified without a restart. */
  private static readonly UPDATE_INTERVAL_MS = 6 * 60 * 60 * 1000;

  /** Silently re-check the session/subscription against Supabase every 30
   *  minutes so a long-running tray session notices an expired/revoked
   *  subscription without needing a restart. */
  private static readonly REVALIDATE_INTERVAL_MS = 30 * 60 * 1000;

  ngOnInit(): void {
    // Register the event listeners FIRST, then trigger the check — otherwise the
    // backend can emit `update_available` before we're listening and the banner
    // is silently missed.
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
    ]).then(fns => {
      this.unlisten = fns;
      this.tauri.checkForUpdate();
    });

    this.updateTimer = setInterval(() => this.tauri.checkForUpdate(), Master.UPDATE_INTERVAL_MS);
    this.revalidateTimer = setInterval(() => this.revalidateSession(), Master.REVALIDATE_INTERVAL_MS);
    this.maybeShowMigrationTip();
  }

  /** Background re-check of the session/subscription against Supabase.
   *  Silent (no loading spinner) — only acts when the session must end or
   *  the user's subscription info has changed. */
  private revalidateSession(): void {
    this.auth.periodicRevalidate().subscribe(res => {
      if (!res.success || !res.data) return;
      const { action, user } = res.data;

      if (action === 'logout') {
        this.messages.add({
          key: 'app',
          severity: 'warn',
          summary: 'Session ended',
          detail: 'Your session has expired. Please sign in again.',
          life: 6000,
        });
        this.userState.clear();
        this.searchState.reset();
        this.router.navigate(['/']);
      } else if (action === 'ok' && user) {
        this.userState.set(user);
      }
    });
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
    if (this.updateTimer) clearInterval(this.updateTimer);
    if (this.revalidateTimer) clearInterval(this.revalidateTimer);
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
