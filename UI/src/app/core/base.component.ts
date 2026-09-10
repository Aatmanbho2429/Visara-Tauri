import { ChangeDetectorRef, inject } from '@angular/core';
import { Observable } from 'rxjs';
import { ApiError } from '../models/response/apiResponse';

export abstract class BaseComponent {
  protected cdr = inject(ChangeDetectorRef);

  // `onError` receives the ApiError ZoneWrapperService.invoke() throws when
  // a call's statusCode falls outside 2xx — omit it for calls whose failure
  // path doesn't need its own handling.
  protected handle<T>(obs: Observable<T>, onSuccess: (value: T) => void, onError?: (err: ApiError) => void): void {
    obs.subscribe({
      next: value => { onSuccess(value); this.cdr.detectChanges(); },
      error: err => { onError?.(err as ApiError); this.cdr.detectChanges(); },
    });
  }
}
