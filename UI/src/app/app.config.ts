import { ApplicationConfig, inject, provideAppInitializer, provideBrowserGlobalErrorListeners } from '@angular/core';
import { provideRouter } from '@angular/router';
import { provideAnimationsAsync } from '@angular/platform-browser/animations/async';
import { provideHttpClient } from '@angular/common/http';
import { provideTranslateService } from '@ngx-translate/core';
import { provideTranslateHttpLoader } from '@ngx-translate/http-loader';
import { providePrimeNG } from 'primeng/config';
import { definePreset } from '@primeuix/themes';
import Aura from '@primeuix/themes/aura';
import { ConfigService } from './services/config.service';

import { routes } from './app.routes';

const VisaraPreset = definePreset(Aura, {
  semantic: {
    primary: {
      50:  '{fuchsia.50}',
      100: '{fuchsia.100}',
      200: '{fuchsia.200}',
      300: '{fuchsia.300}',
      400: '{fuchsia.400}',
      500: '{fuchsia.500}',
      600: '{fuchsia.600}',
      700: '{fuchsia.700}',
      800: '{fuchsia.800}',
      900: '{fuchsia.900}',
      950: '{fuchsia.950}'
    }
  }
});

export const appConfig: ApplicationConfig = {
  providers: [
    provideBrowserGlobalErrorListeners(),
    provideRouter(routes),
    provideAnimationsAsync(),
    provideHttpClient(),
    provideAppInitializer(() => inject(ConfigService).load()),
    provideTranslateService({ defaultLanguage: 'en' }),
    provideTranslateHttpLoader({ prefix: './assets/i18n/', suffix: '.json' }),
    providePrimeNG({
      theme: {
        preset: VisaraPreset,
        options: {
          darkModeSelector: '.dark-mode',
          cssLayer: { name: 'primeng', order: 'tailwind-base, primeng, tailwind-utilities' }
        }
      }
    })
  ]
};
