import { Component, inject, NgZone, OnDestroy, OnInit } from '@angular/core';
import { Router, RouterLink, RouterLinkActive, RouterOutlet } from '@angular/router';
import { TranslateModule } from '@ngx-translate/core';
import { RippleModule } from 'primeng/ripple';
import { AuthService } from '../../services/auth.service';
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
  expanded  = false;
  userState = inject(UserStateService);

  updateInfo:     UpdateInfo | null = null;
  updateDismissed = false;
  installing      = false;
  installPct      = 0;
  updateError:    string | null = null;

  private auth        = inject(AuthService);
  private router      = inject(Router);
  private searchState = inject(SearchStateService);
  private tauri       = inject(TauriService);
  private zone        = inject(NgZone);
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
