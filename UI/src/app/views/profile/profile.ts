import { Component, OnInit, inject } from '@angular/core';
import { CommonModule } from '@angular/common';
import { TranslateModule } from '@ngx-translate/core';
import { BaseComponent } from '../../core/base.component';
import { AuthService } from '../../services/auth.service';
import { UserStateService } from '../../services/user-state.service';

@Component({
  selector: 'app-profile',
  imports: [CommonModule, TranslateModule],
  templateUrl: './profile.html',
  styleUrl: './profile.scss',
})
export class Profile extends BaseComponent implements OnInit {
  userState = inject(UserStateService);
  private auth = inject(AuthService);

  loading = true;

  ngOnInit(): void {
    // Always fetch fresh from Supabase via validate-token — no localStorage
    this.handle(this.auth.validateToken(), res => {
      this.loading = false;
      console.log(res)
      if (res.success && res.data?.user) {
        this.userState.set(res.data.user);
      }
    });
  }

  get statusLabel(): string {
    const map: Record<string, string> = {
      trial:     'Trial',
      active:    'Active',
      expired:   'Expired',
      exhausted: 'Limit Reached',
    };
    return map[this.userState.subscriptionStatus] ?? 'Unknown';
  }

  get subscriptionEndFormatted(): string {
    const end = this.userState.user?.subscription_end;
    if (!end) return '';
    return new Date(end).toLocaleDateString('en-IN', {
      day: 'numeric', month: 'long', year: 'numeric'
    });
  }
}
