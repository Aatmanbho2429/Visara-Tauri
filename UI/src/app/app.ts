import { ApplicationRef, Component, inject, OnDestroy, OnInit } from '@angular/core';
import { Router, RouterOutlet } from '@angular/router';
import { TranslateService } from '@ngx-translate/core';
import { MessageService } from 'primeng/api';
import { ToastModule } from 'primeng/toast';
import { GlobalLoader } from './shared/global-loader/global-loader';
import { HotkeyService } from './services/hotkey/hotkey.service';
import { responseHotkeyPressed } from './models/response/responseMisc';
import { UpdateService } from './services/update/update.service';
import { UserStateService } from './services/user/user-state.service';
import { SearchStateService } from './services/search/search-state.service';

@Component({
  selector: 'app-root',
  imports: [RouterOutlet, GlobalLoader, ToastModule],
  providers: [MessageService],
  templateUrl: './app.html',
  styleUrl: './app.scss'
})
export class App implements OnInit, OnDestroy {
  // Key under which we stash a pending clipboard image path while user is logged out.
  static readonly PENDING_IMAGE_KEY = 'pictoria_pending_hotkey_image';

  // Persistent flag — once true, never show the intro tip again.
  private static readonly TIP_SEEN_KEY = 'pictoria_hotkey_tip_seen_v1';

  private hotkey       = inject(HotkeyService);
  private userState    = inject(UserStateService);
  private searchState  = inject(SearchStateService);
  private router       = inject(Router);
  private appRef       = inject(ApplicationRef);
  private messages     = inject(MessageService);
  private updates      = inject(UpdateService);

  private unlisten: (() => void) | null = null;

  constructor(translate: TranslateService) {
    translate.setDefaultLang('en');
    translate.use('en');
  }

  ngOnInit(): void {
    this.unlisten = this.hotkey.onHotkey(p => this.handleHotkey(p));

    // Start the update check here, at app boot, rather than in `Master` —
    // `Master` only mounts once the user reaches /master/..., so previously a
    // cold start did no check at all until after login and the banner only
    // turned up once something else caused a route change.
    this.updates.start();

    // Show the hot-key intro tip on the user's first session after this
    // release.  Delayed so the toast appears *after* the login transition,
    // not while it's still animating.
    if (!localStorage.getItem(App.TIP_SEEN_KEY)) {
      setTimeout(() => this.showHotkeyIntroTip(), 2500);
    }
  }

  private showHotkeyIntroTip(): void {
    if (localStorage.getItem(App.TIP_SEEN_KEY)) return;
    // Only show once the user is logged in — no point teaching the feature
    // before they have a working session.
    if (!this.userState.user) {
      setTimeout(() => this.showHotkeyIntroTip(), 2500);
      return;
    }

    const isMac = navigator.platform.toLowerCase().includes('mac');
    this.messages.add({
      key: 'app',
      severity: 'info',
      summary: 'New: search from anywhere',
      detail:  isMac
        ? 'Copy any image, then press ⌘ + Shift + V — Pictoria captures it and you can search instantly.'
        : 'Copy any image, then press Ctrl + Shift + V — Pictoria captures it and you can search instantly.',
      life:    10000,
      sticky:  false,
    });
    localStorage.setItem(App.TIP_SEEN_KEY, '1');
  }

  ngOnDestroy(): void {
    this.unlisten?.();
  }

  private handleHotkey(payload: responseHotkeyPressed): void {
    if (this.userState.user) {
      if (payload.hasImage) {
        this.loadClipboardIntoSearchState(payload.imagePath);
        this.messages.add({
          key: 'app',
          severity: 'success',
          summary:  'Clipboard image captured',
          detail:   'Click Find Similar to search your Library.',
          life:     3000,
        });
      } else {
        this.messages.add({
          key: 'app',
          severity: 'warn',
          summary:  'No image in clipboard',
          detail:   'Copy an image (right-click → Copy Image) and press Ctrl+Shift+V again.',
          life:     4000,
        });
      }
      this.router.navigate(['/master/search']).then(() => {
        // Force a change-detection pass: when the user is already on the
        // search route, Angular's default cycle does not always pick up
        // state mutations made from a Tauri event callback.
        this.appRef.tick();
      });
    } else if (payload.hasImage) {
      // Logged out → remember the captured image path; login.ts will pick
      // it up after sign-in.
      sessionStorage.setItem(App.PENDING_IMAGE_KEY, payload.imagePath);
    }
  }

  private loadClipboardIntoSearchState(imagePath: string): void {
    // Delegates to the shared helper so the real filename (file copy) vs the
    // generic "Clipboard image" (captured bitmap) labelling stays consistent
    // with the post-login path in login.ts.
    this.searchState.setClipboardImage(imagePath);
  }
}
