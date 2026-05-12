import { Component } from '@angular/core';
import { RouterLink, RouterLinkActive, RouterOutlet } from '@angular/router';
import { TranslateModule } from '@ngx-translate/core';
import { RippleModule } from 'primeng/ripple';

@Component({
  selector: 'app-master',
  imports: [RouterLink, RouterLinkActive, RouterOutlet, TranslateModule, RippleModule],
  templateUrl: './master.html',
  styleUrl: './master.scss',
})
export class Master {
  expanded = false;
}
