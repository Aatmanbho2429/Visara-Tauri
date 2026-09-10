import { Component } from '@angular/core';

// The counter-rotating ring orb used wherever the app is waiting on something.
//
// Extracted from `GlobalLoader` so the design has one definition rather than
// being retyped per use site. Styles live globally in
// `assets/styles/components/_global-loader.scss` (`.g-loader-*`), so this
// component deliberately declares none of its own.
//
// Renders the orb only — no backdrop. `GlobalLoader` wraps it in the
// full-screen scrim for blocking waits; inline waits (the "Model is loading…"
// state on the search page) drop it straight into the layout.
@Component({
  selector: 'app-loader-orb',
  template: `
    <div class="g-loader-orb">
      <div class="g-loader-ring g-loader-ring--1"></div>
      <div class="g-loader-ring g-loader-ring--2"></div>
      <div class="g-loader-ring g-loader-ring--3"></div>
      <div class="g-loader-core">
        <div class="g-loader-core__dot"></div>
      </div>
    </div>
  `,
})
export class LoaderOrb {}
