import { Component, inject } from '@angular/core';
import { CommonModule } from '@angular/common';
import { TranslateModule } from '@ngx-translate/core';
import { UpdateService } from '../../services/update/update.service';

// docs/plans/force-update.md — a non-dismissible overlay that covers the
// entire app, including the login screen, whenever `UpdateService.required()`
// is true. Deliberately NOT `p-dialog`: PrimeNG dialogs are closable by
// Escape and mask-click by default, which is exactly the behaviour this must
// not have. A plain fixed-position overlay has no escape affordance to
// disable in the first place.
//
// Residual risk, accepted in writing (plan §2.5): there is no server-side
// kill switch here. If a release ships a broken `latest.json` or a bad
// signature, every user sees this overlay with a failing Update button —
// the only real button click is the error path, which is why §2 exists.
// The recovery path is publishing a GOOD release; on their next check
// (`UpdateService`'s 6-hour timer, or relaunch) `update.check()` offers that
// one instead. The Retry / manual-download / Quit escape hatches below exist
// so nobody is stuck staring at a dead end in the meantime.
@Component({
  selector: 'app-force-update',
  imports: [CommonModule, TranslateModule],
  templateUrl: './force-update.html',
})
export class ForceUpdate {
  updates = inject(UpdateService);
}
