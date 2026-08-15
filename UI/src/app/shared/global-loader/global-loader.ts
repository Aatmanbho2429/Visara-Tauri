import { Component, inject } from '@angular/core';
import { AsyncPipe } from '@angular/common';
import { LoaderService } from '../../services/loader.service';
import { LoaderOrb } from '../loader-orb/loader-orb';

@Component({
  selector: 'app-global-loader',
  imports: [AsyncPipe, LoaderOrb],
  template: `
    @if (loader.loading$ | async) {
      <div class="g-loader-backdrop">
        <app-loader-orb />
      </div>
    }
  `,
})
export class GlobalLoader {
  loader = inject(LoaderService);
}
