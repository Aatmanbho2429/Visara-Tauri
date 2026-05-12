import { ChangeDetectorRef, inject } from '@angular/core';
import { Observable } from 'rxjs';

export abstract class BaseComponent {
  protected cdr = inject(ChangeDetectorRef);

  protected handle<T>(obs: Observable<T>, handler: (value: T) => void): void {
    obs.subscribe(value => {
      handler(value);
      this.cdr.detectChanges();
    });
  }
}
