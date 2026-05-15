import { Component } from '@angular/core';
import { RouterOutlet } from '@angular/router';
import { TranslateService } from '@ngx-translate/core';
import { GlobalLoader } from './shared/global-loader/global-loader';

@Component({
  selector: 'app-root',
  imports: [RouterOutlet, GlobalLoader],
  templateUrl: './app.html',
  styleUrl: './app.scss'
})
export class App {
  constructor(translate: TranslateService) {
    translate.setDefaultLang('en');
    translate.use('en');
  }
}
