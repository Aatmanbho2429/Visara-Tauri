import { Component, inject } from '@angular/core';
import { AsyncPipe } from '@angular/common';
import { LoaderService } from '../../services/loader.service';

@Component({
  selector: 'app-global-loader',
  imports: [AsyncPipe],
  template: `
    @if (loader.loading$ | async) {
      <div class="g-loader-backdrop">
        <div class="g-loader-orb">
          <div class="g-loader-ring g-loader-ring--1"></div>
          <div class="g-loader-ring g-loader-ring--2"></div>
          <div class="g-loader-ring g-loader-ring--3"></div>
          <div class="g-loader-core">
            <div class="g-loader-core__dot"></div>
          </div>
        </div>
      </div>
    }
  `,
})
export class GlobalLoader {
  loader = inject(LoaderService);
}
