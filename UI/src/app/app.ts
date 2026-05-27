import { ApplicationRef, Component, inject, NgZone, OnDestroy, OnInit } from '@angular/core';
import { Router, RouterOutlet } from '@angular/router';
import { TranslateService } from '@ngx-translate/core';
import { convertFileSrc } from '@tauri-apps/api/core';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { MessageService } from 'primeng/api';
import { ToastModule } from 'primeng/toast';
import { GlobalLoader } from './shared/global-loader/global-loader';
import { TauriService, HotkeyEvent } from './services/tauri.service';
import { UserStateService } from './services/user-state.service';
import { SearchStateService } from './services/search-state.service';

@Component({
  selector: 'app-root',
  imports: [RouterOutlet, GlobalLoader, ToastModule],
  providers: [MessageService],
  templateUrl: './app.html',
  styleUrl: './app.scss'
})
export class App implements OnInit, OnDestroy {
  /** Key under which we stash a pending clipboard image path while user is logged out. */
  static readonly PENDING_IMAGE_KEY = 'visara_pending_hotkey_image';

  /** Persistent flag — once true, never show the intro tip again. */
  private static readonly TIP_SEEN_KEY = 'visara_hotkey_tip_seen_v1';

  private tauri       = inject(TauriService);
  private userState   = inject(UserStateService);
  private searchState = inject(SearchStateService);
  private router      = inject(Router);
  private zone        = inject(NgZone);
  private appRef      = inject(ApplicationRef);
  private messages    = inject(MessageService);

  private unlisten: UnlistenFn | null = null;

  constructor(translate: TranslateService) {
    translate.setDefaultLang('en');
    translate.use('en');
  }

  ngOnInit(): void {
    this.tauri.onHotkey(p => this.zone.run(() => this.handleHotkey(p)))
      .then(fn => { this.unlisten = fn; });

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
        ? 'Copy any image, then press ⌘ + Shift + V — Visara captures it and you can search instantly.'
        : 'Copy any image, then press Ctrl + Shift + V — Visara captures it and you can search instantly.',
      life:    10000,
      sticky:  false,
    });
    localStorage.setItem(App.TIP_SEEN_KEY, '1');
  }

  ngOnDestroy(): void {
    this.unlisten?.();
  }

  private handleHotkey(payload: HotkeyEvent): void {
    if (this.userState.user) {
      if (payload.has_image) {
        this.loadClipboardIntoSearchState(payload.image_path);
        this.messages.add({
          key: 'app',
          severity: 'success',
          summary:  'Clipboard image captured',
          detail:   'Pick a folder, then click Find Similar.',
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
    } else if (payload.has_image) {
      // Logged out → remember the captured image path; login.ts will pick
      // it up after sign-in.
      sessionStorage.setItem(App.PENDING_IMAGE_KEY, payload.image_path);
    }
  }

  private loadClipboardIntoSearchState(imagePath: string): void {
    this.searchState.imagePath    = imagePath;
    this.searchState.imageName    = 'Clipboard image';
    // Use Tauri's asset protocol to render the temp PNG in the pick-card.
    this.searchState.imagePreview = convertFileSrc(imagePath);
  }
}
