import { Component, inject } from '@angular/core';
import { Router, RouterLink, RouterLinkActive, RouterOutlet } from '@angular/router';
import { TranslateModule } from '@ngx-translate/core';
import { RippleModule } from 'primeng/ripple';
import { AuthService } from '../../services/auth.service';
import { UserStateService } from '../../services/user-state.service';

@Component({
  selector: 'app-master',
  imports: [RouterLink, RouterLinkActive, RouterOutlet, TranslateModule, RippleModule],
  templateUrl: './master.html',
  styleUrl: './master.scss',
})
export class Master {
  expanded  = false;
  userState = inject(UserStateService);
  private auth   = inject(AuthService);
  private router = inject(Router);

  logout(): void {
    // Call Python to delete token file and unload model,
    // then always clear state and redirect regardless of result
    this.auth.logout().subscribe();
    this.userState.clear();
    this.router.navigate(['/']);
  }
}
