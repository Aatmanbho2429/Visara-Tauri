import { Component, inject, OnDestroy, OnInit } from '@angular/core';
import { Router, RouterLink, RouterLinkActive, RouterOutlet } from '@angular/router';
import { TranslateModule } from '@ngx-translate/core';
import { MessageService } from 'primeng/api';
import { RippleModule } from 'primeng/ripple';
import { AuthService } from '../../services/auth.service';
import { LibraryService } from '../../services/library.service';
import { UserStateService } from '../../services/user-state.service';
import { SearchStateService } from '../../services/search-state.service';
import { UpdateService } from '../../services/update.service';

@Component({
  selector: 'app-master',
  imports: [RouterLink, RouterLinkActive, RouterOutlet, TranslateModule, RippleModule],
  templateUrl: './master.html',
  styleUrl: './master.scss',
})
export class Master implements OnInit, OnDestroy {
  /** Persistent flag — shown once to legacy users with an existing index
   *  but no watched folders so they understand to migrate. */
  private static readonly LIBRARY_MIGRATE_KEY = 'pictoria_library_migrate_tip_v1';

  expanded  = false;
  userState = inject(UserStateService);

  /** Updater state is owned by the root `UpdateService`, not by this
   *  component — the check runs at app boot and the answer is cached there,
   *  so the banner is already populated the moment this shell renders
   *  instead of waiting on a check that only started when it mounted. */
  updates = inject(UpdateService);

  private auth        = inject(AuthService);
  private libSvc      = inject(LibraryService);
  private router      = inject(Router);
  private searchState = inject(SearchStateService);
  private messages    = inject(MessageService);
  private revalidateTimer: ReturnType<typeof setInterval> | null = null;

  /** Silently re-check the session/subscription against Supabase every 30
   *  minutes so a long-running tray session notices an expired/revoked
   *  subscription without needing a restart. */
  private static readonly REVALIDATE_INTERVAL_MS = 30 * 60 * 1000;

  ngOnInit(): void {
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
    if (this.revalidateTimer) clearInterval(this.revalidateTimer);
  }

  logout(): void {
    this.auth.logout().subscribe();
    this.userState.clear();
    this.searchState.reset();
    this.router.navigate(['/']);
  }
}
