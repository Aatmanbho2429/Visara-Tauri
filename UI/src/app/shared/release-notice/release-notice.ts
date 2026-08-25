import { Component, inject, signal } from '@angular/core';
import { TauriService } from '../../services/tauri.service';

/**
 * One-time banner shown after a release that wiped the local library
 * (`core::migrate::run_library_reset` on the Rust side).
 *
 * A banner rather than a toast on purpose: the user has lost their watched
 * folders and has to re-add them, so the message must sit there until they
 * acknowledge it instead of disappearing on a timer.
 *
 * Whether to show it is asked of the backend, not stored in localStorage — the
 * wipe happens in Rust before the webview exists, and only Rust knows whether
 * anything was actually deleted (a fresh install gets no apology for data it
 * never had). Dismissal is likewise persisted backend-side, so quitting
 * without closing the banner does not swallow the message.
 *
 * State is signals because the app runs zoneless (Angular 21, no zone.js), so
 * a plain field assigned from a promise callback would not repaint.
 */
@Component({
  selector: 'app-release-notice',
  template: `
    @if (visible()) {
      <div class="release-notice" role="status">
        <div class="release-notice__bar"></div>

        <div class="release-notice__body">
          <div class="release-notice__head">
            <i class="pi pi-sparkles release-notice__icon"></i>
            <h3 class="release-notice__title">Your library needs re-adding — sorry about that</h3>
          </div>

          <p class="release-notice__text">
            This update rebuilds how folders are indexed, and the old index was not
            compatible with it. We had to clear it, so your folders are gone from the
            Library and will need adding again. Everything else — your account and
            settings — is untouched.
          </p>

          <ul class="release-notice__features">
            <li><i class="pi pi-check-circle"></i><span><strong>Sharper matching</strong> — a new image model recognises a design's family more reliably, including recolours and designs used inside a larger sheet</span></li>
            <li><i class="pi pi-search"></i><span><strong>Nothing cut off</strong> — results are no longer capped at a fixed number, so every close match is shown</span></li>
          </ul>
        </div>

        <button class="release-notice__close" (click)="dismiss()" title="Dismiss">
          <i class="pi pi-times"></i>
        </button>
      </div>
    }
  `,
})
export class ReleaseNotice {
  readonly visible = signal(false);

  private tauri = inject(TauriService);

  constructor() {
    this.tauri.resetNoticePending().then(pending => this.visible.set(pending));
  }

  dismiss(): void {
    // Hide immediately — the button should never feel like it did nothing
    // while the backend write is in flight.
    this.visible.set(false);
    void this.tauri.dismissResetNotice();
  }
}
